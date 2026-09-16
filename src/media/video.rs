//! AV1 decoding through rav1d (the memory-safe Rust port of dav1d), built
//! without its assembly so no C compiler or nasm is needed, and the YUV to
//! RGBA conversion that turns its pictures into texture-ready bytes.
//!
//! rav1d exposes dav1d's C API shape (`dav1d_open`, `dav1d_send_data`,
//! `dav1d_get_picture` ...) as `unsafe extern "C"` functions over plain
//! `#[repr(C)]` structs. This file is the ONLY place those are called; the
//! rest of the player sees a safe `Av1Decoder`. The decode loop is the one
//! dav1d documents: send a packet, pull pictures until the decoder says "try
//! again", and if the packet was not fully consumed (the decoder wanted the
//! pictures drained first) send it again.

use std::mem::MaybeUninit;
use std::ptr::NonNull;

use rav1d::include::dav1d::data::Dav1dData;
use rav1d::include::dav1d::dav1d::{Dav1dContext, Dav1dSettings};
use rav1d::include::dav1d::headers::{
    DAV1D_MC_BT2020_NCL, DAV1D_MC_BT601, DAV1D_MC_BT709, DAV1D_MC_IDENTITY, DAV1D_PIXEL_LAYOUT_I400,
    DAV1D_PIXEL_LAYOUT_I420, DAV1D_PIXEL_LAYOUT_I422, DAV1D_PIXEL_LAYOUT_I444,
};
use rav1d::include::dav1d::picture::Dav1dPicture;
use rav1d::src::lib::{
    dav1d_close, dav1d_data_create, dav1d_data_unref, dav1d_default_settings, dav1d_get_picture,
    dav1d_open, dav1d_picture_unref, dav1d_send_data,
};

use super::{MediaError, VideoFrame};

/// dav1d's C convention: results are 0 or a NEGATIVE errno. "Try again" from
/// `get_picture` means "feed me more input"; from `send_data` it means "drain
/// my pictures first, then send this packet again". Taken from libc so the
/// value is right on every platform (11 on Linux and Windows, 35 on macOS).
const AGAIN: i32 = -libc::EAGAIN;

/// A safe handle on one rav1d decoder context.
pub struct Av1Decoder {
    ctx: Dav1dContext,
}

// SAFETY: `Dav1dContext` is rav1d's `RawArc<Rav1dContext>`, a reference-counted
// pointer to state rav1d guards with its own locks; dav1d's API contract is
// that one thread at a time drives a context, which is how this type is
// used (created and driven on the decode thread, or on a test thread).
unsafe impl Send for Av1Decoder {}

impl Av1Decoder {
    /// `threads` = 0 lets rav1d pick one thread per logical processor; the
    /// benchmark passes 1 to measure single-core cost. Frame delay (how many
    /// frames stay in flight) is left on rav1d's automatic choice.
    pub fn new(threads: i32) -> Result<Self, MediaError> {
        let mut settings = MaybeUninit::<Dav1dSettings>::uninit();
        // SAFETY: `settings` is writable stack memory of the right type;
        // dav1d_default_settings fully initialises it.
        unsafe {
            dav1d_default_settings(NonNull::new(settings.as_mut_ptr()).expect("stack address"));
        }
        // SAFETY: initialised by the call above.
        let mut settings = unsafe { settings.assume_init() };
        settings.n_threads = threads.clamp(0, 256);
        settings.max_frame_delay = 0;

        let mut ctx: Option<Dav1dContext> = None;
        // SAFETY: both pointers are to live locals; dav1d_open reads the
        // settings and writes the context slot.
        let result = unsafe { dav1d_open(Some(NonNull::from(&mut ctx)), Some(NonNull::from(&settings))) };
        if result.0 != 0 {
            return Err(MediaError::Decoder(format!("rav1d could not open a decoder (code {})", result.0)));
        }
        let ctx = ctx.ok_or_else(|| MediaError::Decoder("rav1d returned no decoder context".into()))?;
        Ok(Self { ctx })
    }

