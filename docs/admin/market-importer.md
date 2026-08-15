# Market bulk importer

Publish a provider entry and a whole catalog of offerings onto any HumanityOS
server in one command. Everything is signed with your own identity and
validated server-side against the schemas; the platform lists and introduces,
it never moves money (settlement is directory-only by design).

The shapes are defined in `schemas/provider.toml` and `schemas/offering.toml`.
Read those when a field is unclear; the server enforces them and its error
messages quote them.

## One command

```bash
node scripts/import-offerings.mjs \
  --server https://united-humanity.us \
  --seed-hex <your 64-hex master seed> \
  --provider scripts/samples/provider-sample.json \
  --offerings scripts/samples/offerings-sample.json
```

- `--seed-hex` (or `--seed-file`) is the 32-byte master seed behind your chat
  identity; the merchant identity IS your chat identity (same Dilithium3 key).
- `--provider` is your shop/organization entry (one JSON object).
- `--offerings` is a JSON array of offerings: goods and services mixed.
- `--dry-run` builds and signs everything without sending.

Copy the two sample files and edit them; they show a good (with stock) and a
walk-in service (with weekly hours).

## Re-running is the workflow

An offering's identity is `(provider, offering_key)`, so use your own stable
SKU or shelf code as `offering_key`. Re-running the import UPDATES rows
(publishes fresh revisions) instead of duplicating them, and also "touches"
every offering so it stays fresh: offerings expire by TTL (default 30 days)
so the directory never fills with ghosts. A nightly or weekly re-import from
your stock system is exactly the intended use.

## What the server rejects, on purpose

- `settlement.mode` other than `"directory"` (wallet/escrow are reserved for
  future addons; the platform holds no funds today).
- Offerings signed by a key that does not own the referenced provider entry:
  nobody can publish into your shop but you.
- Counted stock on `while_supplies_last`, prices on `free`, a service table
  on a good, clocks more than 5 minutes in the future, and every other rule
  the schemas state. The error message tells you which rule and why.

## Querying the directory

Every Market view is a query over `GET /api/v2/objects`:

```bash
curl "https://united-humanity.us/api/v2/objects?object_type=offering_v1"
```
