# Key cards

**Status: built 2026-10-22** (operator, 2026-10-22: "Let's do this however you believe best").
The card itself: `src/relay/core/key_card.rs` and its website twin `web/shared/key-card.js`, held
to the same bytes by the shared fixture `scripts/tests/fixtures/key-card-v1.json`. The relay:
`src/relay/storage/key_cards.rs` and `src/relay/handlers/key_cards.rs`. The desktop app:
`src/engine/key_cards.rs` and `src/net/key_pins.rs`. The website: `web/chat/chat-key-cards.js`
with the pins in `web/chat/chat-dm-store.js`. The algorithms are in
`docs/reference/cryptography.md` ("Key cards").

## What it fixes, in plain words

To send someone a private message (or a friendship pass, a household permit, a piece of their
recovery phrase, a group's key), an app locks it with that person's Kyber768 (ML-KEM-768) key.
Until now the app learned that key from the server's member list, so a server could hand out a
key of its own for someone, read what was locked to it, and lock it again for the real person.
Nobody would notice.

A key card closes that. It is a small object the person's own app signs with their Dilithium3
(ML-DSA-65) identity key, which IS their public identity: "my Kyber768 key is this". Any app can
check the signature itself, so it no longer matters which server delivered the card, or whether
that server is honest. A server can refuse to hand out a card, but it cannot hand out a false one.

## The card

```
{"v":1,"key":K,"kyber":Y,"at":T,"sig":S}
```

- `key`: the person's Dilithium3 public key, lowercase hex (3,904 characters). Who they are.
- `kyber`: their Kyber768 key, standard base64 with padding (1,580 characters).
- `at`: when it was signed, Unix milliseconds.
- `sig`: the Dilithium3 signature by `key` over the UTF-8 words
  `hum/key-card/v1\n{key}\n{kyber}\n{at}`, standard base64 (4,412 characters).

One spelling only: both base64 fields must be the canonical spelling of their bytes and the key
lowercase, so the signed words cannot be written two ways. A card names no server: it is good on
every server.

The Kyber768 key is derived from the recovery phrase (BLAKE3 `derive_key("hum/kyber768/v1",
seed)`, checked 2026-10-22 in `pq_crypto.rs derive_kyber_seed`, `web/chat/pq.js pqDeriveKyber`), so
today a person's key never changes and every device signs a card with the same key. The card's
time matters when a key does change (a future reset, or a person who starts over).

## Publishing

Each app signs its card when its identity is unlocked (desktop: `apply_pq_identity`, website:
`attachPqIdentity`) and sends it with every sign-in as `identify.key_card`. The relay keeps it once
the sign-in is proven (`identify_response`), and only when it checks out AND names the key that
sign-in just proved. A card that does not is refused, and the person is told; the sign-in goes on.
The bare `kyber_public` the identify used to carry is gone.

What a server keeps (`key_card::keep_rule`, one row per person, `key_cards` table):

| A good card arrives | The server |
|---|---|
| nothing held | keeps it |
| the same key as the one held | changes nothing (the first time stays, so apps do not fetch again just because someone signed in again) |
| a different key, newer | replaces the one held |
| a different key, not newer | refuses it (an old card sent again cannot undo a newer one) |
| dated more than a day ahead of the server's clock | refuses it |

## Serving

- The member lists (`peer_list`, `peer_joined`, `full_user_list`, `/api/peers`,
  `/api/members/{key}`) carry only each person's card TIME, `key_card_at`, never a key. A card is
  6 KB, and the full user list goes to everyone on every change, so the lists got lighter, not
  heavier.
- `{"type":"key_card_request","keys":[...]}` on a signed-in socket, at most 16 keys, is answered to
  the asker alone with `{"type":"key_cards","cards":[...],"missing":[...]}`.
- `GET /api/key-card/{key}` for pages and threads with no socket (the website's Recovery page, the
  desktop app's recovery work).
- `GET /api/v2/groups/{id}/members` carries each member's card inline (`key_card`), since a group
  is small.

## Checking and pinning (both apps)

An app seals to a person ONLY with a key from a card it checked itself: the right shape, signed by
the person's own key, naming that very person. Every seal reads one place: the desktop app's
`GuiState::peer_kyber_keys` (filled only by `engine/key_cards.rs`), the website's
`getPeerEcdhPublic` (answered only by `chat-key-cards.js`). No list's key is ever used.

Pinning, the same rule in both apps (`key_card::compare`, `key-card.js compare`):

| A good card for someone | The app |
|---|---|
| nothing pinned | pins it |
| the pinned key | changes nothing (a newer time is noted) |
| a different key, newer | a CHANGE: nothing is sealed to them until it is accepted |
| a different key, not newer | ignores it |

A change is shown in the conversation with them: "Ann's encryption key changed. This happens when
someone resets their keys. If you did not expect it, check with Ann in person or on a call you
trust before you accept. Until you do, nothing is sent to them." with the button "Accept the new
key". A send meanwhile says: "Nothing was sent: Ann's encryption key changed. Accept the new key in
your conversation with them first." A change waiting is dropped when a newer card brings the
pinned key back.

The pins are the identity's, not a server's: the desktop app keeps one encrypted file per identity
beside the block list (`dms/<hash>.keypins`, AES-256-GCM under BLAKE3 `derive_key("hum/key-pins/v1",
seed)`), the website one encrypted record per identity in the DM store's IndexedDB (scope SHA-256
of `me\nkey-pins`). Who you have sealed things to is your social graph, so the server never holds
any of it.

When there is no key to seal with, the apps say which of three it is, in the same words:

- no card: "Nothing was sent: Ann has no signed key card on this server, so there is no key we can
  check is really theirs. It appears once they sign in with a current version of the app."
- still coming: "Nothing was sent: Ann's key card is still on its way from the server. Try again in
  a moment."
- changed: as above.

### When an app asks for a card

The people it may seal to: those it has a conversation with, follows, is followed by, or has a
contact request from, asked for when a socket signs in and every 30 seconds after (desktop) or when
the store loads (website); the conversation open in Chat, at once; anyone it is about to seal to
(the website waits up to 8 seconds for the card; the desktop app's sweeps try again once it has
come); and a pinned person whose listed card time is newer than the pin. A card is asked for at
most once a minute.

## Federation

A card newly kept is passed on to linked servers as `{"type":"key_card_gossip","card":{...}}`,
numbered in the federation outbox like a profile copy (`federation_log.rs`, kind `key_card`,
with its own flow allowance; `docs/design/federation-activation.md`), so a linked server that was
down or paused gets it when it catches up, applied once and in order;
the receiving server checks it the same way and keeps it by the same rule, and passes it on only
when the person has an account there, never back to where it came from (the profile copies' rule,
BUG-191). When a link comes up, each side sends the cards of its own accounts (newest 1,000) with
its profile copies. A card that does not check out is refused by every server, whoever sent it.

## Every place that seals to a person (checked 2026-10-22)

| Where | Desktop | Website |
|---|---|---|
| Direct messages | `gui/pages/chat.rs build_dm_puts` | `crypto.js pqBuildDmPuts` (via `chat-ui.js`) |
| Notes: follow, unfollow, friendship passes, choices, contact requests | `engine/dm.rs control_puts` | `pqBuildDmPuts` (`chat-social.js`) |
| Household permits | `engine/household.rs` (through `control_puts`) | (no game on the website) |
| Device-to-device file offers | `engine/file_transfer.rs` | `chat-file-transfer.js` (through `pqBuildDmPuts`) |
| Reports to a group's creator | `engine/group_report.rs` | `crypto.js pqBuildGroupReport` |
| Recovery pieces for trusted people | `settings/recovery_net.rs with_keys` | `pages/recovery-app.js myFriends` |
| A group's key (each epoch) | `net/api_v2.rs group_members_of` | `chat-groups-p2p.js` (`hosKeyCards.memberKey`) |
| A direct channel (P2P contact card) | (none) | `chat-p2p.js sendP2PMessage`; an imported contact card carries the key card, checked |
| Notes to my own devices | my own key | my own key |

The scripted walker the rigs use (`scripts/second-player.js`) sends its card at sign-in and keeps
other people's keys only from cards it checked.

## Feature switches

Asking for cards (`key_card_request`) and the answer (`key_cards`) work whatever the owner switches
off (`WS_ALWAYS_ON` in `src/relay/features.rs`): every private thing a person sends is sealed only
to a key that checks out, so checking a key is a person's own protection, like reporting and
choosing who can reach you. A card is public and signed, so handing one out discloses nothing.
A card passed on by another server (`key_card_gossip`) belongs to federation: an owner who switched
federation off does not take cards in from other servers: no link is dialed or accepted, and
`federation_log::ingest`, the one door every linked server's frames come through, takes nothing in
(2026-10-24; before, a listed peer's hello was still accepted, since a peer's frames never pass the
socket's feature gate).

## Export and erase

The account export lists the person's card (`key_card`); the account erase deletes it here, and the
check that nothing of the person is left (`erase_left_rows`) reads the cards table too. Cards
linked servers keep stay there (a card holds only public keys, and lets people reach the person
where they still have an account), the way a profile copy stays until its withdrawal.

## Known limits

- **A group's key follows the newest good card without the change notice.** The roster's cards are
  checked (signature and name), so a server cannot slip its own key into a group; but the group key
  work runs on threads (desktop) or reads the roster as it is (website), and a member whose key
  legitimately changed is sealed to under the new key without anyone accepting it first. Only the
  member can sign such a card, and a member's group messages are signed by the same identity, so
  the notice would add little there.
- **The first card is trusted on first sight.** Pinning protects against a change after that, the
  way other messaging apps' safety numbers do. A person can compare keys out of the app if they
  want to (not built yet: a short fingerprint of the card to read aloud).
- **A server can withhold a card.** It cannot forge one, but it can refuse to hand one out; the app
  then says nothing can be sent and why.
- **Recovery's thread checks the card but does not pin it.** A trusted person with nothing pinned
  yet is sealed to with their checked card's key; a change already waiting for them refuses.
