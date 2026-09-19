//! `frame-sink`: the game's side of the spike, measured.
//!
//! The browser host (`tools/cef-probe`) publishes finished frames into shared
//! memory. This program is what the game would be: it takes them out again and
//! puts them on the GPU. It answers two of the spike's questions.
//!
//! **The pipe.** How many of the published frames actually arrive, how stale
//! they are when they do, and what taking them out costs. The staleness number
//! is a true cross-process one: the publisher stamps each frame with the
//! machine's shared performance counter, and this process reads the same
//! counter, so the difference is real elapsed time and not a guess.
//!
//! **The GPU.** What it costs to put one 1280x720 frame into a wgpu texture,
//! which is exactly what `ScreenSurface::write_pixels` does for every video
//! and stream screen in the game today. Three numbers come out:
//!
//!   1. the cost of the `write_texture` call itself, which is a copy into
//!      wgpu's staging memory and is paid on the game's main thread;
//!   2. the cost of upload plus submit plus waiting for the GPU to finish,
//!      minus the cost of submit plus wait on their own. That difference is
//!      the honest wall-clock the frame pays for one upload;
//!   3. where the adapter allows it, a GPU timestamp around the same copy, so
//!      the number can sit beside `gpu.screen_ui` in the same units.
//!
//! Standalone: not part of the engine, not in its Cargo.toml.
//!
//! Usage:
//! ```text
//! frame-sink --shm=humanity_cef_probe --seconds=30 --gpu=1
//! frame-sink --bench-only=1            (GPU numbers with no publisher)
//! ```

#[path = "../../frame_shm.rs"]
mod frame_shm;

use std::collections::HashMap;
use std::time::{Duration, Instant};

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;

fn main() {
    let args = Args::parse();

    if !args.bench_only {
        run_pipe_measurement(&args);
    }
    if args.gpu {
        run_gpu_measurement(&args);
    }
}

// ---------------------------------------------------------------------------
// Measurement 4: the pipe
// ---------------------------------------------------------------------------

fn run_pipe_measurement(args: &Args) {
    let ring = match frame_shm::FrameRing::open(&args.shm) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("[sink] {e}");
            std::process::exit(1);
        }
    };
    println!(
        "[sink] opened '{}': {}x{}, {} slots, {:.1} MB of shared memory",
        args.shm,
        ring.width,
        ring.height,
        ring.slot_count,
        ring.total_bytes() as f64 / 1048576.0
    );

    let frame_bytes = ring.width as usize * ring.height as usize * 4;
    let mut buf = vec![0u8; frame_bytes];

    // A GPU is optional here: the pipe can be measured on its own, and is,
    // so the two costs never hide inside one another.
    let gpu = if args.gpu { Gpu::new() } else { None };

    let produced_at_start = ring.produced();
    let t0 = Instant::now();
    let deadline = t0 + Duration::from_secs(args.seconds);

    let mut received = 0u64;
    let mut last_seen = 0u64;
    let mut races = 0u64;
    let mut read_ns = 0u64;
    let mut upload_ns = 0u64;
    let mut latencies_ms: Vec<f64> = Vec::with_capacity(4096);
    let qpf = ring.qpf as f64;

    while Instant::now() < deadline {
        let t = Instant::now();
        match ring.read_newest(last_seen, &mut buf) {
            Some((id, published_qpc)) => {
                read_ns += t.elapsed().as_nanos() as u64;
                // Age of the frame at the moment we finished copying it out.
                let age_ticks = frame_shm::qpc().saturating_sub(published_qpc);
                latencies_ms.push(age_ticks as f64 * 1000.0 / qpf);
                last_seen = id;
                received += 1;
                if let Some(g) = &gpu {
                    let tu = Instant::now();
                    g.upload(&buf);
                    upload_ns += tu.elapsed().as_nanos() as u64;
                }
            }
            None => {
                // Either nothing new, or the one unlucky case where the writer
                // landed in our slot. The ring reports the latter as a repeat
                // of "nothing new", so count how often we come back empty
                // while the producer is clearly ahead of us.
                if ring.produced() > last_seen {
                    races += 1;
                }
                // A short yield: the consumer is a game frame loop in real
                // life, not a spinner.
                std::thread::sleep(Duration::from_micros(200));
            }
        }
    }

    let elapsed = t0.elapsed().as_secs_f64();
    let produced = ring.produced() - produced_at_start;
    let (p50, p95, p99, worst) = percentiles(&latencies_ms);
    let mb_per_second = received as f64 * frame_bytes as f64 / 1048576.0 / elapsed;

    println!();
    println!("================ PIPE ================");
    println!("window                 {elapsed:.2} s");
    println!("frames published       {produced}");
    println!("frames received        {received}  ({:.1} per second)", received as f64 / elapsed);
    println!(
        "frames skipped         {}  (the consumer took the newest, not every one)",
        produced.saturating_sub(received)
    );
    println!("read retries           {races}");
    println!("throughput             {mb_per_second:.1} MB/s of pixels moved between processes");
    println!(
        "copy out of shared mem {:.3} ms/frame  ({:.1} GB/s)",
        ms_per(read_ns, received),
        gbps(read_ns, received, frame_bytes)
    );
    println!("frame age at pickup    p50 {p50:.2} ms  p95 {p95:.2}  p99 {p99:.2}  worst {worst:.2}");
    if gpu.is_some() {
        println!(
            "write_texture call     {:.3} ms/frame (in-line, while frames were arriving)",
            ms_per(upload_ns, received)
        );
    }
}

