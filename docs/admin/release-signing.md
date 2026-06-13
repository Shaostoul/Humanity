# Release signing: the supply-chain root of trust

> Closes the **CRITICAL** finding from the 2026-06-12 security audit: the
> auto-updater used to install a downloaded binary after only a 1 MB size check
> (no signature, no hash), so a GitHub/release compromise, or a wrong "Latest"
> pointer like the v0.415 stray tag, would have been silent RCE on every
> desktop user. Now the updater refuses to install anything that isn't signed by
> the operator's keys.

## The scheme

- **Hybrid Ed25519 + Dilithium3 (ML-DSA-65), BOTH must verify.** Classical
  robustness *and* post-quantum safety; an attacker has to break both to forge a
  release, which also hedges against a bug in the still-young ML-DSA libs.
- A **dedicated** signing key, never the operator's personal identity seed.
- The **private** key is held passphrase-encrypted (Argon2id + AES-256-GCM) on
  the operator's machine and used to sign **locally**. It NEVER goes in CI, the
  whole point is to survive a GitHub/CI compromise.
- Each release is described by a `release-manifest.json` (version + every
  platform binary's SHA-256). The operator signs the verbatim manifest bytes;
  the two signatures ride in `release-manifest.json.sig.json`. Both files are
  uploaded as release assets.
- The client (updater + the local exe launcher) embeds the **public** keys
  (compiled in from `data/release/signing_pubkeys.json`), fetches the manifest +
  signature, verifies BOTH signatures over the exact manifest bytes, then checks
  the downloaded binary's SHA-256 against the manifest. Any failure → abort.

Code: [`src/release_update.rs`](../../src/release_update.rs) (sign/verify/keygen +
tests), the verification wiring in [`src/updater.rs`](../../src/updater.rs), and the
CLI subcommands in [`src/main.rs`](../../src/main.rs).

## One-time setup (operator)

1. Pick a strong passphrase (>= 12 chars) and export it for the session:
   ```bash
   export HUMANITY_SIGNING_PASSPHRASE='your-strong-passphrase'
   ```
2. Generate the keypair:
   ```bash
   just gen-release-key
   ```
   This writes:
   - `data/release/signing_pubkeys.json`, the PUBLIC keys. **Commit this**; it
     compiles into every build.
   - `release-signing-key.enc`, the encrypted PRIVATE key. **Gitignored.**
     **Back it up** encrypted + offline (your 3-2-1). Losing it means rotating to
     a new key (ship a build with new embedded pubkeys).
3. Commit the public keys and ship a release:
   ```bash
   git add data/release/signing_pubkeys.json && git commit -m "release signing: provision keys"
   ```
   Once a build with non-empty embedded pubkeys is out, the updater **enforces**:
   it only offers + installs releases that carry a valid signed manifest.

Until step 2 runs, the embedded pubkeys are empty, the updater logs a loud
warning and falls back to the legacy (unverified) behaviour so the app keeps
working during provisioning. **The CRITICAL is only fully closed once the keys
are provisioned and a signed build is the Latest release.**

## Per-release (operator)

After the tag's **Build Desktop App** workflow has finished uploading the
platform binaries to the GitHub release:

```bash
export HUMANITY_SIGNING_PASSPHRASE='your-strong-passphrase'
just sign-release v0.418.0
```

This downloads the release's platform binaries, builds + signs the manifest, and
uploads `release-manifest.json` + `release-manifest.json.sig.json` to the
release. The updater now trusts that release. A release that hasn't been signed
is simply invisible to auto-update (not offered), which also means a stray or
malicious tag is never auto-installed.

## Notes / future hardening

- The local-build launcher (`find_newer_exe` in `main.rs`) is hardened too
  (v0.419.0): it verifies each candidate `vX_HumanityOS.exe` against its detached
  `.sig.json` sidecar before launching, so a malicious local build is skipped.
  `just build-game` signs each archived dev build automatically when
  `HUMANITY_SIGNING_PASSPHRASE` is set + `release-signing-key.enc` exists
  (otherwise the build is left unsigned and simply isn't auto-delegated-to, 
  `just launch` still runs it directly). Sign a build by hand with
  `HumanityOS --sign-file <path>`.
- Optional custody upgrade: move the Ed25519 half onto a hardware token
  (YubiKey) so the classical key never leaves hardware. The manifest format is
  unchanged by that.
- Key rotation: to rotate, generate a new keypair, commit the new pubkeys, and
  ship a build. Clients update to the new embedded keys via a release signed by
  the OLD key (the last good chain link), then trust the new key going forward.
