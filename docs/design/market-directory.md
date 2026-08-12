# The Market: a federated directory of providers and offerings

> Design settled 2026-08-12 with the operator. This is the "infinite retailers
> federate their catalogs" vision, scoped so it ships on ONE server first and
> inherits federation later. Read `docs/design/two-realities.md` alongside it:
> the Market is the Real-side twin of the game's item/trade system, and they
> share a spine on purpose.

## The phone-book model (operator's framing)

Old phone books had two books, and people understood them instantly:

- **White pages = the People Directory.** Who exists. Users, profiles, keys.
  Already built (`/api/members`, signed profiles). Cool/untinted in the UI.
- **Yellow pages = the Provider Directory.** Who OFFERS. A provider is a role
  a white-pages identity wears when it publishes goods or services. Warm light
  tint in the UI (the phone-book nostalgia is deliberate; older users read it
  at a glance).

A person is not always a provider; a provider is not always one person. The
same identity can appear in both books - I am a user (white) who also fixes
bikes (yellow). The two are LINKED (a provider back-references its identity)
but LISTED SEPARATELY.

## The three nouns

- **Provider** - a ROLE, not a business type. Target, a county library, a food
  bank, a solo craftsperson are all providers. Kept role-shaped so it stays
  infinite-of-X: any federated identity can wear the hat. When a provider is
  more than one person, it is backed by a `group_v1` object (that family
  already exists, with membership/invite/join/epoch-key hooks).
- **Offering** - the thing published. NOT "listing" (implies a sale), NOT
  "product" (implies goods only). An offering is a good OR a service, paid OR
  free - which is the whole mission point (a food bank's free meal and Target's
  flashlight are the same kind of object).
- **Market** - the browsable face, the in-game mall. It is a VIEW built from
  the yellow pages, not a fourth stored thing.

## Why "merchant" was rejected

"Merchant" means sells-for-profit. The mission includes mutual aid, libraries,
free clinics, tool-lending co-ops. The codebase already leans cooperative
(`coop` in 45 files, `steward`, `org`). A word that excludes the food bank
excludes half the point.

## Directory, not marketplace (the money decision)

**Settlement is directory-only to launch** (operator decision, 2026-08-12).
The directory LISTS offerings and shows a price and a way to reach the
provider; buyer and seller settle however they like (cash in person, the
built-in Solana wallet peer-to-peer, or the provider's own checkout link). We
move no money and custody no funds.

This is not a limitation, it is a liability firewall: a marketplace that
processes payments is a regulated money-transmitter (KYC/AML/licensing,
per-state), which a donation-funded project cannot carry to launch. A directory
achieves "infinite retailers federate their catalogs" with a fraction of the
exposure.

But the schema is built to GROW into it. Every offering carries a `settlement`
block whose only mode today is `directory`. Two addon modes slot in later with
NO schema migration:

1. `wallet` - a "Pay with wallet" action using the existing Solana wallet;
   buyer signs and sends directly to the provider's address. Non-custodial, so
   still not a money transmitter. (Note: the map found web wallet USDC send is
   currently broken by a name mismatch - `wallet-app.js:394` calls `sendToken`,
   `wallet.js` exports `sendSPLToken` - fix that before wiring this mode.)
2. `escrow` - full order state, refunds, dispute resolution. This is the
   money-transmitter line; do not cross it without legal review. `dispute_v1`
   objects already exist to build on.

The operator's frame: directory is CORE (and doubles as the game's trade
system), wallet-pay and marketplace are ADDONS layered on top, not rewrites.

## The shared-taxonomy spine (the load-bearing decision)

The operator requires two shopping modes:

- **By storefront** - walk into Target, browse Target's offerings.
- **By offering across all providers** - "find all flashlights by ALL
  providers, not just Target."

