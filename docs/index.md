# Docs Index

Comprehensive navigation hub for all Humanity documentation.

---

## Getting Started (numbered guides)

- [00-START-HERE](00-START-HERE.md) -- First-day orientation: what the project is and how it works
- [01-VISION](01-VISION.md) -- Mission statement and product shape
- [02-ARCHITECTURE](02-ARCHITECTURE.md) -- Cargo workspace layout and target architecture
- [03-MODULE-MAP](03-MODULE-MAP.md) -- Module intent definitions in plain language
- [04-CONTRIBUTING](04-CONTRIBUTING.md) -- How humans and AIs contribute safely
- [05-AI-ONBOARDING](05-AI-ONBOARDING.md) -- Minimum context for AI agents to start useful work
- [06-SOURCE-OF-TRUTH-MAP](06-SOURCE-OF-TRUTH-MAP.md) -- Links vision to design docs to implementation status
- [07-MODULE-SPEC-TEMPLATE](07-MODULE-SPEC-TEMPLATE.md) -- Template for every gameplay/learning module
- [08-V1-MODULE-BACKBONE](08-V1-MODULE-BACKBONE.md) -- First executable backbone for "teach everything" trajectory
- [09-LIFEFORM-PARITY-FRAMEWORK](09-LIFEFORM-PARITY-FRAMEWORK.md) -- Modeling non-human lifeforms with systemic depth

## Top-level Documents

- [DESIGN.md](DESIGN.md) -- Design as binding technical law
- [AGENTS.md](AGENTS.md) -- AI workspace orientation and session protocol
- [BOOTSTRAP.md](BOOTSTRAP.md) -- Fresh-workspace initialization for new agents
- [ONBOARDING.md](ONBOARDING.md) -- New contributor onboarding guide
- [OPERATING_CONTRACT.md](OPERATING_CONTRACT.md) -- Operational behaviors that persist across sessions
- [SELF-HOSTING.md](SELF-HOSTING.md) -- Run your own Humanity server in under 10 minutes
- [README.md](README.md) -- Folder-level overview and canonical design doc list
- [roadmap.md](roadmap.md) -- Canonical feature priority list (living document)
- [feature_web.md](feature_web.md) -- Interactive teaching-first feature graph design
- [action_log.md](action_log.md) -- Canonical action log format for deterministic replays
- [earth_fleet_twin.md](earth_fleet_twin.md) -- Earth + Fleet Twin architecture (local-first)
- [header_navigation_architecture.md](header_navigation_architecture.md) -- Proposed header nav restructure
- [humanity_full_replacement_blueprint.md](humanity_full_replacement_blueprint.md) -- Full-replacement blueprint (open draft)
- [knowledge-gardening.md](knowledge-gardening.md) -- Self-sustaining homestead plant growing design
- [market_integration_architecture.md](market_integration_architecture.md) -- In-game shopping and media integration (legal + practical)
- [md_information_architecture_plan.md](md_information_architecture_plan.md) -- Markdown folder organization plan
- [our_homesteading_future.md](our_homesteading_future.md) -- 2026 MVP vision: aeroponic homesteading
- [validate_data.md](validate_data.md) -- Data validation gate for simulation, replay, and merge

---

## Core Design (`core/`)

Authoritative technical law -- foundational constraints that all systems must obey.

