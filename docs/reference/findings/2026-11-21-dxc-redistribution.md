# The shader compiler in the Windows download: may we ship dxcompiler.dll and dxil.dll, and under what terms?

**Researched: 2026-11-20 into 2026-11-21.** Not legal advice: this was written by a developer reading
primary sources, not by a lawyer. Where a step needs a lawyer it says so.

**The question.** The Windows download of HumanityOS carries two Microsoft files beside the program,
`dxcompiler.dll` and `dxil.dll` (the DirectX Shader Compiler, "DXC"): may the project redistribute the
copies it ships today, what notice must travel with them, and if not, where can a copy the project may
redistribute come from?

## The short answer

- **The copies we ship today may not be redistributed as we ship them.** The workflow copies them from
  the Windows SDK's tools folder (`Windows Kits\10\bin\10.0.26100.0\x64`, confirmed in the build log of
  v0.1565.0). Those files come under the Windows SDK licence, which lets a developer redistribute only
  the files on its published "REDIST.TXT list". That list names the SDK's *other* copies of the two
  files, in `Windows Kits\10\Redist\D3D\x64`, and not the tools folder's copies (which are different
  files: same version, different bytes). Outside the list, the licence says you may not "publish the
  software for others to copy".
- **Even the listed copies would not suit an open download.** The SDK licence's conditions for any
  redistributed file include requiring "distributors and external end users to agree to terms that
  protect it at least as much as this agreement" and to "Indemnify, defend, and hold harmless
  Microsoft". A zip anyone may download and pass on has no step where people agree to Microsoft's
  terms, and the indemnity would be a promise by the operator personally.
- **The comments calling DXC "MIT" are wrong.** The DXC project's licence is the University of
  Illinois/NCSA Open Source License (the LLVM licence). The only file DXC's own package puts under its
  MIT licence file is one header, `d3d12shader.h`, which we do not ship (MIT-style terms also appear
  inside LICENSE.TXT, for small third-party pieces compiled into DXC). (Corrected in `build-desktop.yml` and
  `src/renderer/mod.rs` on 2026-11-21, pointing here.)
- **The compliant source is DXC's own open-source release, not the SDK.** DXC's GitHub releases (the
  newest is v1.9.2609, 29 September 2026) are distributed under the NCSA licence, which allows
  redistribution in binary form provided the copyright notice, the conditions and the disclaimer
  travel with it. The terms of `dxil.dll` there have changed: Microsoft's own package still listed a
  separate Microsoft licence file for `dxil.dll` in its July 2024 version and no longer does in its
  February 2025 one (the release whose notes say "DXIL Validator Hash is open sourced."), and the
  repository now builds a `dxil` library from its own NCSA source. **One thing is unread**: the licence files inside the release's zip itself, because reading
  them means downloading the archive (a binary), which needs the operator's yes.
- **A cleaner end state needs no `dxil.dll` at all.** Current wgpu (wgpu-types 30.0.2) asks only for
  `dxcompiler.dll`, from DXC v1.8.2502 on. Our wgpu (24.0.4) still loads both files.
- **The decision is the operator's**: which compliant source to switch to, and what to do with the
  releases that go out before the switch. The options and a recommendation are at the end.

## Findings

Web pages were read through a text-extraction tool, and each site's robots.txt was read first, in a
step of its own. Quotations from web pages are as that tool returned them (a long sentence sometimes
in two pieces, rejoined here); the SDK licence was also read in its installed copy, which matches.

### 1. Which copy we ship, and from where

- **Source:** `.github/workflows/build-desktop.yml`, the "Bundle binary with data" step, read
  2026-11-20 at commit `60ee83b1d`. It picks the newest SDK tools folder,
  `SDKBIN=$(ls -d "/c/Program Files (x86)/Windows Kits/10/bin/"10.*/x64 ... | sort -V | tail -1)`, and
  copies `cp "$SDKBIN/dxcompiler.dll" "$SDKBIN/dxil.dll" dist/HumanityOS/`. Nothing else (no licence,
  no notice) goes with them.
- **The run that built v0.1565.0** (GitHub Actions run 44677787779, 2026-11-20), read with
  `gh run view --log`, prints: "Bundled DXC from /c/Program Files (x86)/Windows Kits/10/bin/10.0.26100.0/x64".
