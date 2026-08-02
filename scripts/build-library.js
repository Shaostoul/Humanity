#!/usr/bin/env node
// Builds the in-app Library's data: copies a curated set of reference docs into
// data/library/ and generates data/library/index.json (the nested-category
// manifest the Library page reads). Re-run after editing or adding source docs.
//
// Data-driven (infinite-of-X): the app loads the manifest + the .md files at
// startup; adding a doc means dropping a file in data/library/ and adding a line
// here (or directly to index.json), never touching Rust. Source docs live in
// docs/; this ships a stable, curated user-facing copy in data/ (which is
// distributed next to the exe, where docs/ is not).

const fs = require('fs');
const path = require('path');

const LIB = 'data/library';

// Curated, nested categories. Sources are repo paths; only ones that exist are
// included (missing ones are reported, not fatal).
const CATEGORIES = [
  { name: 'Credits', docs: [
    { title: 'Credits and Thanks', src: 'CREDITS.md' },
  ]},
  // v0.1090.x mission layer (operator: supporting docs for the overall mission
  // of ending pollution, poverty, corruption, fraud, and tyranny in pursuit of
  // uniting humanity in peaceful harmony). Source: docs/mission/.
  { name: 'The Mission', docs: [
    // Canonical mission statement; the web /mission page forwards to this
    // doc's deep link (/library#humanitys-mission) so there is ONE mission
    // surface, rendered by both clients (merge decision 2026-08-01).
    { title: "Humanity's Mission", src: 'docs/mission/humanitys_mission.md' },
    { title: 'The Five Adversaries', src: 'docs/mission/the_five_adversaries.md' },
    { title: 'What Is a Good Human', src: 'docs/mission/what_is_a_good_human.md' },
    { title: 'What Is a Good AI', src: 'docs/mission/what_is_a_good_ai.md' },
    { title: 'Why Homesteading Works', src: 'docs/mission/why_homesteading_works.md' },
    { title: 'Interstellar Civilization Governance', src: 'docs/mission/interstellar_civilization_governance.md' },
  ]},
  // v0.1092.x outreach layer (operator: audience-tailored "how it helps"
  // docs to reach more people; hub-and-spokes, source docs/outreach/).
  { name: 'How It Helps', docs: [
    { title: 'How HumanityOS Helps', src: 'docs/outreach/how_humanityos_helps.md' },
    { title: 'For You', src: 'docs/outreach/for_individuals.md' },
    { title: 'For Families', src: 'docs/outreach/for_families.md' },
    { title: 'For Public Leaders', src: 'docs/outreach/for_public_leaders.md' },
    { title: 'For Funders and Sponsors', src: 'docs/outreach/for_funders_and_sponsors.md' },
    { title: 'For Affiliate Partners', src: 'docs/outreach/for_affiliate_partners.md' },
    { title: 'What to Expect: Playing', src: 'docs/outreach/what_to_expect_playing.md' },
  ]},
  // v0.1096.x real-skills curriculum, first rung (operator-approved arc:
  // fill the gap the outreach docs honestly admit; every number sourced from
  // agricultural extension / CDC / EPA guidance; source docs/user/skills/).
  { name: 'Real Skills', docs: [
    { title: 'Your First Tomato', src: 'docs/user/skills/your_first_tomato.md' },
    { title: 'Storing Water Safely', src: 'docs/user/skills/storing_water_safely.md' },
    { title: 'Your First Compost', src: 'docs/user/skills/your_first_compost.md' },
  ]},
  { name: 'The Accord', accord: true, docs: [
    { title: 'The Humanity Accord', src: 'docs/accord/humanity_accord.md' },
    { title: 'Absolute Prohibitions', src: 'docs/accord/absolute_prohibitions.md' },
  ]},
  { name: 'Your Rights and Consent', accord: true, docs: [
    { title: 'Rights and Responsibilities', src: 'docs/accord/rights_and_responsibilities.md' },
    { title: 'Consent and Control', src: 'docs/accord/consent_and_control.md' },
    { title: 'Communication and Association', src: 'docs/accord/communication_and_association.md' },
  ]},
  { name: 'How We Decide', accord: true, docs: [
    { title: 'Ethical Principles', src: 'docs/accord/ethical_principles.md' },
    { title: 'Governance Models', src: 'docs/accord/governance_models.md' },
    { title: 'Conflict Resolution', src: 'docs/accord/conflict_resolution.md' },
    { title: 'Transparency Guarantees', src: 'docs/accord/transparency_guarantees.md' },
    { title: 'Minimum Transparency Checklist', src: 'docs/accord/minimum_transparency_checklist.md' },
    { title: 'When Legitimacy Fails', src: 'docs/accord/failure_of_legitimacy.md' },
  ]},
  { name: 'Safety and Care', accord: true, docs: [
    { title: 'Human Needs', src: 'docs/accord/human_needs.md' },
    { title: 'Safety and Responsibility', src: 'docs/accord/safety_and_responsibility.md' },
    { title: 'Harm and Responsibility', src: 'docs/accord/harm_and_responsibility.md' },
    { title: 'Irreversible Actions', src: 'docs/accord/irreversible_actions.md' },
    { title: 'User Safety Overview', src: 'docs/accord/user_safety_overview.md' },
  ]},
  { name: 'Reference', accord: true, docs: [
    { title: 'Glossary', src: 'docs/accord/glossary.md' },
    { title: 'Scope and Boundaries', src: 'docs/accord/scope_boundaries.md' },
    { title: 'Knowledge Sources', src: 'docs/accord/knowledge_sources.md' },
    { title: 'Curriculum: How Humans Learn', src: 'docs/accord/curriculum.md' },
  ]},
  // v0.965.x expansion (operator: "can we get the library page filled with
  // all the docs, not just the accord? We essentially want to make it so
  // the only app people ever need to open again is HumanityOS").
  { name: 'Getting Started', docs: [
    { title: 'Welcome', src: 'docs/user/README.md' },
    { title: 'Getting Started', src: 'docs/user/getting-started.md' },
    { title: 'Onboarding Walkthrough', src: 'docs/user/ONBOARDING.md' },
  ]},
  { name: 'Creating Content', docs: [
    { title: 'What You Can Make', src: 'docs/user/creating/README.md' },
    { title: 'Add a Planet', src: 'docs/user/creating/planet.md' },
    { title: 'Add a Vehicle', src: 'docs/user/creating/vehicle.md' },
    { title: 'Add a Spaceship', src: 'docs/user/creating/spaceship.md' },
    { title: 'Add Furniture', src: 'docs/user/creating/furniture.md' },
    { title: 'Add a Plant', src: 'docs/user/creating/plant.md' },
    { title: 'Add a 3D Model', src: 'docs/user/creating/3d-model.md' },
    { title: 'Add a Sound', src: 'docs/user/creating/audio-file.md' },
    { title: 'Add a Recipe', src: 'docs/user/creating/recipe.md' },
    { title: 'Add a Quest', src: 'docs/user/creating/quest.md' },
    { title: 'Add a Room or Structure', src: 'docs/user/creating/room-structure.md' },
  ]},
  { name: 'Running Your Own Server', docs: [
    { title: 'Overview', src: 'docs/admin/README.md' },
    { title: 'Self-Hosting Guide', src: 'docs/admin/SELF-HOSTING.md' },
    { title: 'Distribution Mirrors', src: 'docs/admin/distribution-mirrors.md' },
    { title: 'Torrent Infrastructure', src: 'docs/admin/torrent-infrastructure.md' },
  ]},
  { name: 'For Contributors', docs: [
    { title: 'Start Here', src: 'docs/contributor/00-START-HERE.md' },
    { title: 'The Vision', src: 'docs/contributor/01-VISION.md' },
    { title: 'Architecture', src: 'docs/contributor/02-ARCHITECTURE.md' },
    { title: 'Module Map', src: 'docs/contributor/03-MODULE-MAP.md' },
    { title: 'Contributing', src: 'docs/contributor/04-CONTRIBUTING.md' },
    { title: 'Source of Truth Map', src: 'docs/contributor/06-SOURCE-OF-TRUTH-MAP.md' },
    { title: 'Development Loop', src: 'docs/contributor/development_loop.md' },
    { title: 'Validating Data Files', src: 'docs/contributor/validate_data.md' },
  ]},
  { name: 'How It Is Designed', docs: [
    { title: 'Design Overview', src: 'docs/design/DESIGN.md' },
    { title: 'Two Realities (Real and Sim)', src: 'docs/design/two-realities.md' },
    { title: 'Infinite of Everything', src: 'docs/design/infinite-of-x.md' },
    { title: 'The UI System', src: 'docs/design/ui-system.md' },
    { title: 'Gameplay Loop Map', src: 'docs/design/gameplay-loop-map.md' },
    { title: 'Progression, Skills, and Gear', src: 'docs/design/progression-skills-gear.md' },
    { title: 'The Cosmos', src: 'docs/design/cosmos-architecture.md' },
  ]},
  { name: 'Project', docs: [
    { title: 'Roadmap', src: 'docs/ROADMAP.md' },
    { title: 'AI Participation', src: 'docs/ai/onboarding.md' },
  ]},
];

