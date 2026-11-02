//! Who may build where, as the wire says it: a household permit (`Permit`, a plot holder's
//! signed leave for one person to build on their plot, on one server, until a date), the
//! longest one may run (`PERMIT_MAX_DAYS`), and the codes a refused build names (`Why`).
//!
//! WHY IT LIVES IN THE CORE (the crate split's rung 0, 2026-10-31,
//! docs/design/crate-split.md): these were `systems::construction::shared`'s, the build
//! contract both the game and the relay compile; the permit book on the app's network side
//! (`net::household`: the permits a player holds and gives, and the relay's verdict on each)
//! speaks them too, and the network side sits beside the simulation, not on it. Plain serde
//! data, std only. What a refusal SAYS (`why_words`, `refusal_message`) stays with the contract;
//! the protocol's `verify_plot_permit` checks a permit's signature.

use serde::{Deserialize, Serialize};

/// The longest a household permit may run, days (the operator, 2026-10-05: every permit has an
/// end date, renewable; no endless permits). The relay enforces it in
/// `relay::core::pq_crypto::verify_plot_permit` (its `PLOT_PERMIT_MAX_DAYS`, which must equal
/// this: the pq_crypto test pins them together), and [`Why::PermitTooLong`]'s words name it.
pub const PERMIT_MAX_DAYS: u64 = 90;

/// A household permit (ship homes section 4): a plot's holder lets one person build on that
/// plot, on one server, until a date. Held by the grantee and sent with each build or take-down
/// they make there; the relay checks it without storing it
/// (`relay::core::pq_crypto::verify_plot_permit`), the friendship certificate's pattern. The
/// relay rebuilds the signed words from what it knows (its own server id, the frame's plot, the
/// sender's own id), so `server`, `plot` and `grantee` here only say what the permit claims to be
/// for: the grantee's game files it by them and shows them.
///
/// v2 (ship homes 5b, amendment A1, 2026-10-12): a permit names a `serial` its giver chose, so
/// the giver can take it back before its end date: `cert_revoke {serial}` on their own signed-in
/// socket puts it on the relay's withdrawal list (the one friendship passes use), and the relay
/// refuses it from then on as [`Why::PermitWithdrawn`]. The fields are in the order the words are
/// signed in, which is also the order a permit note's JSON is written in (net/household.rs).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Permit {
    /// The plot holder's Dilithium3 public key, hex: whose signature this is.
    pub issuer: String,
    /// The server it was given on: that server's own `did:hum:` (its /api/server-info). Good on
    /// that server only: one person often holds the same plot id on several servers.
    pub server: String,
    /// The plot it is for, by its ship-file id (`p3`, not `plot:p3`).
    pub plot: String,
    /// Who it lets build: the grantee's id as a relay holds plots under it (`did:hum:...`,
    /// `relay::storage::plots::plot_owner_id`).
    pub grantee: String,
    /// The name its giver chose for it: 16 random bytes in lowercase hex, the friendship pass's
    /// serial shape (`relay::core::pq_crypto::friend_cert_serial_ok`). What a withdrawal names.
    pub serial: String,
    /// When it runs out, Unix seconds; never more than [`PERMIT_MAX_DAYS`] days ahead.
    pub expiry: u64,
    /// The issuer's Dilithium3 signature over
    /// `hum/permit/v2\n{server}\n{plot}\n{grantee}\n{serial}\n{expiry}`, base64.
    pub sig: String,
}

/// Why a request was `not_allowed`: the `why` code that comes with it, exactly as it goes on the
/// wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Why {
    /// Someone else's plot, and no household permit for it.
    NotYourPlot,
    /// A take-down by a permit holder of a piece someone else put up.
    NotYourPiece,
    /// The sender holds no plot on this ship (a guest) and brought no permit.
    Guest,
    /// The household permit has run out.
    PermitExpired,
    /// The permit's signer does not hold the plot (it may have changed hands).
    PermitNotFromHolder,
    /// The permit is malformed or its signature does not check out.
    PermitBad,
    /// The permit runs out more than [`PERMIT_MAX_DAYS`] days ahead.
    PermitTooLong,
    /// The plot's holder took this permit back (ship homes 5b, amendment A1): its serial is on the
    /// relay's withdrawal list, the one friendship passes use (`friend_cert_revocations`).
    PermitWithdrawn,
    /// A shared space (`zone:*`), and the sender lacks the `can_edit_ship` rank.
    ShipRank,
    /// A code this game does not know (a newer server). Never sent.
    #[serde(other)]
    Unknown,
}

impl Why {
    /// Every code the relay sends.
    pub const ALL: [Why; 9] = [
        Why::NotYourPlot,
        Why::NotYourPiece,
        Why::Guest,
        Why::PermitExpired,
        Why::PermitNotFromHolder,
        Why::PermitBad,
        Why::PermitTooLong,
        Why::PermitWithdrawn,
        Why::ShipRank,
    ];

    /// The code as it goes on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            Why::NotYourPlot => "not_your_plot",
            Why::NotYourPiece => "not_your_piece",
            Why::Guest => "guest",
            Why::PermitExpired => "permit_expired",
            Why::PermitNotFromHolder => "permit_not_from_holder",
            Why::PermitBad => "permit_bad",
            Why::PermitTooLong => "permit_too_long",
            Why::PermitWithdrawn => "permit_withdrawn",
            Why::ShipRank => "ship_rank",
            Why::Unknown => "unknown",
        }
    }
}