- **The files themselves**, on the operator's machine (the same SDK, 10.0.26100.0, installed files
  dated 2026-01-27), read with PowerShell's file version information and SHA-256:

  | Copy | File version | SHA-256 (first 16) |
  |---|---|---|
  | `bin\10.0.26100.0\x64\dxcompiler.dll` | 1.8.2502.11 | `78B5E21A91707B4A` |
  | `bin\10.0.26100.0\x64\dxil.dll` | 1.8.2502.11 | `77E039C905030A64` |
  | `Redist\D3D\x64\dxcompiler.dll` | 1.8.2502.11 | `B855EE494C60A5DD` |
  | `Redist\D3D\x64\dxil.dll` | 1.8.2502.11 | `2AE215743CA9C9C5` |

  So the tools folder's copies and the redistributable folder's copies are different files. The pair
  in the operator's checkout root (which rigs and the taskbar build use) is the tools folder's pair,
  by hash.
- **How the app uses them** (`src/renderer/mod.rs`, the `backend_options` block, and wgpu-hal 24.0.4's
  `src/dx12/shader_compilation.rs`, `get_dynamic_dxc_container`): the app passes both paths to wgpu,
  which loads `dxcompiler.dll` and also `dxil.dll` (it fails with "Failed to load dxil.dll" when that
  file is missing) and uses `dxil.dll`'s validator. Without the pair, shaders compile with FXC, which is
  part of Windows: boot is much slower and the clouds are left off (`src/renderer/pipeline.rs`).

### 2. The Windows SDK licence: what may be redistributed

- **Source:** "Windows SDK license terms", <https://learn.microsoft.com/en-us/legal/windows-sdk/license>,
  read 2026-11-20. The page's metadata gives `ms.date: 2019-05-01` and `updated_at: 2024-03-19`; the
  text ends "EULAID:WIN10SDK.RTM.AUG\_2018\_en-US". The copy installed with SDK 10.0.26100.0
  (`Windows Kits\10\Licenses\10.0.26100.0\sdk_license.rtf`, dated 2026-01-27) carries the same wording
  in every clause quoted here (checked by reading both).
- **Only listed files are distributable:** "REDIST.TXT Files. You may copy and distribute the object
  code form of code listed in REDIST.TXT files plus the files listed on the REDIST.TXT list."
- **The conditions on any file you do distribute** (section 2.a.ii, "Distribution Requirements. For any
  Distributable Code you distribute, you must"), among them:
  - "Require distributors and external end users to agree to terms that protect it at least as much
    as this agreement;"
  - "Display your valid copyright notice on your programs; and"
  - "Indemnify, defend, and hold harmless Microsoft from any claims, including attorneys’ fees, related
    to the distribution or use of your programs."
- **And a restriction** (2.a.iii): you may not "Distribute Distributable Code to run on a platform other
  than the Microsoft operating system platform;" (our Windows download is Windows-only, so this one is
  met).
- **Outside the list**, the general scope clause (section 7) applies: "You may not ... publish the
  software for others to copy;".
- **Third-party code inside the SDK** (section 1.e): "The software may include third party code that
  Microsoft, not the third party, licenses to you under this agreement." The SDK's notices file
  (`sdk_third_party_notices.rtf`, same folder and date) has an entry headed
  "Microsoft/DirectXShaderCompiler" and says of all such code: "Microsoft, not the third party, licenses
  this Third Party Code to you under the terms set forth in the Microsoft Software License Terms for the
  Microsoft Windows Software Development Kit for Windows 10".
- **Other Microsoft programs inside the SDK** (section 1.d): "The software contains other Microsoft
  programs. The license terms with those programs apply to your use of them." No separate licence for
  DXC is installed with the SDK: its Licenses folder holds only the SDK licence and the notices file.

### 3. The REDIST.TXT list: which copies of the two files are on it

- **Source:** "Windows SDK redist lists", <https://learn.microsoft.com/en-us/legal/windows-sdk/redist>,
  read 2026-11-20. The page says "UPDATED 10/21/2024" (metadata `updated_at: 2024-10-30`). The SDK
  licence's link to the list (`go.microsoft.com/fwlink/?LinkId=524842`) redirects here; it was resolved
  through the `/fwlink/p/` form of the same link, the only part of go.microsoft.com its robots.txt
  allows.
