// Child of stun_wire.rs (#[path], so `use super::*` reaches everything it has): the STUN and
// TURN codec against the published test vectors of RFC 5769 ("Test Vectors for Session
// Traversal Utilities for NAT (STUN)", April 2010, read from rfc-editor.org on 2026-10-09). Each
// vector carries a MESSAGE-INTEGRITY and most a FINGERPRINT, the two values browsers check on
// what a TURN server sends them.

use super::*;

/// Hex with spaces and line breaks, as the RFC prints it, to bytes.
fn hex(s: &str) -> Vec<u8> {
    let digits: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    (0..digits.len()).step_by(2).map(|i| u8::from_str_radix(&digits[i..i + 2], 16).unwrap()).collect()
}

/// RFC 5769 section 2.1, "Sample Request": a Binding request with SOFTWARE, PRIORITY,
/// ICE-CONTROLLED, USERNAME, MESSAGE-INTEGRITY (short-term password) and FINGERPRINT.
const SAMPLE_REQUEST: &str = "
    00 01 00 58 21 12 a4 42 b7 e7 a7 01 bc 34 d6 86 fa 87 df ae
    80 22 00 10 53 54 55 4e 20 74 65 73 74 20 63 6c 69 65 6e 74
    00 24 00 04 6e 00 01 ff
    80 29 00 08 93 2f f9 b1 51 26 3b 36
    00 06 00 09 65 76 74 6a 3a 68 36 76 59 20 20 20
    00 08 00 14 9a ea a7 0c bf d8 cb 56 78 1e f2 b5 b2 d3 f2 49 c1 b5 71 a2
    80 28 00 04 e5 7a 3b cf";

/// RFC 5769 section 2.2, "Sample IPv4 Response": XOR-MAPPED-ADDRESS 192.0.2.1:32853.
const SAMPLE_IPV4_RESPONSE: &str = "
    01 01 00 3c 21 12 a4 42 b7 e7 a7 01 bc 34 d6 86 fa 87 df ae
    80 22 00 0b 74 65 73 74 20 76 65 63 74 6f 72 20
    00 20 00 08 00 01 a1 47 e1 12 a6 43
    00 08 00 14 2b 91 f5 99 fd 9e 90 c3 8c 74 89 f9 2a f9 ba 53 f0 6b e7 d7
    80 28 00 04 c0 7d 4c 96";

/// RFC 5769 section 2.3, "Sample IPv6 Response": XOR-MAPPED-ADDRESS
/// [2001:db8:1234:5678:11:2233:4455:6677]:32853.
const SAMPLE_IPV6_RESPONSE: &str = "
    01 01 00 48 21 12 a4 42 b7 e7 a7 01 bc 34 d6 86 fa 87 df ae
    80 22 00 0b 74 65 73 74 20 76 65 63 74 6f 72 20
    00 20 00 14 00 02 a1 47 01 13 a9 fa a5 d3 f1 79 bc 25 f4 b5 be d2 b9 d9
    00 08 00 14 a3 82 95 4e 4b e6 7b f1 17 84 c9 7c 82 92 c2 75 bf e3 ed 41
    80 28 00 04 c8 fb 0b 4c";

/// RFC 5769 section 2.4, "Sample Request with Long-Term Authentication": USERNAME, NONCE,
/// REALM and MESSAGE-INTEGRITY under MD5(username:realm:password), no FINGERPRINT.
const SAMPLE_LONG_TERM_REQUEST: &str = "
    00 01 00 60 21 12 a4 42 78 ad 34 33 c6 ad 72 c0 29 da 41 2e
    00 06 00 12 e3 83 9e e3 83 88 e3 83 aa e3 83 83 e3 82 af e3 82 b9 00 00
    00 15 00 1c 66 2f 2f 34 39 39 6b 39 35 34 64 36 4f 4c 33 34 6f 4c 39 46 53 54 76 79 36 34 73 41
    00 14 00 0b 65 78 61 6d 70 6c 65 2e 6f 72 67 00
    00 08 00 14 f6 70 24 65 6d d6 4a 3e 02 b8 e0 71 2e 85 c9 a2 8c a8 96 66";

/// The section 2.1 sample request as bytes (the forwarder tests answer it).
pub(crate) fn rfc5769_sample_request() -> Vec<u8> {
    hex(SAMPLE_REQUEST)
}

/// The short-term password of the first three vectors.
const SHORT_TERM_PASSWORD: &str = "VOkJxbRl1RmTxUk/WvJxBt";

/// The CRC-32 check value every CRC-32 (ISO-HDLC) implementation publishes.
///
/// Seen red 2026-10-09 with the polynomial changed to 0xEDB88321: "left: 3485321504, right:
/// 3421780262" here, and every FINGERPRINT check below failed with it.
#[test]
fn crc32_matches_the_published_check_value() {
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
}

