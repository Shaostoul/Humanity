//! STUN and TURN messages on the wire (RFC 5389, RFC 5766), for the relay's own address lookup
//! (STUN) and its call forwarder (call_forwarder.rs, docs/design/blocking-and-safe-mode.md 10f).
//!
//! One parser and one builder for both, checked against the published test vectors of RFC 5769
//! (stun_wire_tests.rs). Browsers check MESSAGE-INTEGRITY and FINGERPRINT on what a TURN server
//! sends them, so those two are the parts that must be exactly right.
//!
//! This module is compiled into both the relay and the desktop app (the `native` feature
//! includes `relay`). The desktop app's own STUN and TURN client still frames its messages with
//! the helpers inside `src/net/webrtc.rs` (`mod stun`, `mod turn`); they can move onto this module
//! once the client half of step E has landed, rather than while another session edits that file.
//!
//! # The message layout (RFC 5389 section 6)
//!
//! ```text
//!  0                   1                   2                   3
//!  0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! |0 0|     STUN Message Type     |         Message Length        |
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! |                         Magic Cookie                          |
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! |                     Transaction ID (96 bits)                  |
//! +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
//! ```
//!
//! The message type interleaves a 12-bit method with a 2-bit class (bits 4 and 8). Attributes
//! follow the header as type (2 bytes), length (2 bytes), value, padded to a multiple of four.
//! A ChannelData frame (RFC 5766 section 11.4) starts with a channel number in 0x4000..=0x7FFF,
//! so its first byte is 0x40..=0x7F, where a STUN message's is 0x00..=0x3F.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

/// The fixed magic cookie every RFC 5389 message carries.
pub const MAGIC_COOKIE: u32 = 0x2112_A442;
/// The header: type, length, cookie, transaction id.
pub const HEADER_LEN: usize = 20;

// Methods (RFC 5389 section 18.1, RFC 5766 section 13).
pub const METHOD_BINDING: u16 = 0x001;
pub const METHOD_ALLOCATE: u16 = 0x003;
pub const METHOD_REFRESH: u16 = 0x004;
pub const METHOD_SEND: u16 = 0x006;
pub const METHOD_DATA: u16 = 0x007;
pub const METHOD_CREATE_PERMISSION: u16 = 0x008;
pub const METHOD_CHANNEL_BIND: u16 = 0x009;

// Attributes (RFC 5389 section 18.2, RFC 5766 section 14, RFC 6156 section 4.1.1).
pub const ATTR_MAPPED_ADDRESS: u16 = 0x0001;
pub const ATTR_USERNAME: u16 = 0x0006;
pub const ATTR_MESSAGE_INTEGRITY: u16 = 0x0008;
pub const ATTR_ERROR_CODE: u16 = 0x0009;
pub const ATTR_CHANNEL_NUMBER: u16 = 0x000C;
pub const ATTR_LIFETIME: u16 = 0x000D;
pub const ATTR_XOR_PEER_ADDRESS: u16 = 0x0012;
pub const ATTR_DATA: u16 = 0x0013;
pub const ATTR_REALM: u16 = 0x0014;
pub const ATTR_NONCE: u16 = 0x0015;
pub const ATTR_XOR_RELAYED_ADDRESS: u16 = 0x0016;
pub const ATTR_REQUESTED_ADDRESS_FAMILY: u16 = 0x0017;
pub const ATTR_REQUESTED_TRANSPORT: u16 = 0x0019;
pub const ATTR_XOR_MAPPED_ADDRESS: u16 = 0x0020;
pub const ATTR_SOFTWARE: u16 = 0x8022;
pub const ATTR_FINGERPRINT: u16 = 0x8028;

/// FINGERPRINT is the CRC-32 of the message XORed with this ("STUN" in ASCII).
pub const FINGERPRINT_XOR: u32 = 0x5354_554E;

/// The REQUESTED-TRANSPORT protocol number for UDP.
pub const PROTOCOL_UDP: u8 = 17;