- [README.md](core/README.md) -- Core design overview
- [accord_constraints.md](core/accord_constraints.md) -- How the Humanity Accord constrains design
- [ai_interface.md](core/ai_interface.md) -- AI authority limits, access rules, failure handling
- [architecture.md](core/architecture.md) -- Authority separation, data flow, execution structure
- [asset_rules.md](core/asset_rules.md) -- Constraints on models, textures, audio, assets
- [data_model.md](core/data_model.md) -- Rules for representing reality as structured data
- [distributed_consciousness.md](core/distributed_consciousness.md) -- Distributed consciousness model
- [economy_model.md](core/economy_model.md) -- Material, energy, labor, and time flow constraints
- [education_model.md](core/education_model.md) -- How learning is represented and validated
- [engine_entrypoints.md](core/engine_entrypoints.md) -- Engine entry points
- [epistemology.md](core/epistemology.md) -- Theory of knowledge within the system
- [fibonacci-scope.md](core/fibonacci-scope.md) -- Fibonacci scope tiers (self to cosmos)
- [foundation_decisions_v1.md](core/foundation_decisions_v1.md) -- Foundation decisions v1
- [invariants_and_tests.md](core/invariants_and_tests.md) -- System invariants and test requirements
- [memory_structure.md](core/memory_structure.md) -- Memory structure design
- [realism_constraints.md](core/realism_constraints.md) -- Reality-first constraints and abstraction limits
- [simulation_laws.md](core/simulation_laws.md) -- Determinism, causality, time, conservation rules
- [system_diagrams.md](core/system_diagrams.md) -- System architecture diagrams
- [system_inventory.md](core/system_inventory.md) -- Non-binding catalog of candidate systems
- [terminology.md](core/terminology.md) -- Technical terms used across design and simulation
- [testing_philosophy.md](core/testing_philosophy.md) -- What correctness means and how it is verified
- [tiered_ai_system.md](core/tiered_ai_system.md) -- Tiered AI system design
- [transparency_hooks.md](core/transparency_hooks.md) -- Transparency hook definitions
- [ui_invariants.md](core/ui_invariants.md) -- UI invariants and constraints

## Design Specs (`design/`)

High-level design documents for major subsystems.

- [audio-engine.md](design/audio-engine.md) -- Audio engine design
- [game-engine.md](design/game-engine.md) -- Game engine design
- [gardening-game.md](design/gardening-game.md) -- Gardening game design
- [graphics-pipeline.md](design/graphics-pipeline.md) -- Graphics pipeline design
- [maps-multi-scale.md](design/maps-multi-scale.md) -- Multi-scale map system
- [vr-tracking.md](design/vr-tracking.md) -- VR tracking design

## Gameplay (`gameplay/`)

Player-facing mechanics and interaction systems.

- [README.md](gameplay/README.md) -- Gameplay design overview
- [camera_modes_architecture.md](gameplay/camera_modes_architecture.md) -- Camera mode system architecture
- [first_third_person_traversal.md](gameplay/first_third_person_traversal.md) -- First/third person traversal mechanics
- [gardening.md](gameplay/gardening.md) -- Gardening gameplay system
- [quest_engine_architecture.md](gameplay/quest_engine_architecture.md) -- Quest engine architecture
- [ship_zoning_transit.md](gameplay/ship_zoning_transit.md) -- Ship zoning and transit systems

## Engine (`engine/`)

Renderer, performance, and asset pipeline specs.

- [asset_budget_policy.md](engine/asset_budget_policy.md) -- Asset budget policy and limits
- [custom_rust_wgpu_runtime_plan.md](engine/custom_rust_wgpu_runtime_plan.md) -- Custom Rust + wgpu runtime plan
- [performance_targets.md](engine/performance_targets.md) -- Performance target definitions
- [planet_icosphere_voxel_plan.md](engine/planet_icosphere_voxel_plan.md) -- Planet icosahedral + voxel rendering plan
- [renderer_architecture.md](engine/renderer_architecture.md) -- Renderer architecture
- [streaming_bundle_format.md](engine/streaming_bundle_format.md) -- Streaming asset bundle format
- [visual_downgrade_matrix.md](engine/visual_downgrade_matrix.md) -- Visual downgrade tiers for lower-end hardware

## Game (`game/`)

Game modes, sessions, and world design.

- [celestial_navigation.md](game/celestial_navigation.md) -- Celestial navigation system
- [character_customization_roadmap.md](game/character_customization_roadmap.md) -- Character customization roadmap
- [cli_playtest_mode.md](game/cli_playtest_mode.md) -- CLI playtest mode
- [difficulty_fidelity_matrix.md](game/difficulty_fidelity_matrix.md) -- Difficulty and fidelity matrix
- [first_person_controller_contract.md](game/first_person_controller_contract.md) -- First-person controller contract
- [humanity_one.md](game/humanity_one.md) -- Humanity One ship/world design
- [intro_sequence.md](game/intro_sequence.md) -- Intro sequence design
- [offline_playable_no_combat_milestone.md](game/offline_playable_no_combat_milestone.md) -- Offline playable no-combat milestone
- [offline_world_loop_scaffold.md](game/offline_world_loop_scaffold.md) -- Offline world loop scaffold
- [session_modes.md](game/session_modes.md) -- Session modes and authority model