- **What it permits in general:** "If you have a validly licensed copy of such software, you may copy
  and distribute the unmodified object code form of the files listed below, subject to the software's
  License Terms and to the additional terms or conditions that are indicated below."
- **The two files are listed only in the Redist folder**, under "Binaries":
  - "Program Files\Windows Kits\10\Redist\D3D\x64\dxcompiler.dll"
  - "Program Files\Windows Kits\10\Redist\D3D\x64\dxil.dll"

  followed by: "These files may only be redistributed with your Classic Windows applications. They may
  not be redistributed with any Universal Windows apps." No path under `Windows Kits\10\bin\` appears
  anywhere on the list.

### 4. DXC's own licence (its repository)

- **Source:** <https://github.com/microsoft/DirectXShaderCompiler/blob/main/LICENSE.TXT>, read
  2026-11-20 (the page shows no date for the file; the copyright line in it reads "Copyright (c)
  2003-2015 University of Illinois at Urbana-Champaign.").
- **The licence:** headed "LLVM Release License" and "University of Illinois/NCSA Open Source License".
  The word "MIT" does not appear in it (MIT-style terms appear only in the sections for bundled
  third-party pieces such as the YAML parser).
- **The binary condition**, quoted from the SDK's notices file, whose LLVM entry carries the same
  licence text (the repository page was read only as far as the clause's opening, "Redistributions in
  binary form must reproduce the above copyright notice,", by the reading tool's quotation limit):
  "Redistributions in binary form must reproduce the above copyright notice, this list of conditions
  and the following disclaimers in the documentation and/or other materials provided with the
  distribution."
- **The README**, <https://github.com/microsoft/DirectXShaderCompiler/blob/main/README.md>, read
  2026-11-20, under "License": "DirectX Shader Compiler is distributed under the terms of the
  University of Illinois Open Source License." Under "Features and Goals" it lists what the releases
  provide, including "dxil.[dll|so], a DLL providing the DXIL validation and hashing support", and says
  "The Microsoft Windows SDK releases include a supported version of the compiler and validator."
- **The notices file**, <https://github.com/microsoft/DirectXShaderCompiler/blob/main/ThirdPartyNotices.txt>,
  read 2026-11-20, opens "This project incorporates components from the projects listed below." and
  gives notices for LLVM, tools\clang, test\YAMLParser, include\llvm\Support, OpenBSD regex and
  lib\Headers. A redistributor ships it with LICENSE.TXT.

### 5. dxil.dll's terms in DXC's releases, then and now

- **Microsoft's package of the release, on NuGet** (`Microsoft.Direct3D.DXC`, owners Microsoft and
  Direct3D), read 2026-11-20. Its README is headed "DirectX Shader Compiler Redistributable Package" and
  carries a licence table introduced by "The included licenses apply to the following files:" (its rows
  are quoted below with the two cells separated by a bar).
  - Versions 1.7.2308.12 (2023-08-14) and 1.8.2407.12 (2024-07-31):
    "LICENSE-MS.txt | dxil.dll (if included in package)", "LICENSE-MIT.txt | d3d12shader.h",
    "LICENSE-LLVM.txt | all other files".
  - Versions 1.8.2502.7 (2025-02-12) and 1.9.2609.5 (2026-09-29): only "LICENSE-MIT.txt |
    d3d12shader.h" and "LICENSE-LLVM.txt | all other files". The Microsoft licence row is gone.
  - Each version's "License Info" page (for example
    <https://www.nuget.org/packages/Microsoft.Direct3D.DXC/1.9.2609.5/License>) shows the
    "LLVM Release License" / "University of Illinois/NCSA Open Source License" alone. The text of
    LICENSE-MS.txt itself was found on no page.
- **The release notes.** <https://github.com/microsoft/DirectXShaderCompiler/releases/tag/v1.8.2502>
  (21 February 2025): "DXIL Validator Hash is open sourced." and "The validator binaries will no
  longer be required for "signed" (hashed) shaders when using this release of the compiler, however
  they will still be loaded and used if found."
  <https://github.com/microsoft/DirectXShaderCompiler/releases/tag/v1.8.2505> (30 May 2025): "The
  compiler will now always use the internal validator instead of searching for an external DXIL.dll."
  Both list the binaries a release includes: "validator: dxv.exe and dxil.dll on Windows, dxv and
  libdxil.so on Linux." No release on the releases page (read 2026-11-20, newest v1.9.2609 of
  29 September 2026) mentions a licence file.
