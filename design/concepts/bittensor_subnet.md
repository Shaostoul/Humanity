**Humanity Chat / United Humanity Network as a Bittensor Subnet**  
**Version 1.0 – February 19, 2026**   
**With technical synthesis from Grok (xAI)**  

This living document outlines the full vision, design, implementation roadmap, economics, costs, risks, and next steps for evolving **Humanity Chat** (the open-source, no-accounts, E2E-encrypted P2P communication app with voice/video, livestreaming, and optional federated servers) into a **Bittensor subnet**.  

The goal: Turn voluntary community infrastructure into a self-sustaining, economically incentivized decentralized public good — perfectly aligned with the 2017-inspired mission of united-humanity.us (public-domain CC0 tools to end poverty, reduce isolation, and unite humanity through capability-building).

---

### 1. Executive Summary
Humanity Chat becomes **“HumanityComm Subnet”** (proposed netuid TBD) — the first Bittensor subnet focused on **decentralized private real-time communication infrastructure**.  

Miners earn TAO by providing reliable P2P relaying, stream seeding, and optional federated node hosting. Validators score quality-of-service (QoS) metrics **without ever seeing content** (full E2E privacy preserved).  

**Key Outcomes**:
- Self-funding growth via daily TAO emissions (no more donation-only servers).
- Exponential adoption: Users/miners earn while building community.
- Native integration with Bittensor AI (real-time translation, moderation agents, search).
- Full alignment with no-corporate, public-domain ethos.
- Novelty: First comms-focused subnet in a sea of AI inference/compute/data subnets.

**High-Level Timeline**: Testnet MVP in 4–8 weeks; mainnet launch in 3–6 months (post-funding).  
**Estimated Startup Cost**: $200k–$500k USD equivalent (mostly registration + dev).  
**Potential ROI**: Emissions could repay registration in <12 months at moderate adoption; long-term value through ecosystem lock-in.

---

### 2. Background
- **Humanity Network / Chat** (united-humanity.us): CC0 open-source P2P app. Features: no accounts/tracking, cryptographic friend codes, E2E encryption (Signal-protocol style), voice/video calls, P2P-seeded livestreams, optional self-hostable federated servers for hybrid scaling, desktop PWA + mobile plans.
- Mission: Voluntary, non-corporate infrastructure to connect isolated humans and build capabilities.
- Current state: Volunteer-driven, donation-funded servers. Strong technical foundation but needs economic flywheel for scale.
- The original suggestion (from @ttaovolution reply to the launch thread) perfectly matches: “Turn this into a Bittensor subnet.”

---

### 3. Bittensor Subnets 101 (2026 Context)
Bittensor is the decentralized “marketplace for intelligence.” Subnets are independent incentive markets inside the 128-subnet cap.  
- **Miners** produce a digital commodity → earn TAO.  
- **Validators** score quality → set weights → determine emissions split.  
- **Yuma Consensus** (v3) + dTAO mechanics ensure fair, competitive rewards.  
- Registration: **Burn cost** (dynamic, currently **887.48 TAO** as of Feb 19, 2026 — burned/recycled, not locked). Doubles on new registrations, decays over time. One new subnet every ~4 days. New subnets get **4-month immunity** from deregistration and **1-week inactivity period** (no emissions while bootstrapping).  
- Current top subnets (emissions %): Chutes (SN64 ~10.44% — compute), Affine (SN120 ~8.88%), Score, Vanta, Targon. No existing comms/P2P subnet — huge greenfield opportunity.

Sources: taostats.io, learnbittensor.org docs.

---

### 4. Vision & Value Proposition for united-humanity.us
- **The Commodity Produced**: Reliable, private, censorship-resistant real-time communication bandwidth & infrastructure.
- **Why It Fits Bittensor Perfectly**: Bittensor rewards useful digital work. HumanityComm provides measurable, verifiable **QoS** (uptime, latency, bandwidth) without compromising privacy.
- **Mission Supercharge**:
  - Users running the app can opt-in as miners (earn TAO for background relaying/seeding).
  - High-traffic livestreams become profitable for seeders.
  - Federated server operators earn for hosting.
  - Attracts TAO ecosystem builders who share anti-centralization values.
  - Enables on-subnet AI (pull translation/models from other subnets) → multilingual global community at zero extra cost.

---

### 5. Detailed Incentive Design (Core Innovation)
**Subnet Name Proposal**: HumanityComm (or UnitedHumanity Subnet)

**Miner Role** (produce the commodity):
- Run a full node/relay in the Humanity Chat P2P overlay.
- Advertise availability via Bittensor axon/synapse.
- Provide: connection relaying, stream seeding, optional TURN/STUN-like services.
- Optional advanced: host federated server, run AI translation proxy (pulls from other subnets).
- Metrics rewarded: sustained bandwidth contributed, successful relay hours, stream-seed volume (measured via test packets).

**Validator Role** (score quality):
- Randomly select miners and run **privacy-preserving probes**:
  - Encrypted test packets (never decrypted on validator).
  - Measure: latency (<200 ms target), bandwidth (>1 Mbps sustained), packet loss (<1%), uptime (99.9%+), connection success rate.
  - DDoS/spam resistance, geographic diversity scoring.
- Use Bittensor’s commit-reveal weights to prevent gaming.
- Bonus scoring for miners that serve real user traffic (opt-in anonymized QoS telemetry from app).

**Scoring & Emissions** (custom `reward.py` / `forward.py`):
- Composite QoS score (0–1) per miner.
- Validator consensus via Yuma → weights → proportional TAO split (miners ~70–80%, validators ~20–30% typical).
- Hyperparameters to tune (owner-controlled):
  - `activity_cutoff`, `immunity_period`, `commit_reveal_weights_enabled`, `serving_rate_limit`, `max_burn`/`min_burn`, `liquid_alpha_enabled`, etc.