/// The most attributes one message may carry before it is treated as junk. A real one carries
/// fewer than a dozen; the cap keeps a 64 KB datagram of empty attributes from costing work.
const MAX_ATTRS: usize = 64;

/// The two bits beside the method: what kind of message this is.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Class {
    Request,
    Indication,
    Success,
    Error,
}

impl Class {
    fn bits(self) -> u16 {
        match self {
            Class::Request => 0b00,
            Class::Indication => 0b01,
            Class::Success => 0b10,
            Class::Error => 0b11,
        }
    }

    fn from_bits(b: u16) -> Class {
        match b & 0b11 {
            0b00 => Class::Request,
            0b01 => Class::Indication,
            0b10 => Class::Success,
            _ => Class::Error,
        }
    }
}

/// The 14-bit message type for a method and class (RFC 5389 section 6): the class bits land at
/// positions 4 and 8, the method fills the rest. Binding request is 0x0001, Allocate success
/// 0x0103, Allocate error 0x0113, Send indication 0x0016.
pub fn message_type(method: u16, class: Class) -> u16 {
    let c = class.bits();
    ((method & 0x0F80) << 2) | ((c & 0b10) << 7) | ((method & 0x0070) << 1) | ((c & 0b01) << 4) | (method & 0x000F)
}

/// The method and class a message type carries: the inverse of [`message_type`].
pub fn split_type(t: u16) -> (u16, Class) {
    let method = ((t >> 2) & 0x0F80) | ((t >> 1) & 0x0070) | (t & 0x000F);
    let class = ((t >> 7) & 0b10) | ((t >> 4) & 0b01);
    (method, Class::from_bits(class))
}

/// One attribute inside a parsed message: its type, where its TLV starts, and where its value
/// starts and how long it is (all offsets into [`Message::raw`]).
#[derive(Copy, Clone, Debug)]
pub struct Attr {
    pub kind: u16,
    pub tlv: usize,
    pub start: usize,
    pub len: usize,
}

/// A parsed STUN message, borrowing the datagram it came in.
#[derive(Debug)]
pub struct Message<'a> {
    /// Exactly the message: the header plus the length its header states.
    pub raw: &'a [u8],
    pub method: u16,
    pub class: Class,
    pub txid: [u8; 12],
    pub attrs: Vec<Attr>,
}

