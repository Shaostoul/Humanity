# NIST ACVP test vectors: source and notice

The JSON files in this folder are NIST's official ML-KEM and ML-DSA test vectors (FIPS 203,
FIPS 204), the `internalProjection.json` files of NIST's ACVP-Server repository,
https://github.com/usnistgov/ACVP-Server, at commit `975de31eb83d87039ec88934fdc47d8c312b892d`
(committed 2026-08-12, "RELEASE/v1.1.0.43 including hotfix patches"), under
`gen-val/json-files/`.

**Source: the National Institute of Standards and Technology.**

**Changed (2026-11-19, by `scripts/acvp-extract.mjs`):** each file keeps only the test groups of
the parameter set HumanityOS uses (ML-KEM-768, ML-DSA-65), less the HashML-DSA (pre-hash) groups
that neither implementation here can run, which each file lists in `left_out` with the reason.
Every kept group and test is unchanged. Each file records its source path, the commit and the
source file's SHA-256. What runs them: `crates/humanity-protocol/src/pq_acvp_tests.rs` and
`scripts/tests/pq-acvp.test.js`. The licence question is recorded in
`docs/reference/findings/2026-11-19-nist-acvp-vectors.md`.

## The notice, as the ACVP-Server README gives it (its "License" section, read 2026-11-19 at that commit)

NIST-developed software is provided by NIST as a public service. You may use, copy, and distribute copies of the software in any medium, provided that you keep intact this entire notice. You may improve, modify, and create derivative works of the software or any portion of the software, and you may copy and distribute such modifications or works. Modified works should carry a notice stating that you changed the software and should note the date and nature of any such change. Please explicitly acknowledge the National Institute of Standards and Technology as the source of the software. 

NIST-developed software is expressly provided "AS IS." NIST MAKES NO WARRANTY OF ANY KIND, EXPRESS, IMPLIED, IN FACT, OR ARISING BY OPERATION OF LAW, INCLUDING, WITHOUT LIMITATION, THE IMPLIED WARRANTY OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE, NON-INFRINGEMENT, AND DATA ACCURACY. NIST NEITHER REPRESENTS NOR WARRANTS THAT THE OPERATION OF THE SOFTWARE WILL BE UNINTERRUPTED OR ERROR-FREE, OR THAT ANY DEFECTS WILL BE CORRECTED. NIST DOES NOT WARRANT OR MAKE ANY REPRESENTATIONS REGARDING THE USE OF THE SOFTWARE OR THE RESULTS THEREOF, INCLUDING BUT NOT LIMITED TO THE CORRECTNESS, ACCURACY, RELIABILITY, OR USEFULNESS OF THE SOFTWARE.

You are solely responsible for determining the appropriateness of using and distributing the software and you assume all risks associated with its use, including but not limited to the risks and costs of program errors, compliance with applicable laws, damage to or loss of data, programs or equipment, and the unavailability or interruption of operation. This software is not intended to be used in any situation where a failure could cause risk of injury or damage to property. The software developed by NIST employees is not subject to copyright protection within the United States.
