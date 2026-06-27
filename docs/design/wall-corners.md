# Wall corners with mismatched thickness (diagnosed, fix deferred)

> Operator bug (v0.575 launch test): two interior walls of very DIFFERENT thickness meeting at a
> corner show a "wide seam strip where the two walls overlap that doesn't look right" + z-fighting.
> Equal/similar thickness (granite 0.20 m both) is sharp. Tempered glass (0.012 m) + aluminum (0.04 m)
> is broken. NOTE the operator also said glass + GRANITE reads sharp — so it is NOT a pure
> "any-mismatch" rule; thin+thin vs thin+thick behave differently. This needs understanding before a fix.

## Diagnosis (from an investigation agent, verified by hand)

The miter is `wall_end_miter` (`src/ship/home_structure.rs` ~622). For a wall W ending at corner `c`
next to wall B, it cuts W's two footprint edges against B's two faces and returns (left, right).

For EQUAL thickness the result is the correct flush miter: left = inner-corner (W_inner ∩ B_inner),
right = outer-corner (W_outer ∩ B_outer). The miter tests (`miter_a_end_is_inner_and_outer_corner`,
`miter_corner_is_shared_flush_by_both_walls`) lock this and it looks right.

For MISMATCHED thickness the inner/outer corners are geometrically still inner∩inner and outer∩outer,
but they sit far apart in the wall-normal direction (≈ the THICKER wall's width) while the THIN wall is
only a few mm thick. So the thin wall's end QUAD is a long skewed sliver that **overlaps the thick
wall's body** — that overlap is coplanar with the thick wall's faces → **z-fighting**, and the sliver
is the "wide seam strip." There is also **no corner fill at a 2-wall join** (`corner_column` only fires
for `count >= 3`), so nothing covers it.

## Why the obvious fixes are wrong / risky

- **Swapping the `is_a_end` adj-face pairing** (the agent's first suggestion): hand-computation shows
  this CHANGES the equal-thickness corners too — it would regress the working granite-on-granite case
  and break the two miter tests. Do NOT do this blindly.
- **A blanket corner-fill cylinder for any thickness mismatch**: sized by the thickest wall, this would
  add a fat post at a glass+granite corner — which the operator says is ALREADY sharp. So a naive
  "mismatch → add column" rule overfixes.

## The real fix (needs careful, visually-iterated work)

Likely the right approach is to **clip the THINNER wall's mitered end back so it butts against the
thicker wall's near face instead of overlapping its body** (promote the v0.566 mid-span T-clip idea to
the shared-corner case), possibly plus a small corner-fill ONLY when the thin wall's end would
otherwise leave a gap. The key open questions to resolve against real configs first:
1. Why does glass+granite read sharp but glass+aluminum broken? (Reproduce both, inspect the meshes.)
2. Should the thin wall butt to the thick wall's face (loses the mitered look on the thin side) or
   should both keep a true bisector and a fill quad span the union? Get the operator's eye on a
   side-by-side.
3. Whatever the rule, it must NOT change the equal-thickness miter (keep the tests green).

This is deferred deliberately: a rushed change here regresses the corners that currently work, which is
worse than the present mismatched-thickness seam. Implement it as a focused effort with screenshots of
specific material pairs, not from a headless guess.