impl<'a> Message<'a> {
    /// Parse a datagram as a STUN message. `None` for anything that is not one: too short, the
    /// top two bits set (ChannelData or other traffic), the wrong cookie, a length that is not a
    /// multiple of four or runs past the datagram, or an attribute that runs past the message.
    pub fn parse(buf: &'a [u8]) -> Option<Message<'a>> {
        if buf.len() < HEADER_LEN {
            return None;
        }
        let t = u16::from_be_bytes([buf[0], buf[1]]);
        if t & 0xC000 != 0 {
            return None;
        }
        let len = u16::from_be_bytes([buf[2], buf[3]]) as usize;
        if len % 4 != 0 || HEADER_LEN + len > buf.len() {
            return None;
        }
        if u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]) != MAGIC_COOKIE {
            return None;
        }
        let raw = &buf[..HEADER_LEN + len];
        let mut attrs = Vec::new();
        let mut off = HEADER_LEN;
        while off < raw.len() {
            if off + 4 > raw.len() || attrs.len() >= MAX_ATTRS {
                return None;
            }
            let kind = u16::from_be_bytes([raw[off], raw[off + 1]]);
            let alen = u16::from_be_bytes([raw[off + 2], raw[off + 3]]) as usize;
            let start = off + 4;
            if start + alen > raw.len() {
                return None;
            }
            attrs.push(Attr { kind, tlv: off, start, len: alen });
            off = start + ((alen + 3) & !3);
        }
        if off != raw.len() {
            return None;
        }
        let (method, class) = split_type(t);
        let mut txid = [0u8; 12];
        txid.copy_from_slice(&raw[8..20]);
        Some(Message { raw, method, class, txid, attrs })
    }

    /// The attributes a receiver may act on: everything up to and including MESSAGE-INTEGRITY,
    /// then only FINGERPRINT (RFC 5389 section 15.4: anything else after the integrity check is
    /// ignored, because the integrity check does not cover it).
    fn visible(&self) -> impl Iterator<Item = &Attr> {
        let mut after_integrity = false;
        self.attrs.iter().filter(move |a| {
            if after_integrity {
                return a.kind == ATTR_FINGERPRINT;
            }
            if a.kind == ATTR_MESSAGE_INTEGRITY {
                after_integrity = true;
            }
            true
        })
    }

    /// The value of the first visible attribute of `kind`.
    pub fn get(&self, kind: u16) -> Option<&'a [u8]> {
        let raw = self.raw;
        self.visible().find(|a| a.kind == kind).map(|a| &raw[a.start..a.start + a.len])
    }

    /// The values of every visible attribute of `kind` (CreatePermission may carry several
    /// XOR-PEER-ADDRESS attributes).
    pub fn get_all(&self, kind: u16) -> Vec<&'a [u8]> {
        let raw = self.raw;
        self.visible().filter(|a| a.kind == kind).map(|a| &raw[a.start..a.start + a.len]).collect()
    }

    /// Whether the message carries a visible attribute of `kind`.
    pub fn has(&self, kind: u16) -> bool {
        self.visible().any(|a| a.kind == kind)
    }

    /// `None` when the message has no FINGERPRINT; otherwise whether it is correct. A
    /// FINGERPRINT that is not the last attribute is wrong.
    pub fn fingerprint_ok(&self) -> Option<bool> {
        let pos = self.attrs.iter().position(|a| a.kind == ATTR_FINGERPRINT)?;
        if pos != self.attrs.len() - 1 {
            return Some(false);
        }
        let a = self.attrs[pos];
        if a.len != 4 {
            return Some(false);
        }
        let want = crc32(&self.raw[..a.tlv]) ^ FINGERPRINT_XOR;
        let got = u32::from_be_bytes([self.raw[a.start], self.raw[a.start + 1], self.raw[a.start + 2], self.raw[a.start + 3]]);
        Some(want == got)
    }

    /// Whether the message's MESSAGE-INTEGRITY is the HMAC-SHA1 under `key` (RFC 5389 section
    /// 15.4): computed over the message up to the attribute, with the header's length field set as
    /// though the message ended right after it. False when there is none. Constant time.
    pub fn integrity_ok(&self, key: &[u8]) -> bool {
        use hmac::{Hmac, Mac};
        let Some(mi) = self.visible().find(|a| a.kind == ATTR_MESSAGE_INTEGRITY).copied() else {
            return false;
        };
        if mi.len != 20 {
            return false;
        }
        let mut covered = self.raw[..mi.tlv].to_vec();
        let len = (mi.tlv - HEADER_LEN + 24) as u16;
        covered[2..4].copy_from_slice(&len.to_be_bytes());
        let mut mac = Hmac::<sha1::Sha1>::new_from_slice(key).expect("HMAC accepts any key length");
        mac.update(&covered);
        mac.verify_slice(&self.raw[mi.start..mi.start + 20]).is_ok()
    }
}

/// Builds one message. Every method keeps the header's length field equal to what has been
/// appended, so `integrity` and `fingerprint` can be called in that order at the end.
pub struct MessageBuilder {
    buf: Vec<u8>,
    txid: [u8; 12],
}

impl MessageBuilder {
    pub fn new(method: u16, class: Class, txid: [u8; 12]) -> MessageBuilder {
        let mut buf = Vec::with_capacity(96);
        buf.extend_from_slice(&message_type(method, class).to_be_bytes());
        buf.extend_from_slice(&0u16.to_be_bytes());
        buf.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
        buf.extend_from_slice(&txid);
        MessageBuilder { buf, txid }
    }

    /// Set the length field to the attributes appended so far plus `extra` bytes still to come.
    fn set_len(&mut self, extra: usize) {
        let len = (self.buf.len() - HEADER_LEN + extra) as u16;
        self.buf[2..4].copy_from_slice(&len.to_be_bytes());
    }