// ---------------------------------------------------------------------------
// Measurement 5: the GPU upload
// ---------------------------------------------------------------------------

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    texture: wgpu::Texture,
    /// A staging buffer holding one frame, for the encoder-timed copy.
    staging: wgpu::Buffer,
    timestamps: Option<(wgpu::QuerySet, wgpu::Buffer, wgpu::Buffer, f32)>,
}

impl Gpu {
    fn new() -> Option<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))?;
        let info = adapter.get_info();
        println!("[sink] adapter: {} ({:?}, {:?})", info.name, info.device_type, info.backend);

        // Ask for exactly the timestamp features the adapter has, the same
        // "intersection can never fail" trick the engine uses when it creates
        // its device.
        let wanted = wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
        let granted = adapter.features() & wanted;
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("frame-sink"),
                required_features: granted,
                // The engine asks for the default limits (v0.784.2); matching
                // that keeps the comparison honest.
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            },
            None,
        ))
        .ok()?;

        // The same texture a wall screen has: a colour target that the scene
        // samples, that pixels can be written into.
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("screen surface (stand-in)"),
            size: wgpu::Extent3d { width: WIDTH, height: HEIGHT, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // SURFACE_FORMAT in src/gui/screen_surface.rs.
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("one frame, staged"),
            size: (WIDTH * HEIGHT * 4) as u64,
            usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let timestamps = if granted.contains(wgpu::Features::TIMESTAMP_QUERY)
            && granted.contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS)
        {
            let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
                label: Some("upload timing"),
                ty: wgpu::QueryType::Timestamp,
                count: 2,
            });
            let resolve = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("resolve"),
                size: 16,
                usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            });
            let read = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("readback"),
                size: 16,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            Some((query_set, resolve, read, queue.get_timestamp_period()))
        } else {
            println!("[sink] this adapter cannot time inside an encoder; wall-clock only");
            None
        };

        Some(Self { device, queue, texture, staging, timestamps })
    }

    /// Exactly what `ScreenSurface::write_pixels` does, minus the swizzle
    /// (which the format choice can remove entirely).
    fn upload(&self, rgba: &[u8]) {
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * WIDTH),
                rows_per_image: Some(HEIGHT),
            },
            wgpu::Extent3d { width: WIDTH, height: HEIGHT, depth_or_array_layers: 1 },
        );
    }
}