    /// Decode one container packet and hand every picture that becomes ready
    /// to `on_frame`, in display order, tagged with the pts this packet
    /// carried (rav1d copies the packet's timestamp onto the picture it
    /// produces, which is how pts survives the decoder's reordering).
    /// Returns Ok(false) if `on_frame` asked to stop (returned false).
    pub fn decode_packet(
        &mut self,
        data: &[u8],
        pts_ns: i64,
        on_frame: &mut dyn FnMut(VideoFrame) -> bool,
    ) -> Result<bool, MediaError> {
        let mut packet = Dav1dData::default();
        // SAFETY: `packet` is a live local; on success dav1d_data_create fills
        // it with a buffer of exactly `data.len()` bytes and returns that
        // buffer's start.
        let dst = unsafe { dav1d_data_create(Some(NonNull::from(&mut packet)), data.len()) };
        if dst.is_null() {
            return Err(MediaError::Decoder("rav1d could not allocate a packet buffer".into()));
        }
        // SAFETY: `dst` points at `data.len()` writable bytes (see above) and
        // `data` is a valid slice of that length; the regions cannot overlap.
        unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), dst, data.len()) };
        packet.m.timestamp = pts_ns;

        let outcome = self.feed(&mut packet, on_frame);
        // Whatever happened, release our reference to the packet buffer. On the
        // consumed path rav1d has already taken it and this is a no-op on an
        // emptied struct; on an error path it frees the copy.
        // SAFETY: `packet` is a live local in whatever state rav1d left it,
        // which dav1d_data_unref accepts (it takes the value out).
        unsafe { dav1d_data_unref(Some(NonNull::from(&mut packet))) };
        outcome
    }

    /// The send / drain loop for one packet (split out so the caller above
    /// can unref the packet on every exit path).
    fn feed(&mut self, packet: &mut Dav1dData, on_frame: &mut dyn FnMut(VideoFrame) -> bool) -> Result<bool, MediaError> {
        loop {
            // SAFETY: the context came from dav1d_open and has not been closed;
            // `packet` is a live, initialised Dav1dData.
            let sent = unsafe { dav1d_send_data(Some(self.ctx), Some(NonNull::from(&mut *packet))) };
            if sent.0 != 0 && sent.0 != AGAIN {
                return Err(MediaError::Decoder(format!("rav1d rejected a packet (code {})", sent.0)));
            }
            if !self.drain(on_frame)? {
                return Ok(false);
            }
            // rav1d zeroes `sz` once it has taken the whole packet. A non-zero
            // size after a send means "drained now, send the rest again".
            if packet.sz == 0 {
                return Ok(true);
            }
        }
    }

    /// Pull every picture the decoder has ready. Also the end-of-stream
    /// flush: with no more packets coming, repeated calls hand out the
    /// frames still in flight until the decoder is empty. Returns Ok(false)
    /// if `on_frame` asked to stop.
    pub fn drain(&mut self, on_frame: &mut dyn FnMut(VideoFrame) -> bool) -> Result<bool, MediaError> {
        loop {
            let mut picture = Dav1dPicture::default();
            // SAFETY: valid context; `picture` is a live local rav1d fills in.
            let got = unsafe { dav1d_get_picture(Some(self.ctx), Some(NonNull::from(&mut picture))) };
            if got.0 == AGAIN {
                return Ok(true);
            }
            if got.0 != 0 {
                return Err(MediaError::Decoder(format!("rav1d failed to produce a picture (code {})", got.0)));
            }
            let frame = picture_to_frame(&picture);
            // SAFETY: `picture` was filled by a successful dav1d_get_picture
            // and is released exactly once, here, before it goes out of scope.
            unsafe { dav1d_picture_unref(Some(NonNull::from(&mut picture))) };
            let frame = frame?;
            if !on_frame(frame) {
                return Ok(false);
            }
        }
    }
}

impl Drop for Av1Decoder {
    fn drop(&mut self) {
        let mut slot = Some(self.ctx);
        // SAFETY: the context came from dav1d_open and this is its only close;
        // dav1d_close joins rav1d's worker threads and frees the state.
        unsafe { dav1d_close(Some(NonNull::from(&mut slot))) };
    }
}

/// The YUV to RGB matrix in use, chosen from the sequence header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMatrix {
    Bt601,
    Bt709,
    Bt2020,
    /// The planes are G, B, R already (AV1's identity matrix, 4:4:4 only).
    Identity,
}

/// Pick the matrix from AV1's `matrix_coefficients` value, falling back to
/// the convention players use when a file says "unknown": BT.709 for HD
/// heights and BT.601 for smaller pictures.
pub fn choose_matrix(mtrx: u32, height: u32) -> ColorMatrix {
    match mtrx {
        m if m == DAV1D_MC_IDENTITY => ColorMatrix::Identity,
        m if m == DAV1D_MC_BT709 => ColorMatrix::Bt709,
        m if m == DAV1D_MC_BT601 => ColorMatrix::Bt601,
        m if m == DAV1D_MC_BT2020_NCL => ColorMatrix::Bt2020,
        _ if height >= 720 => ColorMatrix::Bt709,
        _ => ColorMatrix::Bt601,
    }
}

