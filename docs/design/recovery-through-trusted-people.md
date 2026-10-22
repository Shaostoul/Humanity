# Recovery through trusted people

**Status: built 2026-10-21** (desktop app: Settings > Account; website: /recovery). The code:
`src/relay/core/recovery.rs` (the protocol, shared by the relay and the desktop app),
`web/shared/recovery.js` (its website twin), `src/relay/storage/recovery.rs` and
`src/relay/api_v2_recovery.rs` (what the server keeps and its routes),
`src/gui/pages/settings/recovery.rs` + `recovery_net.rs` (desktop),
`web/pages/recovery.html` + `recovery-app.js` (website). The algorithms are in
`docs/reference/cryptography.md` ("Recovery through trusted people").

## What it does, in plain words

Someone who loses their device AND their recovery phrase can get back into the same account with
the help of a few friends they chose earlier. Each friend keeps one locked piece of the recovery
phrase. It takes as many of them as the person chose (for example 3 of 5), together, to put the
phrase back. The server keeps only locked pieces and is never told who the friends are.

## The four steps

1. **Set up** (the person, on a device with their account). They pick 2 to 10 friends (people
   they follow who follow them back) and how many must help (at least 2). Their app splits the
   32 bytes the recovery phrase writes down with Shamir's secret sharing over GF(256) (the
   `blahaj` crate; the website has the same arithmetic written out), seals each piece to one
   friend's Kyber768 key the way a sealed DM is sealed, and posts one `recovery_share_v1` object
   per friend, signed by the person. Each object also carries a note sealed to the person's OWN
   Kyber key ("this piece is kept by Ann, did:hum:..."), and the hash of a random answer token
   that is inside the friend's piece. Changing the setup sends new pieces: the new set replaces
   the old one only once all of its pieces have arrived. Taking it back deletes every piece from
   the server.
2. **Ask for help** (on a new device, with no key). The device makes a one-time Kyber768 key
   pair, keeps the secret half in memory only, and opens a request naming the account and the
   one-time public key. No signature is possible (the key is lost) and none is needed: the
   request alone gives nothing to anyone. The device shows a code, 16 characters in Crockford's
   base 32 (`ABCD-EFGH-JKMN-PQRS`), and the same code as a QR picture that opens the website's
   Recovery page on a phone with the code filled in.
3. **Help** (each trusted person, on their own device). Their app shows "Ann is asking for your
   help to recover their account", with the guidance: call them on a number you already had, or
   see them in person; never trust a text, an email or a message from a new account alone. The
   person reads them the code. The trusted person types it; their app fetches the request,
   checks that the code is the fingerprint of the request's key, opens its own piece, and sends
   it back sealed to the one-time key, with the slot's answer token.
4. **Restore** (the new device). When enough pieces arrived, it rebuilds the 32 bytes, checks
   they derive the very account that asked (any wrong piece is tried around: every choice of the
   needed number is tried, at most 252), writes them as the 24-word recovery phrase, and restores
   through the existing "Restore from recovery phrase" path (desktop: `restore_from_phrase` in
   `settings.rs`; website: Chat's restore box, handed the words in this tab's sessionStorage).
   The same account comes back: no key change. (The first design's `key_rotation_v1` step is
   gone; its route was deleted in Inc5b.)

## Why asking for help is safe

- **Nothing the server holds can be read by it.** A piece is sealed to a trusted person's key, a
  note to the person's own key, an answer to the one-time key that only the asking device has.
  The relay test `the_server_keeps_only_what_it_cannot_read` searches the database file for any
  trusted person's account id, name or key, the secret, and the pieces in the clear.
- **No server step can collect the pieces.** The code a person reads aloud is BLAKE3
  `derive_key("hum/recovery-ask/v1", account "\n" one-time key)`, first 80 bits. A server that put
  its own key into the request would have to find a key whose fingerprint matches an 80-bit code
  it did not choose: a second preimage, about 2^80 tries, inside the seven days a request lasts.
  The trusted person's app refuses to send when the fingerprint does not match.
- **A request from someone pretending to be the person fails at the phone call.** Anyone can
  open a request naming any account that has trusted people. What they cannot do is be the
  person on a call or face to face, and the trusted person's app sends nothing until they have
  typed a code they were read and ticked that they spoke to the person themselves. An attacker
  has to fool as many trusted people as the person chose, not one.
- **The person is told.** Their own devices show "Someone asked your trusted people for help to
  get into your account (on <date>)", with "That was not me: stop it", which closes every
  request naming them (signed with their key, `recovery_not_me`).
- **Junk cannot crowd out the real answers.** The server takes an answer only with the slot's
  answer token: the token's hash is public in the setup, the token itself is readable only by the
  trusted person the piece was sealed to.

## Why the server is not told who the trusted people are

The first design (2026-05, "Phase 4") put each guardian's account id in the clear in the share
object and served `GET /api/v2/recovery/shares-held-by/{guardian}`: a server-side social graph of
who trusts whom, the thing CLAUDE.md's privacy rules removed with the follows table. This design
keeps none:

- The person's own devices learn who keeps each piece from the notes to self.
- A trusted person finds their pieces by trying to open them: the requests list gives only whose
  request it is and when, and the app tries the (at most 10) pieces of that account's setup with
  its own key. "Whose pieces do I keep?" does the same for each of the person's friends.
- The server checks an answer with the token's hash, so it never needs to know who answered.

## Limits and lifetimes (the server)

- A setup: 2 to 10 pieces, at least 2 needed; a piece signed more than a day ago or more than ten
  minutes ahead of the server's clock is refused (a setup is posted as it is made); one setup
  standing and one being sent per person; a setup never finished goes a day after it began.
- A floor per person: a piece signed at or before the newest setup that stood, or before the
  person took everything back, is refused, so old pieces cannot be posted again.
- Requests: 7 days each; at most 3 open per account and 1,000 on the server; one answer per slot.
  Reads leave an ended request out; the six-hour expiry pass deletes it with its answers.
- Pieces stay on the one server they were given to: they are not in the object store that
  `/api/v2/objects` lists and federation passes on, and a piece arriving from another server is
  refused. Taking them back therefore takes them away.
- Account erase deletes the person's pieces, floor, the requests naming them and those requests'
  answers; the account export lists all four (`storage/account.rs`).

## Known limits

- **A piece someone already opened can stay with them.** Taking the pieces back deletes them
  from the server, but a trusted person's app has opened their piece whenever it helped or
  checked. Pieces from different setups never fit together (each setup is a fresh random
  split), so an old piece matters only if enough people from that same old setup work together.
  The section says so where the person takes pieces back.
- **The trusted people's keys come from the server's member list**, the same as for direct
  messages and group keys today. A server that lied about a key while the person set up could
  open that one piece. Signed key cards would close this for every feature at once; until then
  it is the same trust the rest of the app already places in the server.
- **The asking device keeps its one-time key in memory only.** Closing the app or the tab means
  asking again with a new code (on purpose: nothing on that device's disk can open the pieces).
- **The website needs the account in this browser** (Chat's stored key) to set up or to help;
  someone who keeps their key only passphrase-wrapped signs in to Chat first (the same limit as
  every standalone page, `pq-relay-auth.js`).
