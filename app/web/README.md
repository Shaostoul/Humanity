# app/web is GENERATED. Do not edit anything here.

Every file in this directory is a copy written by `scripts/bundle-web.js`,
which runs inside `just ship`. Editing a file here changes nothing: your edit
is overwritten by the next ship, and this tree is not served to anyone.

## Where to edit instead

The sources are `web/shared/`, `web/chat/`, `web/pages/` and `assets/icons/`.
The LIVE site is laid out by `scripts/sync-web-root.sh`, which reads `web/` and
never looks at this directory.

## What this is for

An in-app OFFLINE web bundle, so the desktop app could serve these pages with no
network. The bundle is built and committed, but nothing consumes it yet: the
feature is half-built.

Whether to finish it (regenerate and wire it up) or drop it (remove this tree,
`scripts/bundle-web.js`, and the `bundle-web` lines in the Justfile) is an open
decision recorded in `docs/PRIORITIES.md`. It is deliberately not being made by
whoever is passing through, because dropping it deletes a feature rather than
tidying junk.
