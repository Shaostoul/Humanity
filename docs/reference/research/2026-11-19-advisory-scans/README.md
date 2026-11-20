# Advisory scans in CI: the tools compared (2026-11-19)

The post-quantum review's plan item 1b (docs/reference/research/2026-11-16-pqc-review/) asks for a
CI job that fails on a known advisory in the versions we pin, for the Rust crates and the
website's JavaScript. The house rule (CLAUDE.md, 2026-11-08) is to compare the open alternatives
before adopting a tool. Read 2026-11-19 through GitHub's API (repository metadata, READMEs and
documentation files at their default branches) and crates.io through `cargo search`.

## Rust

| Option | Publisher, licence | Latest (2026-11-19) | What it does | Fit |
|---|---|---|---|---|
| `cargo audit`, installed in the job | RustSec (the Rust Secure Code working group's advisory database), crate licence "Apache-2.0 OR MIT" (cargo-audit's Cargo.toml) | 0.23.0 on crates.io (GitHub release cargo-audit/v0.23.0, 2026-11-10); the dev host runs 0.22.2 | Reads Cargo.lock against the RustSec database; exits non-zero on a vulnerability; unmaintained, unsound and yanked are warnings; `--ignore` or `audit.toml` to accept one by id | **Chosen.** The tool and database docs/SECURITY-CADENCE.md's monthly check already runs on the dev host, so a red in CI reads the same as the monthly check. No configuration needed today. |
| `rustsec/audit-check` action | RustSec, MIT | v2.0.0, released 2024-09-23 | Runs cargo-audit and reports through a check run; on a cron schedule it opens issues | Not chosen: its source installs cargo-audit with `cargo.findOrInstall('cargo-audit')` (an unpinned latest), wants `checks: write` and `issues: write`, and its last release is two years old. The README says of informational advisories: "Note that informational advisories are not affecting the check status." Same database as the chosen option. |
| `cargo-deny` (`EmbarkStudios/cargo-deny-action`) | Embark Studios, Apache-2.0 | 0.22.0 / action v2.1.3, both 2026-11-19 | Advisories, plus licences, banned crates and allowed sources, from a `deny.toml` policy | Not now. Broader than asked: its licence check would suit the AGPL-3.0 move (memory `project_licence_plan_b`), but it needs a curated policy first. Its default for unmaintained crates: "`workspace` (default) - Unmaintained advisories will only fail if they apply to a crate which is a direct dependency of one or more workspace crates." Released the same day as this scan, so not yet two weeks old. A good later step for licences. |

**The pin.** The job builds cargo-audit with `cargo install cargo-audit --locked --version =0.22.2`,
the version the dev host runs, so both read the database the same way; moving it is a deliberate
change, as the review asks of every pin. It runs outside the checkout so the project's pinned
toolchain is not installed just to run it, and the binary is cached by version.

## JavaScript

The website (`web/`) has no package.json: every library it ships is a single vendored file. The
one that carries the identity is `web/shared/vendor/noble-pq.bundle.js`, built by
`scripts/build-noble-bundle.mjs` from exact pins (`@noble/post-quantum` 0.6.1, `@noble/hashes`
2.2.0) with no lockfile. The only lockfile is the root `package-lock.json` (sharp, the image
scripts' development tool).

- **`npm audit`** (npm's own, ships with node): the local documentation (npm 11.8.0,
  `docs/content/commands/npm-audit.md`) says "By default, the audit command will exit with a
  non-zero code if any vulnerability is found." **Chosen**, for the root lockfile as it is, and
  for the vendored bundle's two pins through `scripts/audit-vendored-js.mjs`, which writes a
  throwaway lockfile from them (`npm install --package-lock-only --ignore-scripts`: package
  metadata only, no package code fetched or run) and audits that. It refuses to run when the
  builder's pins and the bundle's own header disagree, so what is audited is what ships. Limit:
  the two pins are exact, but what they depend on is resolved on the day.
- **OSV-Scanner** (google/osv-scanner, Apache-2.0 per its repository; v2.7.1, 2026-11-09) reads
  lockfiles across ecosystems and would cover Cargo.lock too. Not chosen: a second database and tool to learn for what the two above already cover; worth
  a look if the project grows more ecosystems.

## Seen red, and the first runs (2026-11-19, dev host, nothing pushed)

- `cargo audit --no-fetch --file Cargo.lock` (cargo-audit 0.22.2, its database last fetched
  2026-11-06) over a scratch lockfile pinning `crypto-bigint` 0.5.5: exit 1, "ID:
  RUSTSEC-2026-0387", "error: 1 vulnerability found!". Over this repository's Cargo.lock: exit 0,
  862 crates, "7 allowed warnings found" (the unmaintained instant, paste, ttf-parser, zbus_names,
  zvariant, zvariant_derive and the yanked async-executor, as the 2026-11-06 check found).
- `npm audit --audit-level=low` over a scratch lockfile pinning minimist 1.2.5: exit 1, "Prototype
  Pollution in minimist", "1 critical severity vulnerability".
- `node scripts/audit-vendored-js.mjs`: exit 0, "found 0 vulnerabilities" for @noble/post-quantum
  0.6.1 and @noble/hashes 2.2.0.
- **`npm audit` over the root package-lock.json: exit 1.** sharp 0.34.5 "Severity: high",
  GHSA-f88m-g3jw-g9cj (libvips), GHSA-rgj7-g3m4-5g8c (libheif), GHSA-wq5f-xc86-pv6w (librsvg);
  the fix is sharp 0.35.5, "a breaking change". sharp is a development tool here (the rigs' image
  measurements read the game's own screenshots), nothing the website or the app ships; the job
  is red on its first run until sharp moves, which needs a download on the dev host (BUGS.md).
  **2026-11-20:** main moved sharp to 0.35.5 (9de7ba691); `npm audit` over the root lockfile: "found 0
  vulnerabilities" (BUG-327 fixed).
