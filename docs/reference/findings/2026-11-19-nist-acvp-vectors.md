# May HumanityOS keep NIST's ACVP test vectors in its repository?

**Researched: 2026-11-19.** Not legal advice: the author is not a lawyer, and this records what
the sources say, for a person to weigh.

## The question

May the project commit a cut-down copy of NIST's official ML-KEM and ML-DSA test vectors (the
ACVP-Server `internalProjection.json` files) to its public repository, and on what conditions?

## The short answer

Yes, with conditions the copy now meets: keep NIST's notice intact, say that the files were
changed and when and how, and acknowledge NIST as the source. The notice adds that NIST
employees' software is not subject to copyright protection within the United States. The copy is
`scripts/tests/fixtures/acvp/` with its `NOTICE.md`.

## The findings

1. **The notice's conditions.** Source: the "License" section of the ACVP-Server README,
   https://github.com/usnistgov/ACVP-Server, read through GitHub's API at commit
   `975de31eb83d87039ec88934fdc47d8c312b892d` (committed 2026-08-12, "RELEASE/v1.1.0.43 including
   hotfix patches"), the commit the vectors were taken from. The repository has no separate
   licence file (GitHub's licence endpoint answers 404). Quotation: "You may use, copy, and
   distribute copies of the software in any medium, provided that you keep intact this entire
   notice." and "Modified works should carry a notice stating that you changed the software and
   should note the date and nature of any such change. Please explicitly acknowledge the National
   Institute of Standards and Technology as the source of the software."
2. **Copyright status in the United States.** Same source, same date. Quotation: "The software
   developed by NIST employees is not subject to copyright protection within the United States."
3. **The files themselves.** Each `internalProjection.json` was taken from
   `gen-val/json-files/<vector set>/` at that commit; the sizes GitHub lists match the copies
   (for example ML-KEM-keyGen-FIPS203's 558,841 bytes), and each fixture records its source
   file's SHA-256.

## Still unknown

- Whether the notice, written for "software", is meant to cover test-vector data files too. The
  project follows it either way, which costs nothing; a question to the contact the README names
  would settle it.
- How the files are treated outside the United States: the notice speaks of copyright "within the
  United States". A lawyer in the relevant country would settle it; following the notice's
  conditions everywhere is the cautious reading.

## Sources not reached

None.

## What it means for this project

**Certain:** the notice allows copying and distributing modified copies on the three conditions
quoted above; `scripts/tests/fixtures/acvp/NOTICE.md` reproduces the whole notice, states the
change (only the ML-KEM-768 and ML-DSA-65 groups kept, the pre-hash groups left out, on
2026-11-19) and acknowledges NIST, and each fixture repeats its source, commit and SHA-256.

**Judgement call:** treating the vectors as covered by the notice (rather than as uncopyrighted
data needing nothing) is the safer reading and is what the project does.

Not legal advice: the author is not a lawyer.