- **The source of the dxil library.**
  <https://github.com/microsoft/DirectXShaderCompiler/blob/main/tools/clang/tools/dxildll/CMakeLists.txt>,
  read 2026-11-20 (no date shown): it builds `add_clang_library(dxildll SHARED ${sources})` with
  `OUTPUT_NAME "dxil"`, depends on `LLVMDxilHash`, and is headed "# This file is distributed under the
  University of Illinois Open Source License. See LICENSE.TXT for details."
- **How Microsoft's own package manager installs the release.** vcpkg's port,
  <https://github.com/microsoft/vcpkg/blob/master/ports/directx-dxc/portfile.cmake>, read 2026-11-20,
  pins `set(DIRECTX_DXC_TAG v1.9.2609)`, downloads the release's `dxc_${DIRECTX_DXC_VERSION}.zip`,
  installs `"${PACKAGE_PATH}/bin/${DXC_ARCH}/dxil.dll"` among the files, and installs as the whole
  package's copyright only the repository's LICENSE.TXT for that tag
  (`vcpkg_install_copyright(FILE_LIST "${LICENSE_TXT}")`).

### 6. wgpu: whether the app needs dxil.dll at all

- **Our version** (`Cargo.lock`: wgpu-hal 24.0.4), source read in the local cargo registry:
  `get_dynamic_dxc_container(dxc_path, dxil_path)` loads both, mapping a missing second file to
  `FailedToLoad("dxil.dll", e)`, and creates `IDxcValidator` from it.
- **Current wgpu**, <https://docs.rs/wgpu-types/latest/wgpu_types/enum.Dx12Compiler.html>, read
  2026-11-20 (wgpu-types 30.0.2): `DynamicDxc { dxc_path: String }`, documented as "However, it requires
  `dxcompiler.dll` to be shipped with the application." and "Minimum supported version: v1.8.2502". It
  names no `dxil.dll`.

## What is still unknown, and what would settle it

- **The licence files inside DXC's release zip** (`dxc_2026_09_29.zip` from the v1.9.2609 release).
  Everything readable points to NCSA for every file in it (the NuGet table since 1.8.2502, the open
  `dxildll` source, vcpkg installing only LICENSE.TXT), but whether the zip still carries a
  LICENSE-MS.txt for its `dxil.dll` was not read. Settled by downloading that one archive (from
  <https://github.com/microsoft/DirectXShaderCompiler/releases/tag/v1.9.2609>, its size shown there)
  and reading its licence files: free, and needs the operator's yes for the download.
- **Whether the release's `dxil.dll` is the open build** of `tools/clang/tools/dxildll` or a separately
  built, signed Microsoft binary. The same download settles most of it (its licence files and its
  file properties); a question on the DXC issue tracker would settle it for certain. Free. It matters
  only if we keep shipping `dxil.dll` (option B below removes it).
- **Whether a byte-different copy of a listed file is "the file listed"** (our tools-folder copy is the
  same version as the listed Redist copy). A lawyer's question, and moot once we take the files from a
  source that is plainly redistributable.
- **Whether the SDK's distribution conditions can be met by an open-source download at all** (an
  end-user agreement for a zip, the indemnity). A lawyer's question, and also moot under options A or B.
- **When the wgpu change that dropped `dxil_path` landed** (which major version). Settled by reading
  wgpu's changelog by section (the reading tool truncated the whole file) or by the wgpu upgrade itself.

## Sources you could not reach

- The licence files inside `dxc_2026_09_29.zip`: not read, because the brief for this research forbade
  downloading any binary; that is the open question above.
- <https://github.com/gfx-rs/wgpu/blob/trunk/CHANGELOG.md>: the reading tool returned only the page's
  header, the body "truncated due to length". Wanted: the version that removed `dxil_path`.
- File histories (`/commits/` pages) on github.com and `raw.githubusercontent.com` were not read: the
  former is disallowed by github.com's robots.txt; the latter was not needed (the `/blob/` pages, which
  robots.txt allows, show the same files).