- **Unique Flywheel**: App users earn “micro-rewards” in TAO for background mining. High-viewership streams auto-attract more seeders (profit motive).

**Privacy Guarantee**: Zero content inspection. All probes are cryptographic challenges only.

**Future Extensions**:
- Phase 2: AI agents for moderation, summarization, translation (cross-subnet calls).
- Phase 3: Decentralized identity / reputation layer on top.

---

### 6. Technical Implementation Plan
**Base**: Fork official `opentensor/bittensor-subnet-template` (GitHub).  
Key files to customize:
- `template/protocol.py` → define Miner/Validator message schemas.
- `neurons/miner.py` → integrate with existing Humanity Chat Rust/TS backend (expose relay endpoints).
- `neurons/validator.py` + `template/forward.py` + `template/reward.py` → QoS probing logic.
- App updates: “Earn TAO” toggle → runs miner in background.

**Phases**:
1. **Local/Staging** (1–2 weeks): Run template + Humanity P2P integration.
2. **Testnet** (2–4 weeks): Deploy, iterate incentives, invite community miners.
3. **Mainnet** (4–8 weeks post-testnet): Burn registration, 1-week bootstrap, start emissions.
4. **Post-Launch**: App store updates, dashboard on united-humanity.us, TAO reward claims.

**Integration Points**:
- Existing P2P stack (libp2p or custom) exposes axon.
- Federated servers become high-tier miners.
- Bootstrap nodes initially run by project (later decentralized).

---

### 7. Economic Model & Tokenomics Impact
- **Emissions Capture**: Target 1–5% of total Bittensor emissions initially (realistic for useful novel subnet).
- At current ~7,200 TAO/day network emissions → potential 72–360 TAO/day to subnet.
- Split: miners/validators + small owner cut for development.
- **dTAO**: Users can stake into subnet alpha token for leveraged exposure.
- **Sustainability**: Registration burn is one-time sunk cost; ongoing emissions fund servers/dev/community bounties.
- **Deflationary Pressure**: Burn contributes to TAO scarcity.

---

### 8. Cost Estimates (Feb 2026)
- **Registration Burn**: 887.48 TAO ≈ **$159,700** (@ $180/TAO conservative average).
- **Development** (2–3 full-time devs + part-time contributors, 3–4 months): $80k–$150k (or pay in TAO).
- **Infrastructure** (bootstrap servers, monitoring, testnet ops): $10k–$20k initial + $2k/month ongoing.
- **Marketing/Community** (bounties, X campaigns, TAO giveaways): $20k–$50k.
- **Legal/Audit** (optional smart-contract review, privacy audit): $15k–$30k.
- **Total Startup**: **$285k–$410k** USD equivalent.
- **Break-even**: At 2% emissions capture → ~144 TAO/day ≈ $26k/day revenue potential → registration repaid in <1 week at scale (conservative: 3–6 months).

Ongoing: Emissions cover everything after launch.

---

### 9. Timeline & Milestones
- **Week 1–2**: Finalize incentive spec, fork template, basic integration.
- **Week 3–6**: Testnet MVP, community alpha testing.
- **Week 7–10**: Mainnet registration + bootstrap.
- **Month 3+**: App v2 release with “Earn TAO” feature, dashboard, cross-subnet AI.
- **Month 6+**: Governance proposals, liquidity pools, partnerships.

---

### 10. Risks & Mitigations
- **High Registration Cost / Competition**: Mitigate by raising via community/VC aligned with mission (public-domain focus).
- **Low Initial Adoption**: Seed with existing Humanity users + TAO ecosystem airdrops.
- **Technical Integration Complexity**: Start simple (QoS only), iterate.
- **Regulatory**: Privacy-first (E2E + no content access) + voluntary → strong position.
- **Emissions Volatility**: Diversify with dTAO staking, multiple revenue streams.
- **Subnet Deregistration**: 4-month immunity + strong QoS = low risk.

---

### 11. Benefits & Broader Impact
- Accelerates mission: Ends isolation at global scale with economic incentives.
- Public good: All code remains CC0; subnet benefits entire Bittensor ecosystem.
- Attracts talent: Crypto natives + privacy advocates.
- Precedent: Shows how non-AI “real-world utility” subnets can thrive.

---

### 12. Next Steps (Immediate)
1. Push this doc to GitHub (united-humanity/humanity-chat or dedicated repo) as `bittensor_submet.md`.
2. Post link to @ttaovolution with thanks + call for feedback.
3. Open issues for community input on incentive tweaks.
4. Secure initial funding round (target $300k+ in TAO/USD mix) → then feed full spec to Grok/Claude for code generation.
5. Set up testnet wallet & experiment with template.

**Call to Action**: This is fully open for iteration. Comments, PRs, and co-authors welcome. Let’s make decentralized private communication economically unstoppable.

---

**References & Resources**  
- Bittensor Docs: https://docs.learnbittensor.org  
- Subnet Template: https://github.com/opentensor/bittensor-subnet-template  
- Live Metrics: https://taostats.io/subnets (current burn 887.48 TAO)  
- Humanity Chat Repo: (link to your existing)  

**Appendix A**: Full hyperparameter list available on request.  
**Appendix B**: Sample QoS probe pseudocode (will expand post-feedback).  

This document is CC0 — copy, modify, improve freely.  

Let’s build it. Feedback welcome — post it and tag me! Once funded, we feed the refined version straight into code generation.  