/// Section 2.1: the request parses, its FINGERPRINT and MESSAGE-INTEGRITY check, and a wrong
/// password does not.
///
/// Seen red 2026-10-09 with `integrity_ok` setting the length field to the whole message
/// (FINGERPRINT included) instead of ending it at MESSAGE-INTEGRITY: "the RFC 5769 2.1 integrity
/// must check" (the 2.2 and 2.3 responses below failed the same way; 2.4, with no FINGERPRINT
/// after its integrity, still passed, which is why the vectors with one matter).
#[test]
fn rfc5769_sample_request_checks() {
    let bytes = hex(SAMPLE_REQUEST);
    assert_eq!(bytes.len(), 108);
    let m = Message::parse(&bytes).expect("the sample request parses");
    assert_eq!((m.method, m.class), (METHOD_BINDING, Class::Request));
    assert_eq!(m.txid, [0xb7, 0xe7, 0xa7, 0x01, 0xbc, 0x34, 0xd6, 0x86, 0xfa, 0x87, 0xdf, 0xae]);
    assert_eq!(m.get(ATTR_USERNAME), Some(&b"evtj:h6vY"[..]));
    assert_eq!(m.get(ATTR_SOFTWARE), Some(&b"STUN test client"[..]));
    assert_eq!(m.fingerprint_ok(), Some(true), "the RFC 5769 2.1 fingerprint must check");
    assert!(m.integrity_ok(SHORT_TERM_PASSWORD.as_bytes()), "the RFC 5769 2.1 integrity must check");
    assert!(!m.integrity_ok(b"not the password"), "a wrong password must not check");
}

/// Sections 2.2 and 2.3: both responses parse, check, and decode to the documented addresses.
#[test]
fn rfc5769_sample_responses_check_and_decode() {
    for (vector, want) in [
        (SAMPLE_IPV4_RESPONSE, "192.0.2.1:32853"),
        (SAMPLE_IPV6_RESPONSE, "[2001:db8:1234:5678:11:2233:4455:6677]:32853"),
    ] {
        let bytes = hex(vector);
        let m = Message::parse(&bytes).expect("the sample response parses");
        assert_eq!((m.method, m.class), (METHOD_BINDING, Class::Success));
        assert_eq!(m.fingerprint_ok(), Some(true), "{want}: fingerprint");
        assert!(m.integrity_ok(SHORT_TERM_PASSWORD.as_bytes()), "{want}: integrity");
        let mapped = decode_xor_address(m.get(ATTR_XOR_MAPPED_ADDRESS).unwrap(), &m.txid).unwrap();
        assert_eq!(mapped, want.parse::<SocketAddr>().unwrap());
    }
}

/// Section 2.4: the long-term key, and the request's integrity under it.
#[test]
fn rfc5769_long_term_request_checks() {
    let bytes = hex(SAMPLE_LONG_TERM_REQUEST);
    let m = Message::parse(&bytes).expect("the long-term sample parses");
    let username = std::str::from_utf8(m.get(ATTR_USERNAME).unwrap()).unwrap();
    assert_eq!(username, "\u{30DE}\u{30C8}\u{30EA}\u{30C3}\u{30AF}\u{30B9}");
    assert_eq!(m.get(ATTR_REALM), Some(&b"example.org"[..]));
    assert_eq!(m.get(ATTR_NONCE), Some(&b"f//499k954d6OL34oL9FSTvy64sA"[..]));
    assert_eq!(m.fingerprint_ok(), None, "this vector has no fingerprint");
    let key = long_term_key(username, "example.org", "TheMatrIX");
    assert!(m.integrity_ok(&key), "the RFC 5769 2.4 integrity must check under MD5(user:realm:pass)");
    assert!(!m.integrity_ok(&long_term_key(username, "example.org", "TheMatrix")));
}