fs.mkdirSync(LIB, { recursive: true });
const manifest = {
  _comment: 'In-app Library manifest. Generated by scripts/build-library.js. Nested categories of reference docs; the app reads each doc from data/library/<file>. Edit the script or this file to curate.',
  categories: [],
};
let copied = 0;
const missing = [];
const used = new Set();
for (const cat of CATEGORIES) {
  const entry = { name: cat.name, docs: [] };
  // The Accord subset is flagged so /accord can render exactly those
  // categories from THIS manifest instead of a separate relay endpoint.
  if (cat.accord) entry.accord = true;
  for (const d of cat.docs) {
    if (!fs.existsSync(d.src)) { missing.push(d.src); continue; }
    // Collision-safe target name: basename first; if taken (README.md
    // exists in four source folders), fall back to the full path slug.
    // The check is CASE-INSENSITIVE: ONBOARDING.md (docs/user) and
    // onboarding.md (docs/ai) are different keys to a Set but the SAME file
    // on Windows, so one silently overwrote the other while the manifest
    // listed both; the case-sensitive live server then 404ed the loser
    // (found by the 2026-07-31 doc-truth sweep).
    let file = path.basename(d.src);
    if (used.has(file.toLowerCase())) {
      file = d.src.replace(/^docs\//, '').replace(/[\/\\]/g, '-');
    }
    used.add(file.toLowerCase());
    fs.copyFileSync(d.src, path.join(LIB, file));
    entry.docs.push({ title: d.title, file });
    copied++;
  }
  if (entry.docs.length) manifest.categories.push(entry);
}
fs.writeFileSync(path.join(LIB, 'index.json'), JSON.stringify(manifest, null, 2) + '\n');
console.log(`library: copied ${copied} docs into ${LIB}/ across ${manifest.categories.length} categories`);
if (missing.length) console.log('  (skipped missing: ' + missing.join(', ') + ')');