    /// Append one attribute, padded with zero bytes to a multiple of four.
    pub fn attr(&mut self, kind: u16, value: &[u8]) -> &mut MessageBuilder {
        self.buf.extend_from_slice(&kind.to_be_bytes());
        self.buf.extend_from_slice(&(value.len() as u16).to_be_bytes());
        self.buf.extend_from_slice(value);
        let pad = (4 - value.len() % 4) % 4;
        self.buf.extend(std::iter::repeat(0u8).take(pad));
        self.set_len(0);
        self
    }

    /// Append a four-byte attribute (LIFETIME, REQUESTED-TRANSPORT).
    pub fn u32_attr(&mut self, kind: u16, v: u32) -> &mut MessageBuilder {
        self.attr(kind, &v.to_be_bytes())
    }

    /// Append an XOR-encoded address attribute (XOR-MAPPED-ADDRESS, XOR-PEER-ADDRESS,
    /// XOR-RELAYED-ADDRESS) for `addr`.
    pub fn xor_address(&mut self, kind: u16, addr: SocketAddr) -> &mut MessageBuilder {
        let value = encode_xor_address(addr, &self.txid);
        self.attr(kind, &value)
    }

    /// Append ERROR-CODE: two zero bytes, the hundreds digit, the rest, then the reason phrase.
    pub fn error_code(&mut self, code: u16, reason: &str) -> &mut MessageBuilder {
        let mut v = vec![0u8, 0u8, (code / 100) as u8 & 0x07, (code % 100) as u8];
        v.extend_from_slice(reason.as_bytes());
        self.attr(ATTR_ERROR_CODE, &v)
    }

    /// Append MESSAGE-INTEGRITY: HMAC-SHA1 under `key` over everything so far, with the length
    /// field already counting the 24 bytes this attribute adds (RFC 5389 section 15.4).
    pub fn integrity(&mut self, key: &[u8]) -> &mut MessageBuilder {
        use hmac::{Hmac, Mac};
        self.set_len(24);
        let mut mac = Hmac::<sha1::Sha1>::new_from_slice(key).expect("HMAC accepts any key length");
        mac.update(&self.buf);
        let tag = mac.finalize().into_bytes();
        self.attr(ATTR_MESSAGE_INTEGRITY, &tag)
    }

    /// Append FINGERPRINT: the CRC-32 of everything so far XOR 0x5354554E, with the length field
    /// already counting its 8 bytes (RFC 5389 section 15.5). Always the last attribute.
    pub fn fingerprint(&mut self) -> &mut MessageBuilder {
        self.set_len(8);
        let crc = crc32(&self.buf) ^ FINGERPRINT_XOR;
        self.attr(ATTR_FINGERPRINT, &crc.to_be_bytes())
    }

    pub fn build(&mut self) -> Vec<u8> {
        self.set_len(0);
        std::mem::take(&mut self.buf)
    }
}

/// An IPv4 address written as an IPv6 one (`::ffff:a.b.c.d`, what a dual-stack socket reports)
/// as the IPv4 address it is.
pub fn canonical(addr: SocketAddr) -> SocketAddr {
    match addr {
        SocketAddr::V6(v6) => match v6.ip().to_ipv4_mapped() {
            Some(v4) => SocketAddr::new(IpAddr::V4(v4), v6.port()),
            None => addr,
        },
        v4 => v4,
    }
}

/// The value of an XOR address attribute for `addr` (RFC 5389 section 15.2): a reserved byte,
/// the family, the port XOR the cookie's top half, the address XOR the cookie (and for IPv6 the
/// cookie followed by the transaction id).
pub fn encode_xor_address(addr: SocketAddr, txid: &[u8; 12]) -> Vec<u8> {
    let addr = canonical(addr);
    let x_port = addr.port() ^ (MAGIC_COOKIE >> 16) as u16;
    match addr.ip() {
        IpAddr::V4(ip) => {
            let mut v = vec![0u8, 0x01];
            v.extend_from_slice(&x_port.to_be_bytes());
            v.extend_from_slice(&(u32::from(ip) ^ MAGIC_COOKIE).to_be_bytes());
            v
        }
        IpAddr::V6(ip) => {
            let mut v = vec![0u8, 0x02];
            v.extend_from_slice(&x_port.to_be_bytes());
            let mask = xor_mask_v6(txid);
            v.extend(ip.octets().iter().zip(mask.iter()).map(|(a, m)| a ^ m));
            v
        }
    }
}

