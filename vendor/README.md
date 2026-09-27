# vendor/

Third-party crates this repo builds from a patched copy instead of from
crates.io. Each one is wired in through `[patch.crates-io]` in the root
`Cargo.toml`, keeps its own licence file, and is listed here with what was
changed, why, and how to go back to the published crate.

## rav1d 1.1.0 (`vendor/rav1d/`)

**What it is.** rav1d, the memory-safe Rust port of the dav1d AV1 decoder
(<https://github.com/memorysafety/rav1d>), BSD-2-Clause (`rav1d/COPYING`). The
game decodes video on in-world screens with it (`src/media/`). Copied from the
crates.io 1.1.0 package on 2026-09-27.

**What was left out.** Only what our build never compiles: the hand-written
assembly under `src/arm/`, `src/x86/` and `src/ext/` (we build with
`default-features = false`, so the `asm` feature and its `build.rs` path are
off), and the upstream CI files. Everything that builds is byte-for-byte the
published crate except the one patch below. About 2.1 MB instead of 11 MB.

**The patch (one function, `src/cdef.rs` `padding`, marked "HumanityOS
patch").** When CDEF pads a block, it copied the two rows above and below the
block from the CDEF line buffer by borrowing columns `0..x_end` of each row,
starting two pixels left of the block. At the frame's left edge (no
`HAVE_LEFT`) it only reads columns `2..x_end`, but it still borrowed the two
before, and in the line buffer those two belong to the end of the previous
superblock row's region. With rav1d's worker threads on, another thread can be
writing that region at the same moment (`backup2lines` in `src/cdef_apply.rs`),
so a shared borrow overlapped a live mutable one. The debug build's
`DisjointMut` checker caught it and panicked a rav1d worker inside a function
that cannot unwind, which aborted the whole test process
(`STATUS_STACK_BUFFER_OVERRUN`); a release build compiles the checker out and
keeps the overlapping borrow. No pixel was read while it was written (the two
columns are borrowed, never read), but a shared reference over memory another
thread holds mutably is undefined behaviour in Rust whether or not it is read.
The patch borrows exactly the columns it reads, `x_start..x_end`, for the top
rows and the bottom rows. Full account and the measurements: docs/BUGS.md,
BUG-093.

**Measured, 2026-09-27 (Windows, 12 logical processors, debug test build).**
`media::tests::rav1d_threaded_decode_stress` (six decoders at once, rav1d's
automatic threads, 40 decodes each): the published crate aborted in 8 of 20
runs, every time with the same two-column overlap; this copy, 0 of 20. The
fixture decodes to the same bytes with and without the patch, and threaded to
the same bytes as single-threaded (`threaded_decode_is_bit_identical_to_single_threaded`,
hash `bc2eee527943655c` in both builds).

**Upstream.** rav1d's `main` still had the same code on 2026-09-27 (checked
`src/cdef.rs` at `main`, and the repository's issues: no report of it). Not yet
reported; a report is a public post, which is the operator's call.

**Going back to crates.io.** When a rav1d release borrows only `x_start..x_end`
in `padding` (or fixes the overlap another way): delete `vendor/rav1d/`, delete
the `rav1d` line under `[patch.crates-io]` in `Cargo.toml`, bump the `rav1d`
version pin, and run `media::tests::rav1d_threaded_decode_stress` (ignored;
run it with `--ignored` in a debug build) twenty times: zero aborts before the
vendored copy goes.