/// Everything the converter needs to know about a picture's layout, read
/// once per frame from rav1d's structs.
struct PlaneLayout {
    width: usize,
    height: usize,
    /// Chroma subsampling shifts: (horizontal, vertical). 4:2:0 is (1, 1).
    ss_hor: u32,
    ss_ver: u32,
    /// True for I400 (grey; no chroma planes at all).
    grey: bool,
    /// Bits per component: 8, 10 or 12.
    bpc: u32,
    /// Limited (studio) range when false; full range when true.
    full_range: bool,
    matrix: ColorMatrix,
}

/// Turn a rav1d picture into tightly packed RGBA8. 10 and 12-bit pictures are
/// shifted down to 8 bits (no tone mapping; HDR presentation is not this
/// rung's job). Chroma is sampled nearest-neighbour, which is what the GPU
/// path will do too once conversion moves into a shader.
fn picture_to_frame(pic: &Dav1dPicture) -> Result<VideoFrame, MediaError> {
    let width = pic.p.w.max(0) as usize;
    let height = pic.p.h.max(0) as usize;
    if width == 0 || height == 0 {
        return Err(MediaError::Decoder("rav1d produced an empty picture".into()));
    }
    let (ss_hor, ss_ver, grey) = match pic.p.layout {
        l if l == DAV1D_PIXEL_LAYOUT_I420 => (1, 1, false),
        l if l == DAV1D_PIXEL_LAYOUT_I422 => (1, 0, false),
        l if l == DAV1D_PIXEL_LAYOUT_I444 => (0, 0, false),
        l if l == DAV1D_PIXEL_LAYOUT_I400 => (0, 0, true),
        other => return Err(MediaError::Decoder(format!("unknown pixel layout {other}"))),
    };
    let bpc = pic.p.bpc.max(8) as u32;
    // SAFETY: rav1d sets `seq_hdr` to a valid pointer for every picture it
    // returns; it stays valid until the picture is unref'd, which happens
    // after this function returns.
    let (mtrx, color_range) = match pic.seq_hdr {
        Some(hdr) => unsafe {
            let hdr = hdr.as_ref();
            (hdr.mtrx, hdr.color_range)
        },
        None => (DAV1D_MC_BT601, 0),
    };
    let layout = PlaneLayout {
        width,
        height,
        ss_hor,
        ss_ver,
        grey,
        bpc,
        full_range: color_range != 0,
        matrix: choose_matrix(mtrx, height as u32),
    };

    // Plane byte views. Chroma planes are absent for grey pictures.
    let bytes_per_sample = if bpc > 8 { 2 } else { 1 };
    let cw = if grey { 0 } else { (width + (1 << ss_hor) - 1) >> ss_hor };
    let ch = if grey { 0 } else { (height + (1 << ss_ver) - 1) >> ss_ver };
    // SAFETY (all three): `pic` came from a successful dav1d_get_picture and
    // is unref'd only after this function returns, so its plane buffers
    // outlive every slice made here.
    let y = unsafe { plane_bytes(pic, 0, width, height, pic.stride[0], bytes_per_sample)? };
    let u = unsafe { plane_bytes(pic, 1, cw, ch, pic.stride[1], bytes_per_sample)? };
    let v = unsafe { plane_bytes(pic, 2, cw, ch, pic.stride[1], bytes_per_sample)? };

    let rgba = yuv_to_rgba(&layout, y, pic.stride[0].max(0) as usize, u, v, pic.stride[1].max(0) as usize);
    Ok(VideoFrame {
        rgba,
        width: width as u32,
        height: height as u32,
        pts_s: pic.m.timestamp as f64 / 1e9,
    })
}

/// A byte view of one plane of a rav1d picture: `h` rows of `w` samples at
/// `stride` bytes apart. Empty when the plane has no size (grey pictures).
///
/// # Safety
/// `pic` must be a picture rav1d returned and not yet unref'd; rav1d
/// guarantees each plane pointer addresses at least `stride * rows` bytes
/// for the picture's lifetime, and the slice must not outlive it.
unsafe fn plane_bytes<'a>(
    pic: &'a Dav1dPicture,
    idx: usize,
    w: usize,
    h: usize,
    stride: isize,
    bytes_per_sample: usize,
) -> Result<&'a [u8], MediaError> {
    if w == 0 || h == 0 {
        return Ok(&[]);
    }
    let ptr = pic.data[idx].ok_or_else(|| MediaError::Decoder(format!("rav1d picture is missing plane {idx}")))?;
    let stride = stride.max(0) as usize;
    let len = stride * (h - 1) + w * bytes_per_sample;
    Ok(std::slice::from_raw_parts(ptr.as_ptr() as *const u8, len))
}