## Network & API (`network/`)

Networking, sync, federation, and protocol specs.

- [api_and_endpoints.md](network/api_and_endpoints.md) -- API and endpoint reference
- [architecture.md](network/architecture.md) -- Network architecture overview
- [authority_model.md](network/authority_model.md) -- Authority model for state ownership
- [file_sharing.md](network/file_sharing.md) -- File sharing protocol
- [hybrid_replication.md](network/hybrid_replication.md) -- Hybrid replication strategy
- [indexing.md](network/indexing.md) -- Network indexing design
- [membership_and_roles.md](network/membership_and_roles.md) -- Membership and role management
- [memory_sync.md](network/memory_sync.md) -- Memory sync protocol
- [notifications_model.md](network/notifications_model.md) -- Notifications model
- [object_format.md](network/object_format.md) -- Canonical object format
- [object_type_schemas.md](network/object_type_schemas.md) -- Object type schema definitions
- [offline_first_sync.md](network/offline_first_sync.md) -- Offline-first sync strategy
- [protocol_versioning.md](network/protocol_versioning.md) -- Protocol versioning rules
- [realtime_relay_protocol.md](network/realtime_relay_protocol.md) -- Real-time relay protocol spec
- [realtime_transport.md](network/realtime_transport.md) -- Real-time transport layer
- [scope.md](network/scope.md) -- Network scope definitions
- [server_federation.md](network/server_federation.md) -- Server federation design
- [snapshot_delta_recovery.md](network/snapshot_delta_recovery.md) -- Snapshot + delta recovery
- [social_graph.md](network/social_graph.md) -- Social graph model
- [space_creation_and_governance_objects.md](network/space_creation_and_governance_objects.md) -- Space creation and governance objects
- [space_policy_format.md](network/space_policy_format.md) -- Space policy format spec
- [tailnet_onboarding.md](network/tailnet_onboarding.md) -- Tailnet onboarding flow
- [transport_security.md](network/transport_security.md) -- Transport security layer
- [unified_comms_sidebar_plan.md](network/unified_comms_sidebar_plan.md) -- Unified comms sidebar plan
- [voice_video_streaming.md](network/voice_video_streaming.md) -- Voice and video streaming
- [web_client_constraints.md](network/web_client_constraints.md) -- Web client constraints

## Security (`security/`)

Threat models, encryption, and privacy architecture.

- [README.md](security/README.md) -- Security design overview
- [encryption_and_confidentiality.md](security/encryption_and_confidentiality.md) -- Encryption and confidentiality spec
- [private_space_key_management.md](security/private_space_key_management.md) -- Private space key management
- [retention_and_deletion_semantics.md](security/retention_and_deletion_semantics.md) -- Data retention and deletion semantics
- [secure_communication_constraints.md](security/secure_communication_constraints.md) -- Secure communication constraints
- [security_and_privacy_architecture.md](security/security_and_privacy_architecture.md) -- Security and privacy architecture
- [threat_model.md](security/threat_model.md) -- Threat model
- [use_of_force_constraints.md](security/use_of_force_constraints.md) -- Use-of-force constraints
- [voting_integrity_constraints.md](security/voting_integrity_constraints.md) -- Voting integrity constraints

## Identity (`identity/`)

Key management, sessions, and account recovery.

- [keys_and_sessions.md](identity/keys_and_sessions.md) -- Key management and session design
- [recovery_and_backups.md](identity/recovery_and_backups.md) -- Recovery and backup procedures

## Economy (`economy/`)

In-game and real-world economic systems.

