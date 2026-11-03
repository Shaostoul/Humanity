// Child of profile_copy.rs (#[path], so `use super::*` reaches everything it has): the shared
// fixture both apps read (BUG-191), moved out of profile_copy_tests.rs for the relay's tests that
// read it (relay/crossing_tests/profile_copy.rs). Behind the `test-support` feature, since a crate's
// #[cfg(test)] items never reach another crate's tests (the crate split, rung 1).

use super::*;

// ── The shared fixture ──

/// The fixture both apps read.
pub const FIXTURE: &str = "scripts/tests/fixtures/profile-copy-v1.json";
/// The signer's seed byte: the 32-byte seed `[7; 32]`, the key every Rust fixture uses.
pub const FX_SIGNER: u8 = 7;
/// The device's clock in every case, in milliseconds.
pub const FX_NOW_MS: u64 = 1_760_000_000_000;

/// One case: the person's stored profile and privacy map, and the time of the copy the server
/// holds for the key (`signed_at`, 0 for none).
pub struct FxCase {
    pub label: &'static str,
    pub bio: &'static str,
    pub avatar_url: &'static str,
    pub banner_url: &'static str,
    pub socials: &'static str,
    pub pronouns: &'static str,
    pub location: &'static str,
    pub website: &'static str,
    pub privacy: &'static str,
    pub signed_at: u64,
}

/// Since BUG-241 (2026-11-03) only `"directory": "listed"` lists a person, so the listed cases say
/// it, and a map without it (case 4, as every profile saved before the fix is) is the withdrawal.
pub fn fx_cases() -> [FxCase; 5] {
    [
        FxCase {
            label: "listed; location kept for friends; pronouns marked public; a Greek bio with a line break; links in an order a re-serializer would change; the copy held is dated ahead of the clock",
            bio: "Γεια σας!\nΖω στη Λισαβόνα.",
            avatar_url: "https://ann.example/a.png",
            banner_url: "",
            socials: r#"{"github":"ann","discord":"ann#1"}"#,
            pronouns: "she/her",
            location: "Lisbon",
            website: "https://ann.example",
            privacy: r#"{"directory":"listed","location":"private","pronouns":"public"}"#,
            signed_at: FX_NOW_MS + 90_000,
        },
        FxCase {
            label: "unlisted: the withdrawal",
            bio: "not for other servers",
            avatar_url: "https://ann.example/a.png",
            banner_url: "https://ann.example/b.png",
            socials: r#"{"github":"ann"}"#,
            pronouns: "she/her",
            location: "Lisbon",
            website: "",
            privacy: r#"{"directory":"unlisted"}"#,
            signed_at: FX_NOW_MS - 5_000,
        },
        FxCase {
            label: "listed; links stored blank (read as {}); no copy held",
            bio: "",
            avatar_url: "",
            banner_url: "",
            socials: "",
            pronouns: "they/them",
            location: "Porto",
            website: "",
            privacy: r#"{"directory":"listed"}"#,
            signed_at: 0,
        },
        FxCase {
            label: "a map that is not an object: nothing kept for friends and nobody listed, the withdrawal; the copy held has the clock's own time",
            bio: "hello",
            avatar_url: "",
            banner_url: "",
            socials: "{}",
            pronouns: "she/her",
            location: "Lisbon",
            website: "https://ann.example",
            privacy: "[]",
            signed_at: FX_NOW_MS,
        },
        FxCase {
            label: "never chose (map {}, as every profile saved before BUG-241 is): unlisted, the withdrawal",
            bio: "listed by nobody's choice",
            avatar_url: "https://ann.example/a.png",
            banner_url: "",
            socials: r#"{"github":"ann"}"#,
            pronouns: "she/her",
            location: "Lisbon",
            website: "https://ann.example",
            privacy: "{}",
            signed_at: FX_NOW_MS - 1_000,
        },
    ]
}

/// The fixture's `next_copy_time` rows: (now_ms, signed_at, last_here, at).
pub const FX_TIMES: [(u64, u64, u64, u64); 7] = [
    (1_000, 0, 0, 1_000),
    (1_000, 5_000, 0, 5_001),
    (1_000, 0, 3_000, 3_001),
    (1_000, 5_000, 7_000, 7_001),
    (9_000, 5_000, 7_000, 9_000),
    (1_000, 1_000, 0, 1_001),
    (FX_NOW_MS, FX_NOW_MS + 90_000, FX_NOW_MS + 90_001, FX_NOW_MS + 90_002),
];

pub fn fx_key() -> String {
    hex::encode(DilithiumKeypair::from_seed(&derive_dilithium_seed(&[FX_SIGNER; 32])).public_key())
}

/// What a device signs from an own-copy frame: `public_copy` of the frame's name and fields
/// exactly (an absent field is blank) under its privacy map.
pub fn copy_of_frame(frame: &Value) -> PublicCopy {
    let s = |k: &str| frame[k].as_str().unwrap_or("").to_string();
    public_copy(
        &s("name"), &s("bio"), &s("avatar_url"), &s("banner_url"), &s("socials"),
        &s("pronouns"), &s("location"), &s("website"), &s("privacy"),
    )
}