fn run_gpu_measurement(args: &Args) {
    let Some(gpu) = Gpu::new() else {
        eprintln!("[sink] no GPU adapter; skipping the upload measurement");
        return;
    };
    let frame = vec![0x7Fu8; (WIDTH * HEIGHT * 4) as usize];
    let iters = args.iters;

    // Warm everything up: the first submit on a fresh device pays for
    // allocation, not for the copy.
    for _ in 0..20 {
        gpu.upload(&frame);
        gpu.queue.submit([]);
        let _ = gpu.device.poll(wgpu::Maintain::Wait);
    }

    // (1) The call on its own. This is CPU time on the game's main thread:
    // wgpu copies the bytes into its staging belt here.
    let t = Instant::now();
    for _ in 0..iters {
        gpu.upload(&frame);
    }
    let call_only_ns = t.elapsed().as_nanos() as u64;
    // Drain what those calls queued, so it is not charged to the next figure.
    gpu.queue.submit([]);
    let _ = gpu.device.poll(wgpu::Maintain::Wait);

    // (2) Baseline: submit an empty command buffer and wait for the GPU.
    let t = Instant::now();
    for _ in 0..iters {
        let enc = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("empty") });
        gpu.queue.submit([enc.finish()]);
        let _ = gpu.device.poll(wgpu::Maintain::Wait);
    }
    let baseline_ns = t.elapsed().as_nanos() as u64;

    // (3) The same, with one frame uploaded each time. The difference is the
    // wall-clock a frame really pays for one 1280x720 upload.
    let t = Instant::now();
    for _ in 0..iters {
        gpu.upload(&frame);
        let enc = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("with upload") });
        gpu.queue.submit([enc.finish()]);
        let _ = gpu.device.poll(wgpu::Maintain::Wait);
    }
    let with_upload_ns = t.elapsed().as_nanos() as u64;

    let per_call_ms = call_only_ns as f64 / iters as f64 / 1e6;
    let baseline_ms = baseline_ns as f64 / iters as f64 / 1e6;
    let with_ms = with_upload_ns as f64 / iters as f64 / 1e6;
    let added_ms = with_ms - baseline_ms;

    // (0) A CONTROL. Without it there is no way to tell "wgpu's staging memory
    // is slow to write to" from "this machine's memory is slow", and the
    // difference decides whether the cost is avoidable.
    let frame_bytes = (WIDTH * HEIGHT * 4) as usize;
    let mut dst = vec![0u8; frame_bytes];
    let t = Instant::now();
    for _ in 0..iters {
        dst.copy_from_slice(&frame);
    }
    let memcpy_ms = t.elapsed().as_nanos() as f64 / iters as f64 / 1e6;
    std::hint::black_box(&dst);

    println!();
    println!("================ GPU UPLOAD (1280x720 RGBA, {:.2} MB per frame) ================",
        (WIDTH * HEIGHT * 4) as f64 / 1048576.0);
    println!(
        "plain memcpy (control) {memcpy_ms:.4} ms   ({:.1} GB/s, ordinary RAM to RAM)",
        frame_bytes as f64 / (memcpy_ms * 1e6)
    );
    println!(
        "write_texture call     {per_call_ms:.4} ms   (CPU, the staging copy, {:.1} GB/s)",
        frame_bytes as f64 / (per_call_ms * 1e6)
    );
    println!("submit + wait, empty   {baseline_ms:.4} ms   (baseline overhead)");
    println!("submit + wait, +1 frame {with_ms:.4} ms");
    println!("=> added per upload    {added_ms:.4} ms   (wall clock WITH a full GPU sync every");
    println!("                                          iteration, which no real frame does)");

    // (4) The GPU's own view, where the adapter allows timing inside an
    // encoder: the buffer-to-texture copy alone, in the same units the
    // engine's Performance page reports gpu.screen_ui in.
    if let Some((qs, resolve, read, period_ns)) = &gpu.timestamps {
        let mut samples = Vec::new();
        for _ in 0..args.gpu_timed_iters {
            let mut enc = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("timed copy") });
            enc.write_timestamp(qs, 0);
            enc.copy_buffer_to_texture(
                wgpu::TexelCopyBufferInfo {
                    buffer: &gpu.staging,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * WIDTH),
                        rows_per_image: Some(HEIGHT),
                    },
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &gpu.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d { width: WIDTH, height: HEIGHT, depth_or_array_layers: 1 },
            );
            enc.write_timestamp(qs, 1);
            enc.resolve_query_set(qs, 0..2, resolve, 0);
            enc.copy_buffer_to_buffer(resolve, 0, read, 0, 16);
            gpu.queue.submit([enc.finish()]);

            read.slice(..).map_async(wgpu::MapMode::Read, |_| {});
            let _ = gpu.device.poll(wgpu::Maintain::Wait);
            {
                let view = read.slice(..).get_mapped_range();
                let tick = |i: usize| -> u64 {
                    let o = i * 8;
                    u64::from_le_bytes(view[o..o + 8].try_into().unwrap())
                };
                let (a, b) = (tick(0), tick(1));
                if b > a {
                    samples.push((b - a) as f64 * *period_ns as f64 / 1e6);
                }
            }
            read.unmap();
        }
        let (p50, p95, _p99, worst) = percentiles(&samples);
        println!(
            "GPU copy, timestamped  p50 {p50:.4} ms  p95 {p95:.4}  worst {worst:.4}   ({} samples)",
            samples.len()
        );
    }

    // (5) The shape a real frame has: upload every screen, submit ONCE, and
    // never block waiting for the GPU. This is what the engine's loop does,
    // and it is the number the frame budget should be judged against.
    println!();
    println!("---- as a frame loop would do it (one submit per frame, no GPU sync) ----");
    let mut zero_screen_ms = 0.0;
    for screens in [0usize, 1, 2, 3] {
        let frames = 300;
        // Warm up so the staging belt has settled into a steady state.
        for _ in 0..30 {
            for _ in 0..screens {
                gpu.upload(&frame);
            }
            gpu.queue.submit([]);
            let _ = gpu.device.poll(wgpu::Maintain::Poll);
        }
        let t = Instant::now();
        for _ in 0..frames {
            for _ in 0..screens {
                gpu.upload(&frame);
            }
            let enc = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
            gpu.queue.submit([enc.finish()]);
            let _ = gpu.device.poll(wgpu::Maintain::Poll);
        }
        let per_frame_ms = t.elapsed().as_nanos() as f64 / frames as f64 / 1e6;
        if screens == 0 {
            zero_screen_ms = per_frame_ms;
        }
        println!(
            "{screens} browser screen(s)     {per_frame_ms:.4} ms per frame   (+{:.4} ms over none)",
            per_frame_ms - zero_screen_ms
        );
        let _ = gpu.device.poll(wgpu::Maintain::Wait);
    }

    // (6) Is the staging copy avoidable? Writing the frame straight into a
    // buffer the GPU can already see, then letting the GPU do the copy, cuts
    // out wgpu's own staging step. If this is much faster than (1), the
    // 1280x720 upload has a cheaper shape available to it.
    let mapped = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("persistently mapped staging"),
        size: frame_bytes as u64,
        usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: true,
    });
    {
        // Time only the write into mapped memory; unmapping and re-mapping is
        // a separate, known cost and is reported as part of the loop below.
        let t = Instant::now();
        for _ in 0..iters {
            let mut view = mapped.slice(..).get_mapped_range_mut();
            view.copy_from_slice(&frame);
        }
        let ms = t.elapsed().as_nanos() as f64 / iters as f64 / 1e6;
        println!();
        println!(
            "write into mapped GPU buffer  {ms:.4} ms   ({:.1} GB/s) - the alternative to write_texture",
            frame_bytes as f64 / (ms * 1e6)
        );
    }
    mapped.unmap();
}