/// Decode an XOR address attribute's value; `None` when it is malformed.
pub fn decode_xor_address(value: &[u8], txid: &[u8; 12]) -> Option<SocketAddr> {
    if value.len() < 4 {
        return None;
    }
    let port = u16::from_be_bytes([value[2], value[3]]) ^ (MAGIC_COOKIE >> 16) as u16;
    match value[1] {
        0x01 if value.len() == 8 => {
            let x = u32::from_be_bytes([value[4], value[5], value[6], value[7]]);
            Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::from(x ^ MAGIC_COOKIE)), port))
        }
        0x02 if value.len() == 20 => {
            let mask = xor_mask_v6(txid);
            let mut o = [0u8; 16];
            for i in 0..16 {
                o[i] = value[4 + i] ^ mask[i];
            }
            Some(SocketAddr::new(IpAddr::V6(Ipv6Addr::from(o)), port))
        }
        _ => None,
    }
}

/// The 16 bytes an IPv6 address is XORed with: the cookie, then the transaction id.
fn xor_mask_v6(txid: &[u8; 12]) -> [u8; 16] {
    let mut m = [0u8; 16];
    m[..4].copy_from_slice(&MAGIC_COOKIE.to_be_bytes());
    m[4..].copy_from_slice(txid);
    m
}

/// The code in an ERROR-CODE value (hundreds digit times 100 plus the number).
pub fn decode_error_code(value: &[u8]) -> Option<u16> {
    (value.len() >= 4).then(|| (value[2] & 0x07) as u16 * 100 + value[3] as u16)
}

/// A ChannelData frame (RFC 5766 section 11.4): channel, length, data. Over UDP no padding.
pub fn channel_data(channel: u16, data: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(4 + data.len());
    v.extend_from_slice(&channel.to_be_bytes());
    v.extend_from_slice(&(data.len() as u16).to_be_bytes());
    v.extend_from_slice(data);
    v
}

/// Read a ChannelData frame: its channel (0x4000..=0x7FFF) and its data. Padding after the data
/// is allowed (a client may pad over UDP); a length past the datagram is not.
pub fn parse_channel_data(buf: &[u8]) -> Option<(u16, &[u8])> {
    if buf.len() < 4 {
        return None;
    }
    let channel = u16::from_be_bytes([buf[0], buf[1]]);
    if !(0x4000..=0x7FFF).contains(&channel) {
        return None;
    }
    let len = u16::from_be_bytes([buf[2], buf[3]]) as usize;
    (4 + len <= buf.len()).then(|| (channel, &buf[4..4 + len]))
}

/// The long-term credential key, MD5(username ":" realm ":" password) (RFC 5389 section 15.4).
/// The usernames and passwords this relay issues are ASCII, so SASLprep changes nothing.
pub fn long_term_key(username: &str, realm: &str, password: &str) -> [u8; 16] {
    use md5::{Digest, Md5};
    let mut h = Md5::new();
    h.update(username.as_bytes());
    h.update(b":");
    h.update(realm.as_bytes());
    h.update(b":");
    h.update(password.as_bytes());
    h.finalize().into()
}

/// The CRC-32 table (the ISO-HDLC polynomial, reflected), built at compile time.
const CRC_TABLE: [u32; 256] = {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
};

/// CRC-32 as FINGERPRINT uses it (the same CRC as zlib and Ethernet).
pub fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c = CRC_TABLE[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

#[cfg(test)]
#[path = "stun_wire_tests.rs"]
pub(crate) mod tests;