/// Fixed-point (16 fractional bits) YUV to RGBA. Coefficients are the
/// standard ones for each matrix (BT.601, BT.709, BT.2020 non-constant
/// luminance); limited range scales Y by 255/219 and chroma by 255/224.
fn yuv_to_rgba(l: &PlaneLayout, y: &[u8], y_stride: usize, u: &[u8], v: &[u8], c_stride: usize) -> Vec<u8> {
    const FIX: i32 = 16;
    const ONE: f64 = (1i64 << FIX) as f64;
    let fx = |x: f64| (x * ONE).round() as i32;

    let shift = l.bpc - 8;
    let read = |plane: &[u8], row_start: usize, x: usize| -> i32 {
        if l.bpc > 8 {
            let i = row_start + x * 2;
            (u16::from_le_bytes([plane[i], plane[i + 1]]) >> shift) as i32
        } else {
            plane[row_start + x] as i32
        }
    };

    let (ky, y_off, kc) = if l.full_range { (fx(1.0), 0, fx(1.0)) } else { (fx(255.0 / 219.0), 16, fx(255.0 / 224.0)) };
    // (R from Cr, G from Cb, G from Cr, B from Cb), already multiplied by the
    // chroma range scale so each pixel does one multiply per term.
    let (r_cr, g_cb, g_cr, b_cb) = match l.matrix {
        ColorMatrix::Bt601 => (1.402, -0.344136, -0.714136, 1.772),
        ColorMatrix::Bt709 => (1.5748, -0.187324, -0.468124, 1.8556),
        ColorMatrix::Bt2020 => (1.4746, -0.164553, -0.571353, 1.8814),
        ColorMatrix::Identity => (0.0, 0.0, 0.0, 0.0),
    };
    let kc_f = kc as f64 / ONE;
    let (r_cr, g_cb, g_cr, b_cb) = (fx(r_cr * kc_f), fx(g_cb * kc_f), fx(g_cr * kc_f), fx(b_cb * kc_f));
    let half = 1 << (FIX - 1);
    let clamp8 = |v: i32| -> u8 { ((v + half) >> FIX).clamp(0, 255) as u8 };

    let mut out = vec![0u8; l.width * l.height * 4];
    for row in 0..l.height {
        let y_row = row * y_stride;
        let c_row = (row >> l.ss_ver) * c_stride;
        let out_row = &mut out[row * l.width * 4..(row + 1) * l.width * 4];
        for (x, px) in out_row.chunks_exact_mut(4).enumerate() {
            let yy = read(y, y_row, x);
            if l.grey {
                let g = clamp8((yy - y_off) * ky);
                px.copy_from_slice(&[g, g, g, 255]);
                continue;
            }
            let cx = x >> l.ss_hor;
            let uu = read(u, c_row, cx);
            let vv = read(v, c_row, cx);
            if l.matrix == ColorMatrix::Identity {
                // AV1 identity: plane order is G, B, R.
                let g = clamp8((yy - y_off) * ky);
                let b = clamp8((uu - y_off) * ky);
                let r = clamp8((vv - y_off) * ky);
                px.copy_from_slice(&[r, g, b, 255]);
                continue;
            }
            let yl = (yy - y_off) * ky;
            let cb = uu - 128;
            let cr = vv - 128;
            let r = clamp8(yl + r_cr * cr);
            let g = clamp8(yl + g_cb * cb + g_cr * cr);
            let b = clamp8(yl + b_cb * cb);
            px.copy_from_slice(&[r, g, b, 255]);
        }
    }
    out
}

/// Decode every video frame of a file synchronously on the calling thread,
/// handing each one to `on_frame`. Used by the tests (which collect the
/// frames) and the benchmark (which only counts them). Returns the count.
pub fn decode_video<F: FnMut(VideoFrame)>(path: &std::path::Path, threads: i32, mut on_frame: F) -> Result<usize, MediaError> {
    let info = super::probe(path)?;
    let mut mkv = super::open_demuxer(path)?;
    let mut decoder = Av1Decoder::new(threads)?;
    let mut packet = matroska_demuxer::Frame::default();
    let mut count = 0usize;
    let mut deliver = |frame: VideoFrame| -> bool {
        count += 1;
        on_frame(frame);
        true
    };
    while mkv.next_frame(&mut packet)? {
        if packet.track != info.video_track {
            continue;
        }
        let pts_ns = packet.timestamp.saturating_mul(info.timestamp_scale_ns) as i64;
        decoder.decode_packet(&packet.data, pts_ns, &mut deliver)?;
    }
    decoder.drain(&mut deliver)?;
    Ok(count)
}