// ---------------------------------------------------------------------------

fn ms_per(total_ns: u64, n: u64) -> f64 {
    if n == 0 { 0.0 } else { total_ns as f64 / n as f64 / 1e6 }
}

fn gbps(total_ns: u64, n: u64, bytes: usize) -> f64 {
    if total_ns == 0 { 0.0 } else { (n as f64 * bytes as f64) / (total_ns as f64) }
}

fn percentiles(v: &[f64]) -> (f64, f64, f64, f64) {
    if v.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let at = |f: f64| s[((s.len() as f64 - 1.0) * f).round() as usize];
    (at(0.50), at(0.95), at(0.99), *s.last().unwrap())
}

struct Args {
    shm: String,
    seconds: u64,
    gpu: bool,
    bench_only: bool,
    iters: u64,
    gpu_timed_iters: u64,
}

impl Args {
    fn parse() -> Self {
        let mut map: HashMap<String, String> = HashMap::new();
        for a in std::env::args().skip(1) {
            if let Some(rest) = a.strip_prefix("--") {
                let (k, v) = rest.split_once('=').unwrap_or((rest, "1"));
                map.insert(k.to_string(), v.to_string());
            }
        }
        let get = |k: &str, d: &str| map.get(k).cloned().unwrap_or_else(|| d.to_string());
        Self {
            shm: get("shm", "humanity_cef_probe"),
            seconds: get("seconds", "30").parse().unwrap_or(30),
            gpu: get("gpu", "1") != "0",
            bench_only: get("bench-only", "0") != "0",
            iters: get("iters", "500").parse().unwrap_or(500),
            gpu_timed_iters: get("gpu-timed-iters", "200").parse().unwrap_or(200),
        }
    }
}