- `go.microsoft.com/fwlink/?LinkId=524842` in that exact form: its robots.txt disallows everything but
  `/fwlink/p/`; the `/p/` form led to the same list (finding 3).

## What it means for this project

### Certain

- Every Windows release since v0.865.0 (2026-07-16, the one commit that added the copy to the
  workflow; it has not changed since) shipped the SDK tools folder's copies of `dxcompiler.dll` and
  `dxil.dll`, which are not on the SDK's redistribution list, with no notice of any kind beside them.
- The SDK licence allows redistributing only listed files, and attaches to them conditions (end users
  agreeing to protective terms, indemnifying Microsoft) that our open download does not meet today.
- DXC's open-source licence is NCSA, not MIT. NCSA allows redistributing binaries if its copyright
  notice, conditions and disclaimer go with them; for us that means a licence file beside the DLLs.
- The app runs without the pair (FXC), slower to boot and with the clouds off.
- wgpu 24.0.4, which we use, needs both files for DXC; wgpu-types 30.0.2 needs only `dxcompiler.dll`.

### Judgement calls

- **The SDK copy is governed by the SDK licence, not by NCSA.** The DXC source is NCSA, but the SDK's
  notices say Microsoft licenses the code in the SDK "under the terms set forth in the Microsoft
  Software License Terms", and nothing else accompanies the files in the SDK. A lawyer might argue
  the NCSA grant still reaches a binary built from NCSA source; we should not ship on that argument
  when an unambiguous source exists.
- **Copying the SDK pair for the operator's own development** (the checkout root, the rigs, the taskbar
  build) is "design, develop and test" use the SDK licence grants, and is not redistribution. It does
  not need to change.
- **The options, for the operator to choose** (each keeps the program itself AGPL-3.0, untouched):
  - **A. Take the pair from DXC's own GitHub release**, pinned by tag and SHA-256 in the workflow, and
    ship DXC's LICENSE.TXT and ThirdPartyNotices.txt beside the DLLs. Needs: one download to read the
    zip's licence files first (above), then the workflow downloading that pinned archive on every build.
    Small change; keeps DXC and the clouds for every downloader.
  - **B. Upgrade wgpu to a version whose DXC option needs only `dxcompiler.dll`** (wgpu-types 30 does),
    then ship only `dxcompiler.dll` from DXC's release with its LICENSE.TXT and notices. No `dxil.dll`
    at all, so no question about its terms. Large change (wgpu 24 to 30 touches the whole renderer);
    best done when the wgpu upgrade is due anyway.
  - **C. Build `dxcompiler.dll` (and, while wgpu 24 needs it, `dxil.dll`) from DXC's source in CI.** No
    Microsoft binary at all, entirely NCSA. A long CI build of an LLVM-sized project on every release.
  - **D. Keep the SDK's listed Redist copies** and meet the SDK conditions (an end-user agreement and
    the operator's indemnity of Microsoft). Not recommended: it puts a Microsoft licence and a personal
    indemnity inside an open-source download, and it is a poor fit for the Digital Public Goods
    standard's open-licence indicator.
  - **E. Stop bundling the pair** until A or B lands: one line in the workflow; downloaders boot on FXC
    (much slower, clouds off) meanwhile.
- **Recommendation: A now, B later.** A is a small, reversible change that makes the download compliant
  and keeps the fast compiler; B removes `dxil.dll` for good when the wgpu upgrade happens. Until A lands,
  every new release redistributes the SDK copies again; if the operator wants no further release to
  do that, E is the stopgap.
- **The house check.** Since 2026-11-21, `scripts/tests/shipped-library-licences.test.js` fails when a
  DLL (or `.so` / `.dylib`) goes into the desktop download without a licence file named for it in the
  same folder (`<name>.LICENSE.txt`). The DXC pair is the one exception it pins, pointing at this
  finding, and it fails as soon as that exception is no longer needed, so the exception cannot outlive
  the fix.

**Not legal advice.** Written by a developer reading primary sources on 2026-11-20 and 2026-11-21.
Anyone relying on it for a decision with legal weight should have a lawyer read the SDK licence and the
DXC release's licence files.
