# Forward Roadmap: 2026-04-30 snapshot

> Written between sessions while the operator was AFK. Captures what
> was built today, what comes next, what's farther out, and a
> recommended order for picking work back up.
>
> Treat this as advice, not gospel. Adjust to taste.

---

## Where we are

**Today's session shipped v0.122.0 → v0.130.0**, nine releases. Roughly:

| Theme | Releases | Lift |
|-------|----------|------|
| **Audit-driven cleanup** | v0.122.0–v0.124.0 | Federation sig verify, doc truthfulness, BUG-034 updater fix, 17 storage tests, dual-UI parity table |
| **User-reported bugs** | v0.125.0 | BUG-035 (chat reply vanishes) + BUG-036 (channels resurrect) |
| **Channel + plan** | v0.126.0 | Reduced channel seed list, distribution-mirrors plan doc |
| **Distribution sovereignty** | v0.127.0–v0.130.0 | Forgejo at git.united-humanity.us · VPS release mirror at /releases · BitTorrent seeder (50 torrents, ~3.6 GB) · file-level data manifest |

**The distribution-sovereignty progress board:**

```
[✓] Step 1: Forgejo on VPS                    (v0.127.0)
[~] Step 2: Codeberg mirror                   (operator awaiting signup email)
[✓] Step 3: VPS release-binary mirror         (v0.128.0)
[✓] Step 4: BitTorrent seeder + magnets       (v0.129.0)
[✓] Step 4.5: Layered packages + file manifest (v0.130.0 - bonus)
[ ] Step 5: Internet Archive seeder           ← next quick win
[ ] Step 6: Software Heritage                 ← next quick win
[ ] Step 7: WinGet manifest
```

---

## What I'd tackle in the next session

The work falls into three independent tracks. Pick whichever matches energy:

### Track A: finish distribution sovereignty (Steps 5-7)

Three remaining steps, all small:

- **Step 5: Internet Archive seeder**, ~30 min. Upload each release to archive.org via IA's CLI; they auto-generate torrents and act as a permanent free seeder. Adds resilience to the swarm without touching the VPS.
- **Step 6: Software Heritage**, ~10 min form. They harvest the Forgejo + GitHub repos automatically; "Save Code Now" pings them to harvest immediately. Passive forever after.
- **Step 7: WinGet manifest**, ~1 hour PR to `microsoft/winget-pkgs` with a YAML manifest pointing at our release binaries. Every Windows 11 user gets `winget install HumanityOS`.

**Plus**: the operator's Codeberg signup email should arrive eventually. When it does, Step 2 is one signup form + one `git remote add codeberg` + push. Adds a non-profit external mirror.

After all this lands, distribution sovereignty is complete. We can reach
"GitHub fails tomorrow → operators rebuild from VPS / Forgejo / Codeberg / IA / SwH / torrent swarm with no real interruption."

### Track B: User-facing UX work (the original plan after sovereignty)

The operator wants to circle back to UI work. The headline piece is the
**File Explorer / Update Inspector** family:

- **Step 4.6**, `/download` page rewrite on the website. File tree of
  every release, manifest browser, magnet snippets, swarm stats.
  Read-only at first.
- **Step 4.7**, Native `/updates` page. Same UX as the website but
  egui. Reuses the `/files` widget code.
- **Step 4.8**, File-level delta sync in the auto-updater (consumes
  `data-manifest-<tag>.json`).
- **Step 4.9**, Selective sync toggles, mod overlay, P2P contribution
  dashboard.

Step 4.6 is the right entry point, pure HTML/JS, no native
complications, demonstrates the layered architecture to the user.
Builds confidence in the manifest design before native code consumes it.

### Track C: Security debt from the audit

`docs/security-audit-2026-04-30.md` lists 5 BLOCKERs (no urgent crisis
but all real). The two highest-leverage:

1. **B3, DM plaintext downgrade silent fail** (~30 min). User-facing
   trust win. Smallest fix.
2. **B1, FederatedChat signature verification** (~1 hour). Mirror the
   v0.122.0 ProfileGossip pattern. Closes a "compromised peer can
   forge messages from any user" hole.

Both are tractable in a single session. Bigger items (B4, signed
binary releases; B5, signed manifests) are multi-session because they
involve generating + securing an offline signing key.

---

## My recommendation

**Start the next session with Track A's Step 5+6** (Internet Archive +
Software Heritage). It's ~40 minutes total, finishes the sovereignty
sweep cleanly, and feels good, every step ticks a box.

**Then pick between Track B and Track C** based on what feels right:
- If the operator wants to *use* the new infrastructure (UX), do Track B's
  Step 4.6 next.
- If the security findings are nagging, do Track C's B3 + B1 first.

Either order works. They're independent.

**Then come back to the harder stuff**: Step 7 (WinGet PR), B4/B5 (signed
releases), Step 4.8+ (delta sync).

---

## Longer horizon (not for next session, but worth tracking)

Concepts the architecture is *quietly building toward* without forcing yet:

### 1. Federated source distribution via signed objects

Forgejo's eventual support for **ForgeFed** (ActivityPub-based forge
federation) means each operator's Forgejo could federate with others.
Combined with the existing PQ signed-object substrate
(`signed_objects` table, BLAKE3-addressed), you could in principle
publish each commit as a `commit_v1` signed object that gossips
across the federation. The repo becomes federation-native.

This is essentially **Radicle** (peer-to-peer git) but built on
infrastructure HOS already runs. Not for tomorrow, but the pieces are
in place.

### 2. Mod store via the same manifest format

The file-level manifest (Step 4.5) format is generic. A modder
publishes `mod-<name>-manifest.json` with the same shape. The native
File Explorer overlay shows the mod's files. A user enables the mod →
client syncs the files → hot-reload picks them up → mod is live with
no restart.

This is HOS's "marketplace, not a store." Mods become first-class via
the existing infrastructure.

### 3. Local-first, server-amplified updates

Once Step 4.8 lands (file-level delta sync), the natural next step is:
**users on the same LAN gossip among each other before reaching out to
the VPS or other peers**. mDNS or local broadcast for peer discovery.
Cooperative households, schools, libraries all win.

### 4. Reproducible builds

The release-binary signing (B4) opens the door to **reproducible
builds**, anyone can rebuild from the source tree at a tag and
verify their binary matches the signed one. Trust becomes
"auditable," not just "from a known signer." Standard library
problem, well-trodden path; not urgent until the project has
contributors who care.

### 5. Auto-updater as gameplay

What if the act of seeding releases counted as a contribution metric
in the trust system? Users earn reputation for helping the swarm, 
visible on profile cards, factored into the trust score, surfaceable
in the governance vote weight. **Software distribution becomes a
cooperative civic act inside the platform itself.** Aligns with the
"infrastructure for civilization" mission in a way no commercial
platform can match.

That last one is HOS's killer move. None of the steps in the
sovereignty plan close it off; several open onto it.

---

## Maintenance reminders for the next session

- Operator to push v0.130.0 to Codeberg once that signup email arrives;
  it's the only outstanding Step 2 work.
- The forge push GCM auth occasionally hangs on the first attempt;
  retry usually succeeds. If it becomes annoying, switch to SSH key for
  forge instead of HTTPS-via-GCM.
- `data/coordination/orchestrator_state.json` is up to date through
  v0.130.0. Read it at session start.
- The `assets/brand/` directory remains untracked, operator's
  in-flight logo proposals. Commit when ready or .gitignore.

End of writeup. v0.130.1_HumanityOS.exe is ready in the repo root for
testing.