- [crypto_exchange.md](economy/crypto_exchange.md) -- Crypto payment layer design
- [world_resources.md](economy/world_resources.md) -- World resource model

## Storage (`storage/`)

Persistence, save formats, and local data layout.

- [backup_bundle_format.md](storage/backup_bundle_format.md) -- Backup bundle format
- [local_storage_layout.md](storage/local_storage_layout.md) -- Local storage layout
- [sqlite_save_backend.md](storage/sqlite_save_backend.md) -- SQLite save backend design

## UI (`ui/`)

App shell, navigation, and menu architecture.

- [README.md](ui/README.md) -- UI design overview
- [app_shell_information_architecture.md](ui/app_shell_information_architecture.md) -- App shell information architecture
- [header_dropdown_navigation.md](ui/header_dropdown_navigation.md) -- Header dropdown navigation design
- [knowledge_tab_architecture.md](ui/knowledge_tab_architecture.md) -- Knowledge tab architecture
- [menu_submenu_matrix.md](ui/menu_submenu_matrix.md) -- Menu and submenu matrix

## Modules (`modules/`)

Gameplay/learning module specs -- each defines a bounded system.

- [README.md](modules/README.md) -- Module specs overview
- [EXECUTION-BOARD-V1.md](modules/EXECUTION-BOARD-V1.md) -- Execution board v1
- [core-lifeform-model.md](modules/core-lifeform-model.md) -- Core lifeform model
- [core-skill-progression.md](modules/core-skill-progression.md) -- Core skill progression system
- [core-teaching-graph.md](modules/core-teaching-graph.md) -- Core teaching graph
- [module-carpentry.md](modules/module-carpentry.md) -- Carpentry module
- [module-crop-systems.md](modules/module-crop-systems.md) -- Crop systems module
- [module-electrical-basics.md](modules/module-electrical-basics.md) -- Electrical basics module
- [module-health-first-aid.md](modules/module-health-first-aid.md) -- Health and first aid module
- [module-plumbing-basics.md](modules/module-plumbing-basics.md) -- Plumbing basics module
- [module-soil-ecology.md](modules/module-soil-ecology.md) -- Soil ecology module
- [module-water-systems.md](modules/module-water-systems.md) -- Water systems module

## Schemas (`schemas/`)

Data shape contracts consumed by systems.