The cross-provider view only works if every provider's flashlight maps to a
SHARED concept of "flashlight." Free text cannot aggregate ("LED Flashlight
800L" vs "Flashlight, Tactical"). So:

**An offering references the game's item taxonomy (`item.id`, snake_case, as in
`schemas/item.toml` / `data/items.csv`) for GROUPING, and carries its own
`title` for SPECIFICITY.** Target's offering is `{ item_ref: "flashlight_0",
title: "LED Flashlight 800L", ... }`. The cross-provider view queries by
`item_ref`; the storefront view queries by `provider`.

This is why the Real-side Market and the game item system share a spine - the
same `item_ref` a player crafts in-game is the concept a real provider offers.
That is the two-realities principle, not a coincidence. Real-world goods that
have no game-item equivalent map to the nearest category and MAY propose a new
taxonomy entry (a curation/moderation path, not free-for-all).

## How it rides existing infrastructure

Both new nouns are SIGNED OBJECTS (`src/relay/storage/signed_objects.rs`), the
generic typed envelope that already carries dozens of types
(`employment_v1`, `controlled_by_v1`, `dispute_v1`, ...):

- `provider_v1` - the yellow-pages entry: display name, description,
  verification status, back-reference to the signing identity or a `group_v1`.
- `offering_v1` - provider ref, `item_ref` (taxonomy spine), title,
  description, category, `price {amount, currency}` (nullable = free/inquire),
  availability (stock count for goods / schedule for services), condition,
  fulfillment (pickup/ship/remote/in-person), location, `settlement` block,
  and a TTL / `updated_at` so stale inventory ages out of the Market.

Being signed objects gives them signing, verification, and a replication
mechanism. It does NOT mean the catalog should be replicated everywhere - see
the next section, which is the correction that governs everything here.

The Market views are queries over `offering_v1`: storefront = filter by
provider, cross-provider = filter by `item_ref`, category browse = filter by
`category`. Note `list_signed_objects` filters only on object_type / space_id /
author_fp today, so the item_ref and category views need indexed columns or a
projection table (known work; see schemas/offering.toml `[relationships]`).

## NO SERVER HOLDS THE GLOBAL CATALOG (operator correction, 2026-08-12)

An earlier draft of this document said offerings "gossip and FEDERATE for free."
That was wrong, and it was wrong in the direction that breaks at exactly the
scale this design exists for. The operator caught it:

> "United-Humanity.us wouldn't store a list as the space would become too much
> with infinite offerings. A client could cache a list, multiple lists,
> dependent on the thing it is referencing on their own PC, that way people
> remain self-custodial. Not all hardware can store infinite objects so we need
> to keep the list pruned perpetually to not OOM or whatever."

Auto-gossiping every offering to every peer means every server eventually holds
every offering of every provider forever. One retailer with a million SKUs
would impose a million objects on every node in the network, including the ones
running on a donated laptop. The flagship server becomes a central warehouse -
the exact centralisation this platform exists to avoid, and an unbounded
storage bill nobody agreed to.

**The model is PULL, not PUSH:**

1. **A provider's node holds its OWN offerings.** That is the authoritative
   copy, and it is the only complete copy of that catalog. A provider's storage
   scales with their own inventory, which is fair and self-limiting.
2. **Discovery replicates; catalogs do not.** What federates is small and
   bounded: `provider_v1` entries (who exists, what they broadly offer, where
   to reach them) - the yellow-pages INDEX. Not the shelves.
3. **Clients pull what they are actually looking at, and cache it locally.**
   Searching "flashlights" queries the relevant providers' nodes and caches the
   results on the user's own machine. The user's list lives on the user's
   hardware: self-custodial by construction, not by policy.
4. **The cache is bounded and pruned perpetually.** Not all hardware can hold
   infinite objects. A client cache needs a size cap, an eviction policy, and
   respect for each offering's own TTL (`updated_at` / `ttl_days` /
   `expires_at`, already in the schema - those fields exist precisely for this).
   Pruning is a REQUIREMENT, not a nicety: an unbounded cache OOMs the weakest
   device in the network, and the weakest device belongs to the person this
   platform is most for.

This also makes staleness honest. A cached offering is a snapshot with a
timestamp; the client shows "last confirmed N days ago" and refetches from the
source when the user cares. A gossiped global catalog would have shown every
node's stale copy as though it were current.

## Sharding: infinite servers, not one big one

The operator's plan for load: united-humanity.us hosts everything at first,
then aspects SHARD OUT across other servers so work spreads instead of piling
onto one box.

The capability manifest (`features` in `data/server-config.json`, with real
route-level enforcement) IS the sharding mechanism. A node advertises what it
serves; a client routes to a node that serves it. Chat can live on one server,
the directory on another, the game on a third, and none of them carries the
others' load or storage.

Concrete near-term shape:
- **united-humanity.us** - everything, for now. The flagship.
- **public.guide** - a BASIC CHAT SERVER to start (operator, 2026-08-12): the
  simplest possible second node, purely to prove federation works end-to-end
  between two real hosts on different domains before any feature depends on it.
  Chat is the right first test because it is the one federated flow that
  already has code on both sides.
- **shaostoul.com** - untouched, posterity.

Do not give a second node a complicated job until a simple one demonstrably
works. Stocking nodes with real content comes after federation is proven
non-glitchy, not before.

## What we absolutely need (ranked, from the gap map + this design)

1. **Provider identity verification.** Nothing today stops a node calling
   itself "Target" and listing fake goods - in a federated network this is THE
   attack. Need a claim/verification path (domain proof, signed attestation,
   or manual verification for big names) before any real provider. `group_v1` +
   signed profiles are the foundation; the verification LAYER is unbuilt.
2. **The offering schema, goods vs services.** They are different shapes (stock
   / variants / shipping vs availability / scheduling / remote-or-in-person).
   One flat object cannot model both honestly - the schema must branch.
3. **A validating importer.** "Upload your inventory to our specs" = a format
   PLUS a validator that REJECTS malformed data at the door, or one sloppy
   provider pollutes the shared catalog.
4. **Moderation / delisting in a federated world.** One server's admin cannot
   police another's offerings, but the AGGREGATING server chooses what to
   surface. Need a delist/blocklist model or the first scam poisons trust.
5. **Staleness / expiry.** Inventory rots. Offerings need a TTL or update
   cadence or the "auto-updated list" fills with ghosts.

## The increment ladder (each ships something real on one server)

1. **`offering_v1` + `provider_v1` schemas** (schemas/, this is the shape).
2. **A validating importer** on one server: a bulk format + a validator that
   rejects malformed rows, riding `POST /api/v2/objects`.
3. **The Market views**: storefront + cross-provider + category, as queries.
4. **Provider verification** (the trust layer) before inviting real providers.
5. Only then: wallet-pay addon (fix the USDC bug first), then - with legal
   review - escrow.

Federation replication is NOT on this ladder: it is a separate track
(fix the 3 federation bugs) that, once done, lights up cross-node Market
discovery with no changes to any of the above.

## Hardware floor for a node (measured 2026-08-12)

A provider should not need a big server to join. Measured on the live VPS:

| | Running the relay | Building it from source |
|---|---|---|
| RAM | **19.7 MB** resident | 1-2 GB peak (rustc) |
| Disk | 26 MB binary + data | +1.4 GB `target/` +351 MB cargo |
| CPU | **0.5%** of one core | 31-35 min on FOUR cores |

Running HumanityOS is featherweight; COMPILING it is not. If provisioning always
built from source, the minimum spec for a node would be set by the compiler
rather than by the software - which prices a small provider out of the network
over a cost the software never actually incurs at runtime.

So CI publishes a prebuilt headless relay (`HumanityOS-relay-linux-x64` plus a
`.sha256`, from the `relay` job in build-desktop.yml) and provision-vps.sh
FETCHES and checksum-verifies it, building from source only as a fallback or
when `HUMANITY_BUILD_FROM_SOURCE=1`.

**Practical floor for a federation node: 1 GB RAM / 1 core / 20 GB disk**
(roughly 30x headroom on RAM). That is the cheapest tier most hosts sell. A
node that also wants to BUILD from source wants 4 GB+ RAM and 4 cores, or a
long afternoon and a big swap file.

Two things a second node still needs regardless of size: its own DOMAIN (TLS
certs are per-domain; see `node.env`) and DNS pointing at it before provisioning
runs, because certbot validates over port 80 during the script.
