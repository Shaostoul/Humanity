# Asset Rules

## Purpose

This document defines invariant rules for organizing, naming, storing, and referencing assets.

Assets are dependencies. Disorder creates silent breakage, duplication, and loss of trust.

---

## Authority

Assets are authoritative only when:
- stored under the repo `assets/` directory (repo-wide assets), or `website/assets/` (website-only assets)
- referenced by stable relative paths
- not duplicated across domains

No system may fork asset copies into other directories to “make it convenient.”

---

## Canonical Asset Structure (repo)

The repo-wide canonical structure is:

assets/
  ui/
    icons/
    cursors/
    fonts/
  art/
    models/
    textures/
    materials/
    shaders/
  audio/
    sfx/
    music/
    voice/
  docs_media/
    diagrams/
    screenshots/

Rules:
- Add subfolders only when scale forces it.
- Prefer “category first” over “project first.”
- Do not create parallel folder trees for the same asset type.

---

## Website Asset Structure (site)

Website-only assets live under:

website/assets/
  css/
  img/
  js/        (only if needed)
  fonts/     (only if needed)

Website assets must not become canonical sources for the repository.

---

## Naming

- lowercase only
- hyphen-separated
- no spaces
- no version numbers in filenames

Versioning belongs in metadata or git history, not paths.

---

## Preferred Formats

- Models: .glb
- Textures: .png, .webp
- Audio: .opus
- Diagrams: .svg

Other formats require justification.

---

## Referencing

- Use relative paths.
- Never reference absolute local filesystem paths.
- Do not embed large binary blobs into markdown.
- Do not duplicate assets to satisfy different consumers. Fix the consumer.

---

## Prohibited Practices

- Duplicate copies of the same asset in multiple directories
- Silent replacement of an asset without noting the change
- Renaming without updating all references
- Storing repo assets outside `assets/`
- Storing canonical project assets inside `website/assets/`

---

## Enforcement

Any change that violates these rules is invalid and must be corrected before integration.