- [constructibles.md](schemas/constructibles.md) -- Constructible object schemas
- **entities/** -- [plant.schema.md](schemas/entities/plant.schema.md), [substrate.schema.md](schemas/entities/substrate.schema.md)
- **items/** -- [harvest_output.schema.md](schemas/items/harvest_output.schema.md)
- **plots/** -- [plot.schema.md](schemas/plots/plot.schema.md)
- **resources/** -- [nutrients.schema.md](schemas/resources/nutrients.schema.md), [water.schema.md](schemas/resources/water.schema.md)

## Systems (`systems/`)

Bounded system specifications obeying foundational design law.

- [README.md](systems/README.md) -- Systems overview
- [construction.md](systems/construction.md) -- Construction system spec
- **construction/** -- [README.md](systems/construction/README.md), [processes.md](systems/construction/processes.md), [states.md](systems/construction/states.md)
- [farming.md](systems/farming.md) -- Farming system spec
- **farming/** -- [README.md](systems/farming/README.md), [processes.md](systems/farming/processes.md), [states.md](systems/farming/states.md)

## Page Specs (`page-specs/`)

Per-page design specs for the web/desktop UI.

- [README.md](page-specs/README.md) -- Page specs overview
- [calendar.md](page-specs/calendar.md) -- Calendar page
- [equipment.md](page-specs/equipment.md) -- Equipment page
- [h_dashboard.md](page-specs/h_dashboard.md) -- Dashboard page
- [inventory.md](page-specs/inventory.md) -- Inventory page
- [knowledge.md](page-specs/knowledge.md) -- Knowledge page
- [learn.md](page-specs/learn.md) -- Learn page
- [logbook.md](page-specs/logbook.md) -- Logbook page
- [maps.md](page-specs/maps.md) -- Maps page
- [market.md](page-specs/market.md) -- Market page
- [network.md](page-specs/network.md) -- Network page
- [ops.md](page-specs/ops.md) -- Ops page
- [profile.md](page-specs/profile.md) -- Profile page
- [quests.md](page-specs/quests.md) -- Quests page
- [skills.md](page-specs/skills.md) -- Skills page
- [streams.md](page-specs/streams.md) -- Streams page
- [systems.md](page-specs/systems.md) -- Systems page
- [utility_account.md](page-specs/utility_account.md) -- Account utility page
- [utility_alerts.md](page-specs/utility_alerts.md) -- Alerts utility page
- [utility_data.md](page-specs/utility_data.md) -- Data utility page
- [utility_search.md](page-specs/utility_search.md) -- Search utility page
- [utility_settings.md](page-specs/utility_settings.md) -- Settings utility page

## RFCs (`rfc/`)

Proposals for significant changes.

- [README.md](rfc/README.md) -- RFC process overview
- [RFC_TEMPLATE.md](rfc/RFC_TEMPLATE.md) -- RFC template
- [p2p_relay_continuity_rfc.md](rfc/p2p_relay_continuity_rfc.md) -- P2P relay continuity proposal
- [studio_mode_rfc.md](rfc/studio_mode_rfc.md) -- Studio mode proposal

## Decisions (`decisions/`)

Architecture Decision Records (ADRs).

- [README.md](decisions/README.md) -- ADR process overview
- [ADR-0001-modular-boundaries.md](decisions/ADR-0001-modular-boundaries.md) -- Modular boundaries
- [canonical_encoding_and_hashing.md](decisions/canonical_encoding_and_hashing.md) -- Canonical encoding and hashing
- [client_side_identity_keys.md](decisions/client_side_identity_keys.md) -- Client-side identity keys
- [hybrid_network.md](decisions/hybrid_network.md) -- Hybrid network architecture
- [signed_moderation_logs.md](decisions/signed_moderation_logs.md) -- Signed moderation logs
- [two_timeline_offline_model.md](decisions/two_timeline_offline_model.md) -- Two-timeline offline model

## Operations (`operations/`)

Runbooks, checklists, and operational procedures.

- [ECOSYSTEM_RELEASE_CHECKLIST.md](operations/ECOSYSTEM_RELEASE_CHECKLIST.md) -- Ecosystem release checklist
- [OPENCLAW_CONFIG_CHANGE_POLICY.md](operations/OPENCLAW_CONFIG_CHANGE_POLICY.md) -- OpenClaw config change policy
- [OPERATIONS_RUNBOOK.md](operations/OPERATIONS_RUNBOOK.md) -- Operations runbook
- [openclaw-integration.md](operations/openclaw-integration.md) -- OpenClaw integration guide

## Moderation (`moderation/`)

Governance and moderation action schemas.

- [governance.md](moderation/governance.md) -- Moderation governance model
- [moderation_action_schema.md](moderation/moderation_action_schema.md) -- Moderation action schema

## Product (`product/`)

Vision, roadmap, and ecosystem architecture.

- [README.md](product/README.md) -- Product overview
- [ecosystem_architecture.md](product/ecosystem_architecture.md) -- Ecosystem architecture
- [mvp_feature_spec.md](product/mvp_feature_spec.md) -- MVP feature spec
- [open_questions.md](product/open_questions.md) -- Open questions
- [product_roadmap.md](product/product_roadmap.md) -- Product roadmap
- [project_universe_integration.md](product/project_universe_integration.md) -- Project Universe integration
- [vision.md](product/vision.md) -- Product vision

## Runtime (`runtime/`)

Update distribution, hot reload, and native/web boundaries.

- [README.md](runtime/README.md) -- Runtime overview
- [ai_plugin_runtime_architecture.md](runtime/ai_plugin_runtime_architecture.md) -- AI plugin runtime architecture
- [hot_reload_tiers.md](runtime/hot_reload_tiers.md) -- Hot reload tier definitions
- [staged_download_and_trusted_seeding.md](runtime/staged_download_and_trusted_seeding.md) -- Staged download and trusted seeding
- [update_distribution_architecture.md](runtime/update_distribution_architecture.md) -- Update distribution architecture
- [web_vs_native_capabilities.md](runtime/web_vs_native_capabilities.md) -- Web vs native capability boundary

## Abuse Prevention (`abuse/`)

Anti-spam and proof-of-work protections.

- [anti_spam.md](abuse/anti_spam.md) -- Anti-spam measures
- [proof_of_work_stamps.md](abuse/proof_of_work_stamps.md) -- Proof-of-work stamp system

## Conformance (`conformance/`)

Canonical encoding rules and test vectors.

- [canonical_cbor_rules.md](conformance/canonical_cbor_rules.md) -- Canonical CBOR encoding rules
- [reference_implementation.md](conformance/reference_implementation.md) -- Reference implementation
- **test_vectors/** -- [README.md](conformance/test_vectors/README.md), [block_hash_vectors.md](conformance/test_vectors/block_hash_vectors.md), [object_hash_and_signature_vectors.md](conformance/test_vectors/object_hash_and_signature_vectors.md)

## Concepts (`concepts/`)

Research directions and exploratory ideas.

- [bittensor_subnet.md](concepts/bittensor_subnet.md) -- Bittensor subnet exploration
- [to_research.md](concepts/to_research.md) -- Topics to research
- [user_protection_methods.md](concepts/user_protection_methods.md) -- User protection methods

## Game Integration (`game_integration/`)

Rules for merging game and social state.

- [merge_rules_examples.md](game_integration/merge_rules_examples.md) -- Merge rules examples
- [social_vs_game_state_boundary.md](game_integration/social_vs_game_state_boundary.md) -- Social vs game state boundary

## Maps (`maps/`)

Geographic and spatial data.

- [geographic-data.md](maps/geographic-data.md) -- Geographic data sources and formats

## Database Concepts (`database_concepts/`)

SurrealDB schema drafts organized by domain: beings, comms, engineering, entities, health, humanities, mathematics, morality, people, resources, science, technology, users. ~100 `.surql` files defining table structures.

---

## Governance (`../accord/`)

The Humanity Accord -- human principles and ethics that constrain all design.

- [README.md](../accord/README.md) -- Accord overview
- [humanity_accord.md](../accord/humanity_accord.md) -- The Humanity Accord (full text)
- [ethical_principles.md](../accord/ethical_principles.md) -- Ethical principles
- [human_needs.md](../accord/human_needs.md) -- Human needs framework
- [rights_and_responsibilities.md](../accord/rights_and_responsibilities.md) -- Rights and responsibilities
- [governance_models.md](../accord/governance_models.md) -- Governance models
- [conflict_resolution.md](../accord/conflict_resolution.md) -- Conflict resolution
- [consent_and_control.md](../accord/consent_and_control.md) -- Consent and control
- [transparency_guarantees.md](../accord/transparency_guarantees.md) -- Transparency guarantees
- [absolute_prohibitions.md](../accord/absolute_prohibitions.md) -- Absolute prohibitions
- [harm_and_responsibility.md](../accord/harm_and_responsibility.md) -- Harm and responsibility
- [irreversible_actions.md](../accord/irreversible_actions.md) -- Irreversible actions
- [safety_and_responsibility.md](../accord/safety_and_responsibility.md) -- Safety and responsibility
- [failure_of_legitimacy.md](../accord/failure_of_legitimacy.md) -- Failure of legitimacy
- [scope_boundaries.md](../accord/scope_boundaries.md) -- Scope boundaries
- [communication_and_association.md](../accord/communication_and_association.md) -- Communication and association
- [curriculum.md](../accord/curriculum.md) -- Curriculum guidelines
- [glossary.md](../accord/glossary.md) -- Glossary of terms
- [knowledge_sources.md](../accord/knowledge_sources.md) -- Knowledge sources
- [minimum_transparency_checklist.md](../accord/minimum_transparency_checklist.md) -- Minimum transparency checklist
- [user_safety_overview.md](../accord/user_safety_overview.md) -- User safety overview