/// Our builder rebuilds the section 2.2 and 2.3 responses byte for byte: same attributes, same
/// MESSAGE-INTEGRITY, same FINGERPRINT. That pins the builder's length handling for both, and
/// the IPv6 XOR (which mixes in the transaction id). The RFC pads SOFTWARE with a space where the
/// builder writes a zero (padding may be any value), so the test puts the space in first.
///
/// Seen red 2026-10-09 with `fingerprint` leaving the length field as it was (no `set_len(8)`):
/// "192.0.2.1:32853: the builder must write the RFC's bytes".
#[test]
fn the_builder_reproduces_the_rfc5769_responses() {
    for (vector, mapped) in [
        (SAMPLE_IPV4_RESPONSE, "192.0.2.1:32853"),
        (SAMPLE_IPV6_RESPONSE, "[2001:db8:1234:5678:11:2233:4455:6677]:32853"),
    ] {
        let want = hex(vector);
        let txid: [u8; 12] = want[8..20].try_into().unwrap();
        let mut b = MessageBuilder::new(METHOD_BINDING, Class::Success, txid);
        b.attr(ATTR_SOFTWARE, b"test vector");
        b.buf[HEADER_LEN + 4 + 11] = 0x20;
        let got = b
            .xor_address(ATTR_XOR_MAPPED_ADDRESS, mapped.parse().unwrap())
            .integrity(SHORT_TERM_PASSWORD.as_bytes())
            .fingerprint()
            .build();
        assert_eq!(got, want, "{mapped}: the builder must write the RFC's bytes");
    }
}

/// The message type interleaving against the values RFC 5389 and RFC 5766 give, both ways.
#[test]
fn message_types_round_trip() {
    for (method, class, t) in [
        (METHOD_BINDING, Class::Request, 0x0001),
        (METHOD_BINDING, Class::Success, 0x0101),
        (METHOD_BINDING, Class::Error, 0x0111),
        (METHOD_ALLOCATE, Class::Request, 0x0003),
        (METHOD_ALLOCATE, Class::Success, 0x0103),
        (METHOD_ALLOCATE, Class::Error, 0x0113),
        (METHOD_REFRESH, Class::Request, 0x0004),
        (METHOD_SEND, Class::Indication, 0x0016),
        (METHOD_DATA, Class::Indication, 0x0017),
        (METHOD_CREATE_PERMISSION, Class::Request, 0x0008),
        (METHOD_CHANNEL_BIND, Class::Request, 0x0009),
        (METHOD_CHANNEL_BIND, Class::Success, 0x0109),
    ] {
        assert_eq!(message_type(method, class), t, "{method:#x} {class:?}");
        assert_eq!(split_type(t), (method, class), "{t:#06x}");
    }
}

/// What is not a STUN message is not parsed as one: ChannelData, a wrong cookie, a length that
/// runs past the datagram, an attribute that runs past the message.
#[test]
fn junk_is_not_a_message() {
    assert!(Message::parse(&[0u8; 19]).is_none());
    assert!(Message::parse(&channel_data(0x4001, b"hello")).is_none());
    let mut bytes = hex(SAMPLE_REQUEST);
    bytes[4] ^= 1;
    assert!(Message::parse(&bytes).is_none(), "wrong cookie");
    let mut bytes = hex(SAMPLE_REQUEST);
    bytes[3] = 0x5c;
    assert!(Message::parse(&bytes).is_none(), "length past the datagram");
    let mut bytes = hex(SAMPLE_REQUEST);
    bytes[23] = 0x40;
    assert!(Message::parse(&bytes).is_none(), "an attribute past the message");
}

/// A changed byte breaks the FINGERPRINT; ChannelData frames read back.
#[test]
fn a_changed_byte_breaks_the_fingerprint_and_channel_data_reads_back() {
    let mut bytes = hex(SAMPLE_REQUEST);
    bytes[30] ^= 0x01;
    assert_eq!(Message::parse(&bytes).unwrap().fingerprint_ok(), Some(false));
    let f = channel_data(0x4abc, b"voice");
    assert_eq!(parse_channel_data(&f), Some((0x4abc, &b"voice"[..])));
    let mut padded = f.clone();
    padded.extend_from_slice(&[0, 0, 0]);
    assert_eq!(parse_channel_data(&padded), Some((0x4abc, &b"voice"[..])), "UDP padding is allowed");
    assert_eq!(parse_channel_data(&f[..6]), None, "a length past the datagram is not");
    assert_eq!(parse_channel_data(&channel_data(0x3fff, b"x")), None, "not a channel number");
}

/// XOR addresses round-trip for IPv4, IPv6, and an IPv4 address a dual-stack socket reports as
/// IPv6 (written back as the IPv4 address it is).
#[test]
fn xor_addresses_round_trip() {
    let txid = [7u8; 12];
    for a in ["203.0.113.45:51234", "[2001:db8::1]:443"] {
        let addr: SocketAddr = a.parse().unwrap();
        assert_eq!(decode_xor_address(&encode_xor_address(addr, &txid), &txid), Some(addr));
    }
    let mapped: SocketAddr = "[::ffff:192.0.2.7]:3478".parse().unwrap();
    assert_eq!(
        decode_xor_address(&encode_xor_address(mapped, &txid), &txid),
        Some("192.0.2.7:3478".parse().unwrap())
    );
    assert_eq!(decode_error_code(&[0, 0, 4, 38]), Some(438));
}
