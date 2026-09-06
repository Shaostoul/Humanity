# Market bulk importer

> One-at-a-time publishing needs no terminal at all: the native app's
> Market > Directory > "+ Publish" creates your shop and offerings in-app
> (v0.1143), signed with the same identity, validated by the same rules.
> This importer is the BULK path: whole catalogs, nightly re-imports.

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

## Taking something down

There is no delete. A signed object cannot be withdrawn by the server, by an
admin, or by anyone except the key that signed it, and even then the original
object continues to exist. What you do instead is publish a NEWER revision that
says the thing is no longer on offer, which is what the directory reads:

- an offering comes down with `status = "withdrawn"`
- a whole storefront comes down with the provider's `status = "closed"`

Both Market views filter on `status == "active"`, so a withdrawn offering and a
closed provider vanish from the directory. Republish the FULL payload with only
that field changed, exactly as you would for any other update, and sign it with
the same seed you published with. Someone else's key cannot take your listing
down, and yours cannot take down theirs.

```bash
# Withdraw every offering in a catalogue file, then close the storefront.
node -e '
  const fs = require("fs");
  const off = JSON.parse(fs.readFileSync("my-offerings.json", "utf8"));
  off.forEach(o => { o.status = "withdrawn"; });
  fs.writeFileSync("withdraw-offerings.json", JSON.stringify(off, null, 2));
  const p = JSON.parse(fs.readFileSync("my-provider.json", "utf8"));
  p.status = "closed";
  fs.writeFileSync("close-provider.json", JSON.stringify(p, null, 2));
'
node scripts/import-offerings.mjs \
  --server https://united-humanity.us \
  --seed-file ~/.humanity-merchant-seed \
  --provider close-provider.json \
  --offerings withdraw-offerings.json
```

Rehearse with `--dry-run` first. If you no longer hold the seed that published
something, you cannot take it down through this path at all, and the only
remaining option is the server operator editing the database directly.

Note that the importer refuses to publish the bundled `scripts/samples/*.json`
to a non-local server (see the guard added 2026-09-06), because doing so once
put an invented storefront, "Rivertown Bikes", into the live directory where it
sat for months telling visitors the market had listings it did not have. Point
`--provider` and `--offerings` at your own catalogue, or pass
`--allow-sample-data` if you genuinely mean to publish the demo shop.
