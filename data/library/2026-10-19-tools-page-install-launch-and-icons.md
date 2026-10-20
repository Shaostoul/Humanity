# Tools page: finding installed programs, installing through winget, starting them, and their icons

**Researched 19 October 2026** (sources read between about 18:25 and 19:00
Pacific time, which is 01:25 to 02:00 UTC on 20 October). Everything quoted
below was read that day, or, where a live page refused the request, read from an
Internet Archive copy whose date is given. Package manager documentation,
project policies and download pages change often. If you are reading this later,
check each source's own date before relying on it.

**This is a reading of public sources by an AI working on the project. It is
not legal advice, and nobody who wrote it is a lawyer.** Where it says what the
project should do, that is the project's own reading, marked as such.

This finding sits beside
[`2026-10-19-games-page-art-stats-and-links.md`](/library#2026-10-19-games-page-art-stats-and-links),
which in turn extends
[`2026-10-13-game-stores-and-a-games-library.md`](/library#2026-10-13-game-stores-and-a-games-library).
It does not repeat or replace either. Already settled there and not re-argued
here: that showing a picture to the person at their own computer is not a
*public* display under US copyright law (17 U.S.C. §§ 101 and 106, games
finding 1.3); the Ninth Circuit's nominative fair use test from *New Kids on the
Block* (1992), which lets a plain word name another's product and contrasts it
with using a company's "distinctive lettering" (games finding 2.4); Valve's
documented `steam://run` and `steam://rungameid` links and the advice that every
store action follows a person's click (2026-10-13 finding). It also leans on
[`2026-10-10-california-ab-1043-age-signals.md`](https://github.com/Shaostoul/Humanity/blob/main/docs/reference/findings/2026-10-10-california-ab-1043-age-signals.md)
for California's age-signal law, and adds one passage to it (section 2.10).

## The question

What may the desktop Tools page (`src/gui/pages/tools.rs`, a list of free
programs from `data/external/catalog.json`, kind `"software"`, which today only
links to each project's website) do to become a library like the Games page:
show which programs are installed, install one through winget when a person
clicks Install, start an installed one, and show each program's icon? Six parts:

1. **Finding what is installed** on Windows: what Microsoft documents about the
   Uninstall registry keys and the App Paths key, and the gaps.
2. **Installing through winget** on a person's click: the client's and the
   community repository's terms, whether calling winget from another program is
   intended, what the `--accept-package-agreements` and
   `--accept-source-agreements` switches mean and whether an app may accept for
   a person, how installers are checked and reviewed, administrator rights,
   failures, and the catalog's programs that are not plain free software
   (DaVinci Resolve above all).
3. **The alternatives**, briefly: downloading the installer ourselves; Homebrew
   on macOS; Flatpak and Flathub on Linux.
4. **Starting** an installed program on a click, including a Steam copy such as
   Blender's.
5. **Icons and logos**: (a) read from the installed program itself, (b) each
   project's logo from its website, against each project's own policy.
6. **Safety advice for people new to technology**: what Microsoft and the
   projects say about where to get software.

## Short answer

1. **Finding what is installed.** Microsoft documents the Uninstall key as what
   installers write and what Add or Remove Programs shows, and its own
   PowerShell documentation reads that key from a script to list installed
   software, while warning: "There is no guaranteed way to find every
   application on a system." Per-user installs register under the current user
   only; 64-bit Windows keeps a separate 32-bit view of `HKLM\Software`; the
   `DisplayIcon` value is not in Microsoft's reference documentation at all;
   Store and MSIX apps are listed through a different interface
   (`PackageManager`). **Nothing read forbids another program reading these
   keys.**
2. **winget.** The client and the community repository are both MIT licensed,
   and Microsoft writes that "Third party package managers may leverage the
   WinGet Community Repository". The client prints, on every install that is
   not from the Store, "This application is licensed to you by its owner." and "Microsoft is not
   responsible for, nor does it grant any licenses to, third-party packages."
   Microsoft documents `--accept-package-agreements` as a way to suppress the
   prompt that asks *the user* to agree, useful "when scripting". **Our
   working assumption is confirmed, with one addition:** the app must show the
   licence and any agreements and get the person's own "I agree" before it
   passes that switch, and it must also show the licence for the many packages
   that carry no agreement in winget's data, because winget runs installers
   without their own windows by default, so the installer's own licence page
   never appears. And winget's COM interface accepts both kinds of agreement
   by default in its code, so a program using it must turn that off. The Ninth
   Circuit: a click is assent "only if the user is
   explicitly advised that the act of clicking will constitute assent".
   Installers are checked against a SHA-256 hash; manifests are validated
   automatically and "may also be reviewed manually". **DaVinci Resolve is not
   in the community repository**; Blackmagic offers its free version only from
   its own site, behind a registration form. See section 2.9 for the other non-free entries, and 2.10 for a
   California question the Install button raises.
3. **Alternatives.** Downloading ourselves means we check the file ourselves:
   several projects publish SHA-256 sums and signatures, and Windows can check
   a publisher's signature. Homebrew (BSD licence) and Flathub are the natural
   later routes for macOS and Linux. Windows comes first.
4. **Starting a program.** Microsoft documents `ShellExecuteEx` and
   `CreateProcess` for exactly this; the GPL says "The act of running the
   Program is not restricted". **Nothing read is against it.** Blender's Steam
   copy is App ID 365670 and starts through Steam, as the Games page already
   does.
5. **Icons and logos.** (a) The icon Windows shows for an installed program can
   be read from the program's own file and shown to that person only, the same
   reading as the Games page. (b) Most projects allow their logo to refer to
   their program without asking (Blender, Krita, Inkscape, LibreOffice; GIMP's
   logo is CC BY-SA 4.0; VS Code allows its icon "to link to our website"), each
   with conditions (unaltered, credit, no implied endorsement); OBS's published
   rule is "Do not use the OBS logo or IP without authorization"; Blackmagic
   publishes no logo rules. **Recommended: show the installed program's own
   icon, and plain names for the rest; ship no logo files.**
6. **Safety.** Microsoft: "Download apps only from trusted sources, such as
   Microsoft Store". The FTC: "Don't click on ads to download software." The
   projects: their own site, plus the stores and package managers each one
   names (KeePassXC and Audacity name winget). HandBrake: its site, its GitHub
   and Flathub "are the only official places". An OBS developer: "never click
   search engine ads for any software."

## How each finding is recorded

Each finding gives the source's address, the source's own date where it shows
one, and a quotation of the words that carry the meaning. For Microsoft Learn
pages the date given is the page's own metadata (`ms.date`, and `updated_at`
for its latest revision). For files on GitHub it is the date of the latest
commit that touched the file. Long passages are trimmed to the words that
matter; "(fragment)" marks a quotation that is part of a longer sentence, and
nothing inside a quotation is reworded. Every quotation was checked by script
against the text saved on the day. The text extraction put a space wherever a
link or formatting tag ended; those stray spaces are removed here, and nothing
else is changed.

## 1. Finding what is installed (Windows)

### 1.1 The Uninstall key, as Microsoft documents it

- "Windows Installer Properties for the Uninstall Registry Key",
  https://learn.microsoft.com/en-us/windows/win32/msi/uninstall-registry-key
  (ms.date 2018-05-31, updated 2025-04-15):
  - "The following installer properties give the values written under the
    registry key:" followed by
    `HKEY_LOCAL_MACHINE\Software\Microsoft\Windows\CurrentVersion\Uninstall`.
  - "The values are stored in a subkey identified by the application's product
    code GUID."
  - The table pairs each value with the installer property it comes from:
    `DisplayName` from "ProductName property", `DisplayVersion` "Derived from
    ProductVersion property", `Publisher` from "Manufacturer property",
    `InstallLocation` from "ARPINSTALLLOCATION property".
- "Configuring Add/Remove Programs with Windows Installer",
  https://learn.microsoft.com/en-us/windows/win32/msi/configuring-add-remove-programs-with-windows-installer
  (ms.date 2018-05-31, updated 2025-03-11):
  - "If an application is not registered, it is not listed in Add/Remove
    Programs in Control Panel."
  - `ARPSYSTEMCOMPONENT`: "Prevents display of the application in the Programs
    List of the Add/Remove Programs in the Control Panel." (fragment)
  - `ARPPRODUCTICON`: "Identifies the icon displayed in Add/Remove Programs. If
    this property is not defined, Add/Remove Programs specifies the display
    icon."

These pages are written for people building installers. They say what an
installer writes and what Add or Remove Programs shows. **They say nothing about
other programs reading the key, for or against.**

### 1.2 Microsoft reading the key from another program

"Working with software installations", PowerShell documentation,
https://learn.microsoft.com/en-us/powershell/scripting/samples/working-with-software-installations
(ms.date and updated 2023-03-17), has a section titled "Querying the Uninstall
registry key to find installed software":

- "Because most standard applications register an uninstaller with Windows, we
  can work with those locally by finding them in the Windows registry."
- "There is no guaranteed way to find every application on a system."
- "it's possible to find all programs with listings displayed in Add or Remove
  Programs in the following registry key:" (fragment), naming
  `HKEY_LOCAL_MACHINE\Software\Microsoft\Windows\CurrentVersion\Uninstall`.
- "applications installed by copying the files to a folder on the computer
  usually can't be managed using techniques discussed here." (fragment)
- "There is no guarantee that the DisplayName values are unique."
- On the alternative, WMI's `Win32_Product`: "The Win32_Product class isn't
  query optimized." The same caution says such a query "initiates a consistency
  check of packages installed, verifying and repairing the install" (fragment).

Windows Installer also has a documented interface for its own (MSI) products:
"MsiEnumProductsEx function enumerates through one or all the instances of
products that are currently advertised or installed in the specified contexts."
(https://learn.microsoft.com/en-us/windows/win32/api/msi/nf-msi-msienumproductsexw,
ms.date 2018-12-05, updated 2025-11-25), and "A user must have administrator
privileges to enumerate products across all user accounts or a user account
other than the current user account." It covers MSI packages only; many of the
catalog's programs use other installers (section 2.6).

winget's own `list` command reads the same ground:
"The list command will show apps that were installed through the Windows
Package Manager as well as apps that were installed by other means."
(https://learn.microsoft.com/en-us/windows/package-manager/winget/list, ms.date
2026-07-19, updated 2026-07-21).

### 1.3 Per-user installs

- "Installation Context",
  https://learn.microsoft.com/en-us/windows/win32/msi/installation-context
  (ms.date 2018-05-31, updated 2025-04-15): "Applications appear only under
  Add/Remove Programs on Control Panel for users that have installed the
  applications."
- "Configuring Add/Remove Programs" (1.1): "Applications that have not been
  installed per-machine, and have only been installed as per-user applications
  for users other than the current user, do not appear in the Add/Remove
  Programs of the current user."
- "Single Package Authoring",
  https://learn.microsoft.com/en-us/windows/win32/msi/single-package-authoring
  (ms.date 2018-05-31, updated 2025-03-11): "In a per-user installation,
  registry entries that have -1 in the Root column are redirected to
  HKEY_CURRENT_USER, and in a per-machine installation these are directed to
  HKEY_LOCAL_MACHINE."
- "ALLUSERS property", https://learn.microsoft.com/en-us/windows/win32/msi/allusers
  (ms.date 2020-07-27, updated 2025-11-18): "The recommended default
  installation context is per-user."

**Negative finding:** no Microsoft documentation page found names the path
`HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Uninstall`. Per-user
registration is documented only in the general terms above. (VS Code's own
winget manifest offers a "user" installer, section 2.6, so the catalog has
programs that install this way.)

### 1.4 The 32-bit view on 64-bit Windows

- "Registry Redirector",
  https://learn.microsoft.com/en-us/windows/win32/winprog64/registry-redirector
  (ms.date 2018-05-31, updated 2025-04-15): "For example,
  HKEY_LOCAL_MACHINE\Software is redirected to
  HKEY_LOCAL_MACHINE\Software\Wow6432Node." and "Applications should not access
  a key's physical location directly, because this location may change."
- "Accessing an Alternate Registry View",
  https://learn.microsoft.com/en-us/windows/win32/winprog64/accessing-an-alternate-registry-view
  (ms.date 2018-05-31, updated 2025-03-13): "By default, a 32-bit application
  running on WOW64 accesses the 32-bit registry view and a 64-bit application
  accesses the 64-bit registry view." The flags `KEY_WOW64_64KEY` and
  `KEY_WOW64_32KEY` reach the other view, and "To accurately enumerate all keys
  in both views, perform the enumeration in two passes."
- "Registry Keys Affected by WOW64",
  https://learn.microsoft.com/en-us/windows/win32/winprog64/shared-registry-keys
  (ms.date 2021-01-23, updated 2025-10-23): "Subkeys of the keys in this table
  inherit the parent key's behavior unless otherwise specified." The table lists
  `HKEY_LOCAL_MACHINE\SOFTWARE` as redirected, `HKEY_CURRENT_USER\SOFTWARE` as
  shared, and, for Windows 7 and later, the `App Paths` key as shared.

**Inference, not stated by Microsoft:** the Uninstall key is not in that table,
so under `HKLM` it inherits the redirection (32-bit programs register under
`Wow6432Node`) and under `HKCU` it is shared. A reader must look in both views
of `HKLM`.

### 1.5 The App Paths key

"Application Registration",
https://learn.microsoft.com/en-us/windows/win32/shell/app-registration
(ms.date 2018-05-31, updated 2025-07-03):

- "When the ShellExecuteEx function is called with the name of an executable
  file in its lpFile parameter, there are several places where the function
  looks for the file. We recommend registering your application in the App
  Paths registry subkey."
- Its purpose: "To map an application's executable file name to that file's
  fully qualified path."
- "An application that is installed for per user can be registered under
  HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\App Paths." and
  the same under `HKEY_LOCAL_MACHINE` for all users.

So App Paths, where a program registers there, gives the full path of its
program file, which the Uninstall key often does not (`InstallLocation` is a
folder, and is optional).

### 1.6 `DisplayIcon` and `SystemComponent`

- **Negative finding:** no Microsoft reference page names the `DisplayIcon`
  value. It is missing from the Uninstall key table and the Add/Remove Programs
  page (1.1). It appears in blog posts by Microsoft engineers: Raymond Chen,
  "Why does Add or Remove Programs show a large blank space?"
  (https://devblogs.microsoft.com/oldnewthing/20050526-07/?p=35533, 26 May
  2005): "The format of the icon registration is a filename, optionally followed
  by a comma and a decimal number."; Aaron Stebner, archived blog
  (https://learn.microsoft.com/en-us/archive/blogs/astebner/what-controls-the-information-that-appears-in-addremove-programs-for-installed-programs,
  7 February 2006): "One of the items that is notably absent from the list of
  entries that Windows Installer populates is the DisplayIcon."
- **Negative finding:** no Microsoft documentation page names the
  `SystemComponent` registry value; only the installer property
  `ARPSYSTEMCOMPONENT` (1.1) is documented. Heath Stewart's 2005 blog post
  (https://devblogs.microsoft.com/setup/the-dangers-of-arpsystemcomponent/,
  5 August 2005) explains it: "When ARPSYSTEMCOMPONENT is defined for a
  package, all the data is read from the registry key under" (fragment).

### 1.7 Microsoft Store and other packaged (MSIX) apps

- "PackageManager Class",
  https://learn.microsoft.com/en-us/uwp/api/windows.management.deployment.packagemanager
  (ms.date 2017-02-08, updated 2025-11-25): "Manages the software available to a
  user." Its method `FindPackagesForUser`: "Finds all Packages installed for the
  specified user." The method's own page
  (https://learn.microsoft.com/en-us/uwp/api/windows.management.deployment.packagemanager.findpackagesforuser,
  same dates): "If this parameter is string.Empty, the packages are retrieved for
  the current user." and administrative rights are needed only "if the user
  security ID specified is different from the user security ID of the calling
  user" (fragment).
- "Understanding how packaged desktop apps run on Windows" (MSIX),
  https://learn.microsoft.com/en-us/windows/msix/desktop/desktop-to-uwp-behind-the-scenes
  (ms.date 2026-09-30, updated 2026-10-12), for virtualized apps: "App packages
  contain a registry.dat file, which serves as the logical (virtual) equivalent
  of HKLM\Software in the real registry."
- **Negative finding:** no Microsoft documentation page says, in so many words,
  that packaged apps are absent from the Uninstall key. Microsoft documents a
  separate interface for listing them, and says their registry writes are
  virtualized. Several catalog programs are also offered in the Microsoft Store
  (section 6.3), so a Store copy needs this second route.

## 2. Installing through winget

### 2.1 What winget is, and whether other programs are meant to use it

- "Windows Package Manager" overview,
  https://learn.microsoft.com/en-us/windows/package-manager/ (ms.date
  2025-02-13, updated 2025-12-12): "Third party package managers may leverage
  the WinGet Community Repository to increase the size of their software
  catalog." The same page names its audiences as developers, software vendors
  and enterprises; people new to technology are not among them.
- "Use WinGet to install and manage applications",
  https://learn.microsoft.com/en-us/windows/package-manager/winget/ (ms.date
  2026-07-19, updated 2026-07-21):
  - "WinGet the Windows Package Manager is available on Windows 11, modern
    versions of Windows 10, and Windows Server 2025 as a part of the App
    Installer." and "The App Installer is a System Component delivered and
    updated by the Microsoft store on Windows Desktop versions" (fragment).
  - "The WinGet command line tool is only supported on Windows 10 version 1809
    (build 17763) or later."
  - "It's possible that App Installer is not installed on your device." and
    "WinGet will not be available until you have logged into Windows as a user
    for the first time" (fragment).
- The COM interface, in the client's design document "#888 - Com Api",
  https://github.com/microsoft/winget-cli/blob/master/doc/specs/%23888%20-%20Com%20Api.md
  (latest commit 10 June 2026, a repository-wide line-ending change, so the
  wording may be older):
  - "This API is designed to provide another way for callers to make use of that
    functionality."
  - "The API will be preferred by callers that want to receive progress and
    completion events, and UWP packages that do not have permission to launch
    command line processes."
  - "This API allows packaged apps with the packageManagement capability and
    other higher privilege processes to start, manage, and monitor installation
    of packages that are listed in Windows Package Manager repositories."
  - **Negative finding:** Microsoft Learn has no reference page for this
    interface (`Microsoft.Management.Deployment`); it is described only in the
    design document and the interface file in the repository. The PowerShell
    module `Microsoft.WinGet.Client` ships it and "provides native cmdlets for
    interacting with the Windows Package Manager" (winget-cli README, latest
    commit 13 August 2026).
- winget also carries an assistant interface: "The Windows Package Manager
  (WinGet) includes a built-in Model Context Protocol (MCP) server."
  (https://learn.microsoft.com/en-us/windows/package-manager/winget/mcp-server-overview,
  ms.date 2026-07-19, updated 2026-07-20).

So Microsoft documents three ways for another program to drive winget (the
command line, the COM interface, the PowerShell module) and names other package
managers as welcome users of its repository. **No page read limits who may call
it, or for what.**

### 2.2 Licences, and who is responsible for a package

- The client: `LICENSE` in https://github.com/microsoft/winget-cli (latest
  commit 10 June 2026, the line-ending change) begins "MIT License" and
  "Copyright (c) Microsoft Corporation. All rights reserved."
- The community repository of manifests: `LICENSE` in
  https://github.com/microsoft/winget-pkgs (latest commit 11 September 2022) is
  the same MIT licence. Its README (latest commit 22 June 2026): "This repository
  contains the manifest files for the **Windows Package Manager** default
  source." (bold in the original markup), and, on what may be submitted, "To
  avoid doubt, you may not make any Submissions linking to third-party materials
  if such Submission is prohibited by the applicable third party and/or
  otherwise violates such third party's rights."
- What the client prints on an install, from its English text file
  `src/AppInstallerCLIPackage/Shared/Strings/en-us/winget.resw` (latest commit
  2 October 2026), shown before every non-Store install
  (`ShowInstallationDisclaimer` in `InstallFlow.cpp`):
  - `InstallationDisclaimer1`: "This application is licensed to you by its
    owner."
  - `InstallationDisclaimer2`: "Microsoft is not responsible for, nor does it
    grant any licenses to, third-party packages."
- The repository's own policy page, `doc/Policies.md` in winget-pkgs (latest
  commit 12 August 2026): "These agreements are between the publisher and the
  user (or organization) installing the software."

So the licence for winget itself is MIT, which permits any use, and the licence
for each program comes from that program's publisher to the person installing
it, not from Microsoft and not from whoever pressed the keys.

### 2.3 The two "accept" switches

Microsoft Learn, "install command",
https://learn.microsoft.com/en-us/windows/package-manager/winget/install
(ms.date 2026-07-19, updated 2026-07-21):

- `--accept-package-agreements`: "Accepts any license agreements or EULAs
  presented by the package installer, suppressing the interactive prompt." and
  "This applies to the package's own license terms only — it does not affect
  optional components or bundled software offered by the installer." (the
  page's own dash)
- `--accept-source-agreements`: "Accepts the license agreement for the WinGet
  source (repository), suppressing the interactive prompt." and "This is
  separate from any package license — it covers the terms of use for the source
  itself, such as the winget community repository."
- Under "License Agreements": "Some applications when installed will require
  the user to agree to the license or other agreements before installing. When
  this occurs, the Windows Package Manager will prompt the user to agree to the
  agreements. If the user does not agree, the application will not install."
  and "From the command line, you can auto accept the agreements by passing the
  following option --accept-package-agreements on the command line. This can be
  beneficial when scripting the Windows Package Manager."

The client's own prompt (winget.resw, 2.2): `PackageAgreementsPrompt`, "The
publisher requires that you view the above information and accept the
agreements before installing. Do you agree to the terms?"; the switch's
one-line help, `AcceptPackageAgreementsArgumentDescription`, "Accept all license
agreements for packages".

Source agreements, "source command",
https://learn.microsoft.com/en-us/windows/package-manager/winget/source (ms.date
2026-07-19, updated 2026-07-21):

- "An individual source may request that the user agree to agreements presented
  before adding or using the source. If a user does not accept the agreements,
  WinGet will not be able to access the source."
- "Source agreements are required to be accepted before use."
- "A WinGet source provides the data for you to discover and install
  applications. Only use secure, trusted sources."
- The default sources are `msstore` ("The Microsoft Store catalog."), `winget`
  (the community repository) and `winget-font`.
- The `msstore` source's agreement, as its own server reports it
  (https://storeedgefd.dsx.mp.microsoft.com/v9.0/information, a live response
  with no date): `"AgreementLabel":"Terms of Transaction","AgreementUrl":"https://aka.ms/microsoft-store-terms-of-transaction"`.
  The client also tells the person that this source needs "the current
  machine's 2-letter geographic region" (`SourceAgreementsMarketMessage`,
  fragment).
- **Negative finding:** no agreement for the `winget` community source itself
  was found; the only source agreement found is the Store's. The install page's
  wording ("such as the winget community repository") suggests one could exist.

The agreements a package may carry, manifest schema 1.28.0, `defaultLocale.md`
in winget-pkgs (latest commit 4 August 2026):

- `Agreements`: "This key holds any agreements a user must accept prior to
  download and subsequent install or upgrade." and "Agreements are only allowed
  in the community repository when the manifest is maintained by a verified
  developer."
- `License` (a required field): "This key represents the license governing the
  use and or distribution for the product." (fragment) and "If there is no
  available information on a product's license, "Proprietary" should be the
  value in this field." `LicenseUrl` is optional.
- Policies.md (2.2): "When installer switches represent an explicit agreement,
  the manifest must include:" an agreement title and URL; "The agreement body is
  not allowed. Only verified developers may provide agreement text." and
  "**Manifests with agreements require a review by Microsoft.**"

What the manifests of the catalog's programs actually carry (read through
GitHub on the day): Blender 5.2.2 `License: GPL-3.0-or-later`, GIMP 3.2.6.0
`License: GPL-3.0`, LibreOffice 26.8.1.1 `License: MPL-2.0`, OBS Studio 32.2.2
`License: GPL-2.0`, HandBrake 1.11.2 `License: GPL-2.0`, and Visual Studio Code
`License: Microsoft Software License` with
`LicenseUrl: https://code.visualstudio.com/license`. **None of them has an
`Agreements` field.**

**And why that matters: winget hides the installer's own licence page by
default.** Schema 1.28.0, `installer.md` (latest commit 4 August 2026),
`SilentWithProgress`: "This is intended to allow a progress indication to the
user, and the indication may come from an installer UI dialogue, but it must
not require user interaction to complete. The Windows Package Manager currently
defaults to this install experience." The install page (above): `--interactive`
"Runs the installer in interactive mode." So in winget's normal mode a licence
page inside the installer is skipped, and for a
package with no `Agreements` field nothing about the licence is shown at all
beyond the two disclaimer lines.

**What the COM interface does by default.** In the client's source code
(not its prose): `InstallOptions.h`,
https://github.com/microsoft/winget-cli/blob/master/src/Microsoft.Management.Deployment/InstallOptions.h
(latest commit 1 October 2026), `bool m_acceptPackageAgreements = true;`; and
`PackageCatalogReference.h` (latest commit 20 May 2026),
`bool m_acceptSourceAgreements = true;`. `PromptFlow.cpp` (latest commit 2
October 2026) skips the prompt when "We are running in a non-interactive context
(e.g., COM call)" (fragment of a code comment). **A program that uses the COM
interface and does nothing accepts both kinds of agreement for the person
silently.**

**Negative finding:** no Microsoft text says who may accept a package's or a
source's agreements, or that a program may accept them for a person. The
nearest words are "beneficial when scripting" and "between the publisher and
the user (or organization) installing the software."

### 2.4 What "accepting" means in law (United States, the Ninth Circuit)

*Berman v. Freedom Financial Network, LLC*, No. 20-16900 (9th Cir.), filed 5
April 2022, 30 F.4th 849, read from the court's own copy,
https://cdn.ca9.uscourts.gov/datastore/opinions/2022/04/05/20-16900.pdf . The
case is about websites holding consumers to terms, the reverse of our case, but
it states what makes a click mean "I agree":

- "an enforceable contract will be found based on an inquiry notice theory only
  if: (1) the website provides reasonably conspicuous notice of the terms to
  which the consumer will be bound; and (2) the consumer takes some action,
  such as clicking a button or checking a box, that unambiguously manifests his
  or her assent to those terms." (fragment)
- "A user's click of a button can be construed as an unambiguous manifestation
  of assent only if the user is explicitly advised that the act of clicking will
  constitute assent to the terms and conditions of an agreement."
- On the plain form that works ("clickwrap"): "a website presents users with
  specified contractual terms on a pop-up screen and users must check a box
  explicitly stating "I agree" in order to proceed." (fragment)

Read with 2.2 and 2.3 (our reading, a judgement): the licence is between the
publisher and the person, and winget's switch exists to skip asking. If
HumanityOS passes the switch without having shown the terms and been told
"I agree" by the person, then by this test the person has most likely not
agreed to anything, while HumanityOS has told winget that they did. Who is bound in that case was not researched (see "What is still
unknown").

### 2.5 How winget checks installers, and who reviews manifests

- Schema 1.28.0, `installer.md`, `InstallerSha256` (a required field): "It is
  used to confirm the installer has not been modified. The Windows Package
  Manager will compare the hash in the manifest with the calculated hash of the
  installer after it has been downloaded." (fragment)
- The client's messages (winget.resw): "Installer hash does not match; to
  override this check use --ignore-security-hash" and "Installer hash does not
  match; this cannot be overridden when running as admin". The install page:
  `--ignore-security-hash`, "Ignore the installer hash check failure. Not
  recommended." The override must first be enabled by an administrator setting
  (`InstallerHashOverride`; "Administrative settings are disabled by default.",
  `Enable-WinGetSetting.md` in winget-cli, latest commit 9 September 2024).
- Other checks the client reports (winget.resw): "An anti-virus product reports
  an infection in the installer" and "The installer failed security check".
- Review of submissions, "Windows Package Manager repository",
  https://learn.microsoft.com/en-us/windows/package-manager/package/repository
  (ms.date 2024-11-21, updated 2026-07-14):
  - "an automated process will validate your manifest file and check to make
    sure the package complies with the Windows Package Manager polices and is not
    known to be malicious." (fragment, the page's spelling)
  - "Once complete, your submission will be manually reviewed by a moderator,
    and after it is approved, your application will be added to the Windows
    Package Manager catalog." But the same page's label table says: "If no
    issues are encountered during the test pass it will automatically be
    approved. If a test fails, it may be flagged for manual review."
  - Among the expectations: "The installer and application are virus free." and
    "The installer comes directly from the publisher's website."
- The overview (2.1): "Manifests and the packages they reference are
  automatically validated and may also be reviewed manually."
- winget-pkgs `doc/Validation.md` (latest commit 12 August 2026): "**Static
  antivirus scanning** — the installer binary is scanned with multiple
  antivirus engines for known malware signatures and heuristic detection." (the
  file's own dash) and "If validation passed and the PR is approved, it is
  **automatically merged**." `doc/FAQ.md` (latest commit 27 July 2026): "There
  are additional manual checks in place, as each submission requires moderator
  approval before it can be merged." `doc/Moderation.md` (latest commit 3
  October 2026): "In addition to Microsoft employees, several community members
  have been identified through their high-quality submissions, willingness to
  help others, and adherence to our" (fragment).
- Policies, https://learn.microsoft.com/en-us/windows/package-manager/package/windows-package-manager-policies
  (ms.date 2026-08-30; the text says "Document date: May 22, 2021"): "The
  InstallerUrl must be the ISV's release location for the Product. Products
  from download websites will not be allowed."
- Who may submit, https://learn.microsoft.com/en-us/windows/package-manager/package/
  (ms.date 2024-11-21, updated 2024-11-22): "If you are a GitHub community
  member, you may also submit packages to Windows Package Manager following the
  steps above." So a manifest can be written by someone other than the
  program's maker.

Where the catalog's installers come from (the manifests read on the day):
Blender from `download.blender.org`, LibreOffice from
`download.documentfoundation.org`, OBS Studio and HandBrake from their projects'
own GitHub releases, VS Code from Microsoft's own download server. HandBrake
names "https://github.com/HandBrake" among "the only official places where
HandBrake can be downloaded from" (section 6.3).

### 2.6 Administrator rights

- Schema 1.28.0, `installer.md`, `ElevationRequirement`:
  - "**elevationRequired** — The installer always requires administrator
    privileges to complete (e.g., it writes to `Program Files`, modifies system
    registry keys, or installs a Windows service). If WinGet is running in a
    non-elevated context, it will launch the installer elevated via a UAC
    prompt." (the file's own dash)
  - "**elevatesSelf** — The installer contains its own logic to determine
    whether elevation is needed and will request it when necessary." and
    "WinGet will not elevate the installer itself but will warn the user that a
    UAC prompt may appear."
- "Use WinGet" (2.1): "When running WinGet without administrator privileges,
  some applications may require elevation to install. When the installer runs,
  Windows will prompt you to elevate. If you choose not to elevate, the
  application will fail to install." and "When running WinGet in an
  Administrator Command Prompt, you will not see elevation prompts if the
  application requires it." (The page itself warns: "Always use caution when
  running your command prompt as an administrator, and only install
  applications you trust.")
- "Troubleshooting",
  https://learn.microsoft.com/en-us/windows/package-manager/winget/troubleshooting
  (ms.date 2026-07-19, updated 2026-07-20): "Packages installed in user scope
  may still require UAC (User Account Control) authorization from an
  administrator." and "the WinGet CLI is not supported in the system context."
  (fragment)
- The client's message (winget.resw): "The installer will request to run as
  administrator. Expect a prompt."
- In the catalog: Blender's manifest says `Scope: machine` and
  `ElevationRequirement: elevatesSelf`; LibreOffice, OBS Studio and HandBrake
  are `Scope: machine`; VS Code and GIMP offer both a per-user and a
  per-machine installer. The installer types differ too: Blender `wix` and
  LibreOffice `msi` (Windows Installer packages), OBS Studio and HandBrake
  `nullsoft`, VS Code `inno`.

### 2.7 Failure modes

The return codes table, `doc/windows/package-manager/winget/returnCodes.md` in
winget-cli (its front matter says "ms.date: 05/13/2026"; latest commit 1
October 2026; "This file is generated by running: winget error --output
<file>"). Microsoft Learn has no page for it (the address
`.../winget/returncodes` answers 404), and its troubleshooting page sends
readers to this file. Rows that a click-to-install button must handle, quoted
exactly:

| Code | Name | Description |
|---|---|---|
| 0x8A150010 | `NO_APPLICABLE_INSTALLER` | "None of the installers are applicable for the current system" |
| 0x8A150011 | `INSTALLER_HASH_MISMATCH` | "The installer file's hash does not match the manifest" |
| 0x8A150041 | `PACKAGE_AGREEMENTS_NOT_ACCEPTED` | "Package agreements were not agreed to" |
| 0x8A150046 | `SOURCE_AGREEMENTS_NOT_ACCEPTED` | "Source agreements were not agreed to" |
| 0x8A150061 | `PACKAGE_ALREADY_INSTALLED` | "Found at least one version of the package installed." |
| 0x8A15010C | `INSTALL_CANCELLED_BY_USER` | "You cancelled the installation." |
| 0x8A150109 | `INSTALL_REBOOT_REQUIRED_TO_FINISH` | "Restart your PC to finish installation." |
| 0x8A15003A | `BLOCKED_BY_POLICY` | "Operation is blocked by Group Policy" |
| 0x8A15010F | `INSTALL_BLOCKED_BY_POLICY` | "Organization policies are preventing installation. Contact your admin." |
| 0x8A150101 | `INSTALL_PACKAGE_IN_USE` | "Application is currently running. Exit the application then try again." |
| 0x8A150105 | `INSTALL_DISK_FULL` | "There's no more space on your PC. Make space, then try again." |
| 0x8A150108 | `INSTALL_CONTACT_SUPPORT` | "This application encountered an error during installation. Contact support." |

(Each name carries the prefix `APPINSTALLER_CLI_ERROR_`.) The troubleshooting
page adds: "Many .exe-based installers have non-standard error codes and may
not be displayed." Two more failures sit before any of these: winget missing
("It's possible that App Installer is not installed on your device.", 2.1), and
a UAC prompt the person declines ("If you choose not to elevate, the
application will fail to install.", 2.6).

### 2.8 What winget sends to Microsoft

- winget-cli README: "The winget.exe client is instrumented to collect usage
  and diagnostic (error) data and sends it to Microsoft to help improve the
  product." and "The winget.exe client respects machine-wide privacy settings
  and users can opt out on their device" (fragment).
- `PRIVACY.md` in winget-cli (latest commit 2 February 2023): "Your use of the
  software operates as your consent to these practices."
- "winget settings",
  https://learn.microsoft.com/en-us/windows/package-manager/winget/settings
  (ms.date 2026-07-19, updated 2026-07-21): "If set to true, the
  telemetry.disable setting will prevent any event from being written by the
  program."

A Tools page that drives winget therefore also causes Microsoft's client to send
Microsoft usage data under Microsoft's terms, unless the person has opted out
in Windows.

### 2.9 The catalog's programs that are not plain free software

Whether a program is in winget's community repository was checked by listing
every publisher folder of https://github.com/microsoft/winget-pkgs on the day
(repository head 20 October 2026, 01:35 UTC); GitHub's code search does not
index that repository, so a name search proves nothing.

**DaVinci Resolve** (Blackmagic Design, proprietary, free version and paid
Studio version).

- Product page, https://www.blackmagicdesign.com/products/davinciresolve (no
  date shown): "That’s why there’s a free version of DaVinci Resolve, so you can
  learn how to use the same tools that professional Hollywood artists use."
- The download list the support page is built from,
  https://www.blackmagicdesign.com/api/support/us/downloads.json (entry dated
  "02 Oct 2026"), marks the free version
  `"requiresRegistration":true,"requiresTermsAndConditions":false,"termsAndConditions":null`
  (for "DaVinci Resolve 21.1.1 Update"), and the paid Studio version of the same
  day `"requiresRegistration":false`.
- The download form, https://www.blackmagicdesign.com/support/modal/download-start
  (no date shown), marks as required "First Name *", "Last Name *", "Email *",
  "Phone *", "Street *" and "City *", and says "Your details will be registered
  on the Blackmagic Design database to receive occasional news of important
  software updates, new products and services." Its "Download Only" button
  appears only for items that do not require registration (the page's code:
  `ng-if="!downloadItem.requiresRegistration"`). So the free version is not
  downloaded without filling in the form.
- **Negative finding:** no copy of Resolve's licence agreement on Blackmagic's
  site, and none shown at download (`"termsAndConditions":null`). A 2021 forum
  post on PortableApps.com (https://portableapps.com/node/64312 , 27 June 2021)
  quotes the licence as saying "You may install the Software on a single
  computer device for access and use by one end user at any one time"; that is
  a third party's copy, not Blackmagic's.
- **Not in winget.** No Blackmagic or DaVinci publisher folder. A request to
  add it (winget-pkgs issue 296774, 24 September 2025) was answered "Hence this
  package is not supported." An earlier thread (issue 207784, 10 January 2025)
  explains: "Unfortunately the official site also has dynamic token parameters
  in the URL." Homebrew has no cask for it either.

**Paint.NET** (the catalog says "MIT"; it is not). Licence,
https://www.getpaint.net/license.html ("License last updated: February 22,
2019"): "You may not modify, adapt, rent, lease, loan, sell, or create
derivative works based upon the Software or any part thereof." and
"Redistribution of the "Classic" release of Paint.NET is permitted so long as
it is free-of-charge." and "Before using "Classic" release of Paint.NET, you
must accept the License Agreement as part of the setup process." In winget as
`dotPDN.PaintDotNet`, `License: Proprietary (Freeware)`, installer from the
publisher's GitHub releases, no `Agreements` field. **So winget's default
silent install would skip the very step the licence says must happen.**

**Aseprite** (the catalog says "GPL (source)"; it is not). FAQ,
https://www.aseprite.org/faq/ (no date shown): "From August 2016 you cannot
redistribute compiled versions of Aseprite. We have replaced the General Public
License (GPLv2) with the new Aseprite EULA." (fragment) and "$19.99 /USD is the
minimum pledge amount to get the convenient .exe and .dmg files of the 1.x
version series of Aseprite" (fragment). EULA,
https://github.com/aseprite/aseprite/blob/main/EULA.txt (latest commit 6
December 2018): "You may only compile and modify the source code of the
SOFTWARE PRODUCT for your own personal purpose or to propose a contribution to
the SOFTWARE PRODUCT." Not in winget.

**Visual Studio Code** (the catalog says "MIT"; that is the source code, not the
download). Licence, https://code.visualstudio.com/license (no date shown):
"This license applies to the Visual Studio Code product. Source Code for Visual
Studio Code is available at https://github.com/Microsoft/vscode under the MIT
license agreement" (fragment), and "The software may collect information about
you and your use of the software, and send that to Microsoft." (fragment) FAQ,
https://code.visualstudio.com/docs/supporting/faq (ms.date 02/04/2026): "Yes,
VS Code is free for private or commercial use." In winget as
`Microsoft.VisualStudioCode`, `License: Microsoft Software License`, no
`Agreements` field.

**Ardour** (GPLv2 source, paid ready-made builds). FAQ, https://ardour.org/faq.html
(the server's Last-Modified 24 September 2026): "If you want the convenience of
using our ready-to-run version and/or for support from Ardour developers and
experienced users, we ask that you pay something for this." and, of the free
demonstration build, "I paid for the full version, but it keeps going silent
after 10 minutes" (a question heading). Not in winget.

**Zed** (GPL source, application under Zed's terms; the catalog lists no Windows
build, but one is offered now). Download page, https://zed.dev/download (shows
1.24.2, "October 14, 2026"): "By downloading and using Zed, you agree to its
terms and conditions." Terms, https://zed.dev/terms ("Last Updated : March 2,
2026", as written): "These Terms of Service govern your use of the Zed
application distributed by Zed Industries and Zed-operated services accessed
through it." (fragment) FAQ, https://zed.dev/faq : "Zed supports macOS, Linux,
and Windows." In winget as `ZedIndustries.Zed`, `License: GPL-3.0`,
`Scope: user`, no `Agreements` field. So the person agrees to Zed's terms by
downloading from the site, and winget's data does not mention them.

**Vectr** (an online service): terms of use, https://vectr.com/en/terms-of-use
("Last updated: 13 May 2026"), grant use "solely for Your own personal,
non-commercial use" (fragment). **Boxy SVG**: Flathub records its licence as
`"is_free_license":false` (https://flathub.org/api/v2/appstream/com.boxy_svg.BoxySVG).
Neither is in winget.

**Everything** (voidtools): its licence, https://www.voidtools.com/License.txt
(server Last-Modified 27 April 2026), is MIT wording ("Copyright (C) 2018 David
Carpenter"); the FAQ says "Yes, "Everything" is Freeware." In winget as
`voidtools.Everything`, `License: MIT`.

**7-Zip**: https://www.7-zip.org/license.txt (server Last-Modified 6 October
2026): "The "GNU LGPL" with "unRAR license restriction" for some code"
(fragment).

**MuseScore**: the default Windows download goes through a separate installer,
MuseHub ("MuseScore Studio 64-bit without MuseHub" is offered beside it,
https://musescore.org/en/download); winget's `Musescore.Musescore` installs the
plain installer from the project's GitHub releases.

The rest of the catalog's Windows programs carry ordinary free-software
licences (GPL, LGPL, MPL, BSD, Apache, MIT) whose texts were not re-read for
this finding.

### 2.10 An Install button and California's "covered application store"

The 2026-10-10 finding read California's Digital Age Assurance Act, as rewritten
by AB 1856, and found HumanityOS probably outside it today. That finding
considered HumanityOS as an *app* listed in someone else's store. An Install
button raises the other question: whether HumanityOS would itself be a store.
AB 1856, chaptered 10 September 2026, read on the day in the built-in browser at
https://leginfo.legislature.ca.gov/faces/billTextClient.xhtml?bill_id=202520260AB1856
("Date Published: 09/11/2026 02:00 PM"; direct requests answered 403, as on
10 October):

- "“Covered application store” means a publicly available internet website,
  software application, online service, or platform that distributes and
  facilitates the download of applications from third-party developers"
  (fragment).
- "(c) A covered application store shall do both of the following with respect
  to a user of the covered application store:" "(1) Request a signal from the
  user’s operating system provider." "(2) Provide the signal received pursuant
  to paragraph (1) to a developer upon request."
- "(e) A person shall not request a signal with respect to a particular user
  from an operating system provider or a covered application store if not
  required to do so by this title or any other applicable law."

The definition joins "distributes and facilitates". A Tools page that only
links to each maker's website distributes nothing. A page that runs winget
facilitates a download, and whether it also "distributes" when the file comes
from the maker's server through Microsoft's client is not answered by the text
read. The act takes effect on 1 January 2027 (2026-10-10 finding). **This is
the one place in this finding where the Install button, rather than the rest of
the page, changes a legal question.**

## 3. The alternatives, briefly

### 3.1 Downloading the installer ourselves (Windows)

HumanityOS would fetch the file from the maker's site and run it. Then the
checking winget does (2.5) becomes ours. What the makers publish for it:

- GIMP's downloads page lists a "SHA256 hash sum for gimp-3.2.6-setup.exe"
  (fragment) beside each file and a VirusTotal link; Blender's lists "Checksums
  md5 sha256"; Audacity's "Checksum (SHA-256)"; LibreOffice's mirror list
  "SHA-256 Hash" and "PGP signature available" (fragments).
- KeePassXC, "Verifying Signatures", https://keepassxc.org/verifying-signatures/
  (latest commit to its source 10 April 2023): "Each of these package files has
  two related sidecar files, a *.sig containing a PGP signature and a *.DIGEST
  containing the SHA-256 hash for basic integrity checks." and "The Windows MSI
  installation file is protected by an authenticode signature, this means that
  authenticity and integrity checks are verified directly by Windows when you
  run the program." It also says: "If you downloaded KeePassXC directly from
  our website, there is no need for further verification of the downloaded
  files." (fragment)
- Windows itself, `WinVerifyTrust`,
  https://learn.microsoft.com/en-us/windows/win32/api/wintrust/nf-wintrust-winverifytrust
  (ms.date 2018-12-05, updated 2025-08-15): "For example, the Software Publisher
  Trust Provider can verify that an executable image file comes from a trusted
  software publisher and that the code in the file has not been modified since
  it was signed."

A hash read from the same site as the file proves the file was not damaged on
the way; it does not prove the site was not compromised, which is what happened
to HandBrake's mirror in 2017 (6.3). A publisher's signature, checked against a
publisher name the catalog holds, does. Not every catalog program signs its
Windows installer; which do was not checked.

### 3.2 macOS later: Homebrew casks

- Licence: Homebrew's `LICENSE.txt` (https://github.com/Homebrew/brew , latest
  commit 3 January 2017) and the cask repository's (latest commit 4 October
  2026) are the "BSD 2-Clause License".
- Other programs using it: "brew info can output JSON-formatted information
  about formulae. This JSON can then be parsed using your tools of choice."
  (https://docs.brew.sh/Querying-Brew , source commit 19 July 2026)
- Which apps it carries: "A cask must use a download published by the developer
  or by a distribution source the developer publicly endorses."
  (https://docs.brew.sh/Acceptable-Casks , source commit 25 August 2026)
- Root: "Running Homebrew as root is extremely dangerous and no longer
  supported." (`Library/Homebrew/brew.sh`, commit 17 October 2026)
- Analytics: "Homebrew collects aggregate, anonymous usage analytics in
  InfluxDB." and "Homebrew displays a notice before analytics are enabled so a
  user can opt out before sending an event." (https://docs.brew.sh/Analytics ,
  source commit 24 July 2026)
- **Negative finding:** Homebrew publishes no terms of use (none on brew.sh or in
  its documentation). Homebrew is not installed on a Mac by default, so a
  macOS Tools page would have to install Homebrew first or link to it. VS Code
  and Zed have casks; DaVinci Resolve, Aseprite and Ardour do not.

### 3.3 Linux later: Flatpak and Flathub

- Flatpak documentation, https://docs.flatpak.org/en/latest/using-flatpak.html
  (source commit 1 July 2026): "Flatpak commands are run system-wide by
  default." and "To do this, use the --user option, which can be used in
  combination with most flatpak commands." (fragment); "Applications and
  runtimes that are installed per-user are only available to the users that
  installed them."
- Flathub's requirements, https://docs.flathub.org/docs/for-app-authors/requirements
  (source commit 4 October 2026): "All content hosted on Flathub must allow
  legal redistribution, and the license must be correctly specified in the
  app's MetaInfo file." and "Submissions are accepted at the discretion of the
  reviewers" (fragment).
- Verification, https://docs.flathub.org/docs/for-app-authors/verification
  (source commit 31 July 2026): "Verification is the process by which Flathub
  and developers confirm that an app is published by the original developer or
  an authorized party." For users
  (https://docs.flathub.org/docs/for-users/verification, commit 22 January
  2026): "Some apps are published on Flathub by the community or third parties
  and not directly maintained by the original developer."
- Terms, https://flathub.org/en/terms-and-conditions (no date shown), a general
  website template beginning "Flathub offers this website, including all
  information, tools and services available from this site to you, the user,
  conditioned upon your acceptance of all terms, conditions, policies and
  notices stated here." (fragment) It does not mention other programs using
  Flathub.
- The makers point there themselves: OBS Studio "is officially distributed as a
  Flatpak on Flathub" (fragment), HandBrake names "FlatHub" among its only
  official places, KeePassXC marks its Flatpak "Recommended" (6.3).

A Flatpak listing is shown with its licence and its verified or unverified
status, which is the information section 2.3 says winget's data often lacks.

## 4. Starting an installed program

### 4.1 What Windows documents for it

- "Launching Applications (ShellExecute, ShellExecuteEx, SHELLEXECUTEINFO)",
  https://learn.microsoft.com/en-us/windows/win32/shell/launch (ms.date
  2025-07-02, updated 2025-07-03): "If the file of interest is an executable,
  your application might want to simply launch it. This document discusses how
  to use ShellExecute or ShellExecuteEx to perform these tasks." and "For .exe
  files, open simply launches the application."
- `CreateProcessW`,
  https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw
  (ms.date 2018-12-05, updated 2025-07-01): "Creates a new process and its
  primary thread. The new process runs in the security context of the calling
  process." Its security remarks, on paths with spaces: "To avoid this problem,
  do not pass NULL for lpApplicationName." (fragment) And: "Note that this
  function does not search the per-application path specified by the App Paths
  registry key. To include this per-application path in the search sequence,
  use the ShellExecute function."
- Store apps start differently: `IApplicationActivationManager::ActivateApplication`,
  https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nf-shobjidl_core-iapplicationactivationmanager-activateapplication
  (ms.date 2018-12-05, updated 2024-02-22): "Activates the specified Windows
  Store app for the generic launch contract (Windows.Launch) in the current
  session."

### 4.2 The programs' own licences on running them

- GNU GPL version 3 ("Version 3, 29 June 2007"),
  https://www.gnu.org/licenses/gpl-3.0.txt , section 2: "This License
  explicitly affirms your unlimited permission to run the unmodified Program."
  Section 9: "You are not required to accept this License in order to receive
  or run a copy of the Program." (fragment)
- GNU GPL version 2 ("Version 2, June 1991"),
  https://www.gnu.org/licenses/old-licenses/gpl-2.0.txt , section 0: "The act
  of running the Program is not restricted" (fragment).
- Mozilla Public License 2.0 (LibreOffice), https://www.mozilla.org/en-US/MPL/2.0/
  (no date shown), 2.1: each contributor grants a licence "to use, reproduce,
  make available, modify, display, perform, distribute, and otherwise exploit
  its Contributions" (fragment).

**Negative finding:** no licence or policy read for this finding limits which
program may start another, or how. The proprietary licences (VS Code's, DaVinci
Resolve's) are covered in section 2.9.

### 4.3 Blender installed through Steam

- Steam store page, https://store.steampowered.com/app/365670/Blender/ (no page
  date): the page's own address and data give App ID 365670; "Developer:
  Blender Foundation Publisher: Blender Foundation" (the page's labels run into
  the names); "Blender is the free and open source 3D creation suite. Free to
  use for everyone, for any purpose."
- Blender manual, "Installing from Steam",
  https://docs.blender.org/manual/en/latest/getting_started/installing/steam.html
  ("Last updated on 2026-10-19"): "Blender should now be available in the
  Library tab of the Steam client, where it can be launched." and "When an
  update for Blender is available on Steam, Steam will automatically download
  and apply the update for you."

Starting a Steam copy through Steam's own link (`steam://rungameid/365670`) is
the route the 2026-10-13 finding settled for games, and nothing about Blender
being software rather than a game changes it. Steam's own install records,
which the Games page already reads, show where a Steam copy is.

## 5. Icons and logos

Two separate rights meet in a program's icon. **Copyright** covers the picture
as a piece of art: who drew it, and the licence it carries (often the program's
own free licence, sometimes Creative Commons, sometimes all rights reserved).
**Trademark** covers the picture as a sign of who makes the program: it is about
confusion and endorsement, not copying. A use can be fine under one and not the
other. The games finding (1.3 and 2.4) already recorded that showing a picture
to the person at their own computer is not a *public* display, and that naming
another's product plainly is nominative fair use while a company's "distinctive
lettering" is not.

### 5.1 The icon read from the installed program itself

- `ExtractIconExW`,
  https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-extracticonexw
  (ms.date 2018-12-05, updated 2025-10-31): "The ExtractIconEx function creates
  an array of handles to large or small icons extracted from the specified
  executable file, DLL, or icon file."
- `SHGetFileInfoW`,
  https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shgetfileinfow
  (ms.date 2018-12-05, updated 2025-07-01): with `SHGFI_ICON`, "Retrieve the
  handle to the icon that represents the file and the index of the icon within
  the system image list."
- `IExtractIconW`,
  https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nn-shlobj_core-iextracticonw
  (ms.date 2018-12-05, updated 2024-11-20): "The Shell uses IExtractIcon to
  retrieve icons when it displays the contents of a folder."

So the icon in a program's own file is what Windows itself shows for that
program in File Explorer, the Start menu and Add or Remove Programs (1.1,
`ARPPRODUCTICON`). The Games page already reads it from a game's program file,
as data, without running anything (`src/games/art.rs`).

What the projects say about their icons as copyrighted art:

- Blender FAQ, https://www.blender.org/support/faq/ (no date shown): "The
  splash-screen and icons are GPL’d material therefore when using them the
  terms of the GPL must be followed." The GPL governs copying and passing on
  (4.2); showing an icon on the person's own screen is neither.
- KeePassXC's `COPYING`,
  https://github.com/keepassxreboot/keepassxc/blob/develop/COPYING (latest commit
  19 June 2025): the application icon files
  (`share/icons/application/scalable/apps/keepassxc.svg` and its variants)
  carry "Copyright: 2016, Lorenzo Stella <lorenzo.stl@gmail.com> License:
  LGPL-2", and the same files appear again in a later block under "License:
  Apache-2.0" from Pictogrammers. Under Debian's file format, "The last stanza
  that matches a particular file applies to it."
  (https://www.debian.org/doc/packaging-manuals/copyright-format/1.0/). Either
  way, an open licence.
- KDE's application icons (Kdenlive, Okular), `COPYING-ICONS` in
  https://github.com/KDE/breeze-icons (latest commit 31 August 2015): "The
  exception in section 5 of the GNU Lesser General Public License covers the use
  of elements of this art library in a GUI."
- HandBrake's `graphics/LICENSE`,
  https://github.com/HandBrake/HandBrake/blob/master/graphics/LICENSE (latest
  commit 28 December 2017): "Unless otherwise specified, all graphic assets are
  released under the Creative Commons Attribution-ShareAlike 4.0 International
  Public License."
- **Against, read literally:** Visual Studio Code's icon guidelines,
  https://code.visualstudio.com/brand (no date shown), list among icons not to
  use "…or the app icon…", whose picture is captioned "Do not use: VS Code app
  icon (for example the macOS app icon)" (the image's alt text). Microsoft's
  general Trademark and Brand Guidelines,
  https://www.microsoft.com/en-us/legal/intellectualproperty/trademarks (no
  date shown): "our logos, app and product icons, illustrations, photographs,
  videos, and designs can never be used without an express license"
  (fragment). These are written about branding and marketing; neither page
  mentions a program list on the person's own computer, which is exactly where
  Windows itself shows the same icon.

### 5.2 Each project's logo, from its own policy

**Blender** (Blender Foundation). Logo page, https://www.blender.org/about/logo/
(no date shown; the site's own page record says modified 7 August 2025), and
trademark policy, https://www.blender.org/about/trademark-policy/ (same record,
created 18 April 2025, modified 7 August 2025):

- "The Blender logo is a registered property of Blender Foundation."
- "Third parties can only use it to provide credit for the product Blender or to
  link to blender.org."
- "The Blender logo can be placed among other logos, for example when listing
  3D software."
- "The logo is used unaltered, without fancy enhancements, in original colors,
  original typography, and always complete (logo + text blender)."
- "please do not use the logo in isolation" (fragment)
- Trademark policy, without permission: "Use Blender logos in visuals to
  truthfully refer to and/or to link to the applicable programs, products,
  services and technologies hosted on Blender servers."
- "Don’t use the Blender trademark in a way that incorrectly implies
  affiliation with, or sponsorship, endorsement, or approval by Blender
  Foundation of your products or services."
- "The logo and the brand name “Blender” are not part of the GNU GPL"
  (fragment).

**GIMP.** "Citing GIMP and Linking to Us", https://www.gimp.org/about/linking.html (no date
shown), and the logo README in GIMP's `gimp-data` repository (read from its
GitHub mirror,
https://github.com/GNOME/gimp-data/blob/main/images/logo/README.md ; latest
commit 22 April 2026):

- "most images on news articles or on the main pages can be copied and used
  freely elsewhere. This includes our logo (Wilber) shown below." (fragment)
- The current Wilber is "available as Creative Commons Attribution-ShareAlike
  4.0" (fragment), by Aryeom Han.
- "Even licensed as Libre Art, images are still copyrighted by their respective
  authors."
- "we are not into enforcing strong branding guidelines to others" (fragment)
  and "Yet when reusing or modifying our logo, we ask of people not to pretend
  to be the GIMP team if you are not."
- **Negative finding:** no GIMP trademark usage policy; the user FAQ says only
  that the project has been "fighting for the trademark" (fragment).

**Krita** (Stichting Krita Foundation). "Krita Trademark Policy",
https://krita.org/en/posts/2013/krita-trademark-policy/ ("Monday, 22 April
2013"):

- "Krita is a trademark owned by the Stichting Krita Foundation."
- "Throughout this document, the "Marks" refers to the Krita name and logo,
  individually or together."
- "You may use the Krita Mark without prior written permission for the
  following purposes (subject to the other sections):" including "To identify
  Krita as a distinct component of a software offering;"
- Not without permission: "In any way that indicates a greater degree of
  association between you and the Krita project than actually exists;"
- On the project's forum, 7 June 2022, Halla Rempt, Krita's maintainer
  (https://krita-artists.org/t/usage-of-krita-logos/42355): "It is fine if you
  use the Krita logo to refer to Krita in manuals and other places!" (fragment)
- **Negative finding:** no copyright licence for the logo image was found.

**Inkscape** (marks held by Software Freedom Conservancy). "Branding and
Trademark", https://inkscape.org/about/branding/ (no date shown; the page's
revision metadata says 30 June 2026):

- "The Inkscape Logo is a registered trademark (see below), and is licenced as
  CC-By-SA 3.0. It's original author is Andrew Michael Fitzsimon." (the page's
  spelling)
- "You may use the Marks without prior written permission (subject to the
  other sections):" including "To identify Inkscape software as a distinct
  component of a software offering." and "To factually refer to the Inkscape
  Project itself, its products, or its protocols."
- "In any way that indicates that Inkscape favors one distribution, platform,
  product, etc. over another except where explicitly indicated in writing by
  Inkscape." (among the uses not allowed)
- "The first prominent mention of the name should be immediately followed by a
  symbol for trademark" (fragment), with "We encourage the use of the symbol
  whenever possible, but recognize that many non-commercial and informal uses
  will omit it."

**LibreOffice** (The Document Foundation). The live wiki answered with a
bot-check page; read from Internet Archive copies. Trademark Policy
(https://web.archive.org/web/20260106114144/https://wiki.documentfoundation.org/TDF/Policies/Trademark_Policy ,
"This page was last edited 09:07, 14 May 2020"), Logo Policy (archive copy of 7
November 2025, last edited 23 September 2023), Design/Branding (archive copy of
11 January 2026, last edited 9 April 2025):

- "Marks cover the names "The Document Foundation" and "LibreOffice" as well as
  the paper clip icon."
- "Except as outlined below, you may use the Marks without prior written
  permission (subject to the following terms):" including "2. To identify
  LibreOffice as a distinct component of a software offering."
- "Websites and software that are not created or produced by TDF should not
  imply, either directly or by omission, that they are."
- "Individual community members and other people referring to our product and
  the community should use the logo without the subline."
- Design/Branding: "* You may use a stand alone "Document Symbol"." and "*
  Don't change the logo by e.g. adding additional graphic elements to the
  "Document Symbol"."
- "The first or most prominent mention of a TDF mark should be accompanied by a
  symbol indicating whether the mark is a registered trademark ("®") or an
  unregistered trademark ("™")." (waived in some contexts)

**OBS Studio** (Wizards of OBS LLC). Site footer, https://obsproject.com/ :
"OBS, OBS Studio, Open Broadcaster Software, and the OBS Studio logo are
registered trademarks of Wizards of OBS LLC." No brand guidelines page exists
(`/brand`, `/press`, `/trademark` answered 404). The nearest official text is
the forum's "Resource and IP Policy",
https://obsproject.com/forum/threads/forum-resource-and-ip-policy.178569/
(posted 8 August 2024 by the forum admin; "Last edited: Jul 29, 2026"):

- "These rules apply to ANY application, tool, or plugin you create, not only
  ones submitted to the Resources system" (fragment)
- "Do not use the OBS logo or IP without authorization"
- "the OBS Studio logo and other associated OBS Project assets should not be
  used in your resource icons, marketing, or other visual assets." (fragment)
- "It is permissible to include the OBS logo or application screenshots on
  websites or marketing material only when it is made clear that your
  application or plugin is a third party tool or extension."
- A forum question asking whether the logo is under the GPL (30 March 2024,
  https://obsproject.com/forum/threads/obs-logo-license-question.174461/) has no
  answer.

**Audacity** (the site's copyright line names "Muse Group & contributors.").
**Negative finding:** no current trademark or logo policy; `/trademark/`
answers 404 and the legal page (https://www.audacityteam.org/legal/) lists none.
The old official permissions page now redirects to the licence file; its last
archived content (Internet Archive, 2 October 2023) said "The name Audacity® is
a registered trademark." and "Inappropriate use of the Audacity trademark, for
example to imply endorsement by Audacity, is not acceptable" (fragment). On the
forum, 4 December 2023, a team member answered a request to credit Audacity
with its logo: "Yes, that’s OK to do."
(https://forum.audacityteam.org/t/use-logo-audacity/92052).

**Visual Studio Code** (Microsoft). Icon guidelines,
https://code.visualstudio.com/brand (no date shown):

- Allowed: "Using the icon to link to our website" (fragment) and "Using the
  icon in a documentation or tutorial (in both text and video formats), blog
  post, news article about Visual Studio Code."
- Not allowed: "Using the icon as a lock-up for Visual Studio Code or another
  brand (for example, don't use the icon as a logo paired up with the “Visual
  Studio Code” name)." and "Creating a modified version of the icon, for
  example:" (fragment)
- "Send your brand question to vscode at microsoft.com" (as written).
- Microsoft's general guidelines (5.1): "In general, wordmarks can be used to
  truthfully convey information about your product or service, as long as
  customers and the public will not be confused into believing Microsoft is
  affiliated with or endorses your product or service."

**KeePassXC.** No trademark policy found (home page, team page, `COPYING`); the
icon's open licences are in 5.1. "KeePassXC is released under the GPLv3."

**DaVinci Resolve** (Blackmagic Design). The DeckLink SDK documentation's
copyright page, https://sdk-doc.blackmagicdesign.com/decklink-sdk/copyright.html
("© Copyright 2003-2026") lists "‘Blackmagic’, ‘Blackmagic Design’,
‘DaVinci’, ‘Resolve’" (fragment) among names that "are registered trademarks in
the US and other countries." Site footer: "All items on this website are
copyright Blackmagic Design Pty. Ltd. 2026, all rights reserved. All trademarks
are property of their respective owners." **Negative finding:** no logo-use
policy (`/legal` answered 404; the media pages carry none). Three forum threads
on asking to use the Resolve logo could not be read (see "Sources that could not
be reached").

### 5.3 The rest of the catalog, briefly

- **FreeCAD**, logo guidelines, https://fpa.freecad.org/handbook/process/logo.html
  (latest commit to its source 3 June 2026): "Third parties can only use it to
  provide credit for FreeCAD or to link to freecad.org" and "You should not
  alter colors, shape, style, or apply any effects such as gradients, borders,
  shadows, or outlines." Also "When used with other logos and partner brands,
  all logos should be of equal size and have enough distance to each other."
- **KDE** (Kdenlive, Okular), https://kde.org/stuff/clipart/ (latest commit to
  its source 7 September 2022): "Copying of the KDE Logo is subject to the LGPL
  copyright license. Trading and branding with the KDE Logo is subject to our
  trademark licence." Kdenlive, https://kdenlive.org/trademark-logo/ :
  "Kdenlive™, Kde’n’live™ and the playhead logo are trademarks of KDE e.V."
  **Negative finding:** no current KDE trademark policy page (`/trademark/`
  answered 404; a 2020 mailing-list post called it "The missing piece").
- **Shotcut**: "Shotcut is a trademark of Meltytech, LLC." (https://shotcut.org/),
  no usage policy. **MuseScore, Notepad++, 7-Zip, ShareX**: no trademark or logo
  policy found.

**The pattern.** Where a project has a written policy, it allows its logo to
refer to or link to its own program without asking, on four conditions that
recur in almost the same words: unaltered, no implied endorsement or closer
association than exists, the maker's name and site kept clear, and (for
Inkscape, LibreOffice, Audacity) a ® or ™ on first prominent mention. GIMP,
HandBrake and KDE add an open copyright licence with attribution. Two stand
apart: OBS ("Do not use the OBS logo or IP without authorization") and
Microsoft for VS Code (the icon allowed only to link to its website, never as a
"lock-up" with the name). Blackmagic publishes nothing. **No policy read
mentions a program list on the person's own computer.**

## 6. Where to get software safely

### 6.1 Microsoft's advice to ordinary people

- "Protect your PC from unwanted software", Microsoft Support,
  https://support.microsoft.com/en-us/windows/security/threat-malware-protection/protect-your-pc-from-unwanted-software
  (ms.date 03/16/2026, updated 2026-09-28): "Download apps only from trusted
  sources, such as Microsoft Store, and be thoughtful about which apps you
  install." It describes a "trojan horse" as something "that pretends to be
  something it's not in order to trick you into installing it" (fragment).
- "How to install programs from online sources on Windows", Microsoft Support,
  https://support.microsoft.com/en-us/windows/apps/how-to-install-programs-from-online-sources-on-windows
  (ms.date 01/09/2025, updated 2026-09-28): "You should download and install
  programs only from trusted publishers and retail websites."
- "Protect my PC from viruses", Microsoft Support,
  https://support.microsoft.com/en-us/office/protect-my-pc-from-viruses
  (ms.date 01/22/2026, updated 2026-09-26): "Be cautious about running
  unrecognized apps downloaded from the Internet. Unrecognized apps are more
  likely to be unsafe."
- "SmartScreen reputation for Windows app developers",
  https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/smartscreen-reputation
  (ms.date 2026-05-04, updated 2026-08-17): "Store-distributed apps are signed
  by a Microsoft certificate and are never subject to SmartScreen download
  warnings."
- And for winget (2.1, 2.3): "Only use secure, trusted sources." and "only
  install applications you trust" (fragment).

### 6.2 The US government's consumer advice

- Federal Trade Commission, "Malware: How To Protect Against, Detect, and
  Remove It",
  https://consumer.ftc.gov/articles/malware-how-protect-against-detect-and-remove-it
  (published 11 October 2024, modified 1 May 2026): "Scammers place bogus ads
  for software on social media sites and on search engines. Don't click on ads
  to download software." and "If you see an ad for software that interests you,
  don’t click on it. Instead, go to the website by typing in the address."
- CISA, "Protecting Against Malicious Code",
  https://www.cisa.gov/news-events/news/protecting-against-malicious-code
  ("Released July 14, 2009 Revised November 19, 2019"; marked as archived):
  "It is not uncommon that free software contains a Trojan horse making a user
  think they are using legitimate software, instead the program is performing
  malicious actions on your computer."

### 6.3 What the projects say

- **HandBrake**, downloads page, https://handbrake.fr/downloads.php (no date
  shown): "Please take note that handBrake.fr, https://github.com/HandBrake and
  FlatHub are the only official places where HandBrake can be downloaded from."
  and "Please be cautious of possible scams. HandBrake is free software and does
  not require an account to use!" Its documentation, "Where to get HandBrake",
  https://handbrake.fr/docs/en/latest/get-handbrake/where-to-get-handbrake.html
  (no date shown): "Beware of third-party websites and peer-to-peer downloads of
  HandBrake." and "Downloaded files with unlisted checksums may be tampered
  with." In May 2017 HandBrake's own download mirror was compromised (forum
  notice of 6 May 2017, read from the Internet Archive copy of 22 December
  2017): "Anyone who has downloaded HandBrake on Mac between [02/May/2017 14:30
  UTC] and [06/May/2017 11:00 UTC] needs to verify the SHA1 / 256 sum of the
  file before running it."
- **Krita**, "Only Download Krita From Trusted Places",
  https://krita.org/en/posts/2022/only-download-krita-from-trusted-places/
  (Wednesday, 28 September 2022): "We were informed that there are compromised
  downloads of Krita being offered, from Russian domains:" (fragment); the
  trusted list begins with krita.org and download.kde.org and continues "the
  Windows Store, Steam, the Epic Store and the Google Play Store (we're working
  on the Apple macOS store...) or your Linux distribution's package manager."
  (fragment); "Third party download sites are outside our control, and can be
  compromised!"
- **GIMP**, downloads page, https://www.gimp.org/downloads/ (current release
  "3.2.6 (2026-09-10)"), lists the Microsoft Store as an official channel and
  warns of Apple's store: "Any downloads on the store are created by third party
  and is not in anyway connected to the GIMP team." (the page's grammar) Its
  2022 news post, https://www.gimp.org/news/2022/06/18/gimp-2-10-32-on-microsoft-store/
  (18 June 2022): "we have no way to know what is actually in a third-party
  package, hence we cannot vouch for them." (fragment)
- **Notepad++**, "Help us to take down the parasite website",
  https://notepad-plus-plus.org/news/help-to-take-down-parasite-site/ (3 April
  2024): "Some users have mistakenly believed that https://notepad.plus/ is the
  official Notepad++ website." And "Trademark Violation: Fake Notepad++ for
  Mac", https://notepad-plus-plus.org/news/npp-trademark-infringement/ (1 May
  2026): "Notepad++ has never released a macOS version."
- **OBS Studio**, on a fake site reached through a search advert (forum, 26
  December 2022, reply by R1CH, listed as forum admin and developer,
  https://obsproject.com/forum/threads/google-ad-serving-fake-obs-website-with-malware.162235/):
  "This isn't limited to OBS either - never click search engine ads for any
  software." Its download page, https://obsproject.com/download (no date shown),
  offers installers, BitTorrent, a zip and GitHub for Windows, and: "OBS Studio
  is officially distributed as a Flatpak on Flathub and a PPA for Ubuntu 24.04
  and newer."
- **KeePassXC**, download page, https://keepassxc.org/download/ (no date shown),
  lists for Windows the Microsoft Store, its own installers with "PGP
  Signature" and "SHA-256 Digest", and under "Windows Package Manager" the
  command `winget install -e --id KeePassXCTeam.KeePassXC`, and Chocolatey.
- **Audacity**, support site (read from its machine-readable copy,
  https://support.audacityteam.org/llms-full.txt , no date shown): "You can
  install Audacity from the [Microsoft Store]" (fragment, with the link markup
  as written) and "Audacity is distributed through WinGet." Also: "We only test
  the builds available on [audacityteam.org](https://audacityteam.org/)."
- **LibreOffice**, https://www.libreoffice.org/download-other/ (no date shown):
  the Microsoft Store and Mac App Store versions carry "a small charge to cover
  the effort of putting software in the app stores" (fragment), and the page
  also links Chocolatey.
- **ShareX**, README, https://github.com/ShareX/ShareX (no date shown): "ShareX
  is available from the official website, GitHub releases, Microsoft Store and
  Steam." and "For the safest download options, use the official links below."
- **Blender**, download page, https://www.blender.org/download/ ("Released
  September 15, 2026", version 5.2.2 LTS), lists the Microsoft Store, the Snap
  Store and Steam beside its own downloads, with "Checksums md5 sha256" for
  each.
- **Negative findings:** no statement about fake download sites was found from
  7-Zip, Blender, LibreOffice, KeePassXC, Inkscape, VS Code, ShareX or
  Blackmagic on their own sites. (Press reports exist for some; they are
  secondary and not used.)

So the projects agree on the rule and differ on the list: get the program from
its maker, or from a store or package manager the maker itself names. Two
projects in the catalog name winget (KeePassXC, Audacity); many name the
Microsoft Store; HandBrake names only its own site, its GitHub and Flathub (and
winget's HandBrake package downloads from that GitHub, 2.5).

## Where the sources pull in different directions

- **Who approves a winget package.** Microsoft's repository page says a
  submission "will be manually reviewed by a moderator" and, in its label
  table, that a clean test pass "will automatically be approved"; the
  repository's FAQ says "each submission requires moderator approval"; its
  validation document says an approved pull request is "**automatically
  merged**". Our reading (a judgement): automated checks run on everything, and a
  person (a Microsoft employee or a community moderator) approves, but the depth
  of that look is not stated. Either way it is less than the maker's own release
  process, and more than an unknown download site.
- **What `--accept-package-agreements` covers.** The install page says it
  accepts agreements "presented by the package installer", which reads as the
  installer's own licence screen; the schema says agreements live in the
  manifest and only verified developers may add them, and the default install
  mode keeps the installer's screen from appearing at all. We take the schema
  and the client's own code as the precise account: the switch answers winget's
  prompt for manifest agreements; the installer's licence page is skipped by the
  silent mode, not by the switch.
- **Whether the `winget` source has an agreement.** The install page names "the
  winget community repository" as an example of a source with terms; no such
  agreement was found, and the client's source records show one only for the
  Store.
- **VS Code's icon.** Microsoft's VS Code page says not to use "the app icon"
  and its general guidelines say icons "can never be used without an express
  license"; Windows itself shows that same icon for the installed program in
  File Explorer and the Start menu, and Microsoft documents interfaces for any
  program to read it (5.1). Both are Microsoft's words. We read the brand pages
  as rules for branding and marketing, not for a program list on the person's
  own screen; that is a judgement a lawyer could disagree with.
- **KeePassXC's icon licence.** The same icon files are listed under LGPL-2
  (2016) and Apache-2.0 (2023) in its `COPYING`. Both are open licences, so the
  answer for this page does not change.

## What is still unknown, and what would settle it

- **Whether an app may tell winget that the person agreed, on the person's
  behalf, and who is bound if it does so without asking.** No Microsoft text addresses
  it (2.3). Settled by Microsoft in writing (the winget-cli issue tracker is the
  public channel) or by a lawyer. Avoided entirely by never passing the switch
  without the person's own "I agree" (judgement call 2).
- **Whether a Tools page that runs winget is a "covered application store"**
  under California's act from 1 January 2027 (2.10). Settled by a lawyer, by
  guidance from the California Attorney General (none found on 10 October), or
  by asking the bill's author's office. Cost: a consultation; the free
  alternatives are a letter and a wait.
- **Whether a program list showing an installed program's own icon is a use
  any brand owner objects to** (Microsoft's VS Code wording, OBS). Settled by
  asking: vscode at microsoft.com (as written on Microsoft's page), and OBS
  through https://obsproject.com/help . One message each.
- **Whether each catalog program's Windows installer is signed, and by whom.**
  Needed only if HumanityOS ever downloads installers itself (3.1). Settled by
  checking each installer's signature on a Windows machine; an afternoon.
- **The text of DaVinci Resolve's licence.** Not published on Blackmagic's site.
  Settled by reading it in the installer, or by asking Blackmagic. Not needed
  for a link to Blackmagic's own page.
- **Whether Blackmagic would let a launcher show Resolve's logo.** Three forum
  threads on exactly this could not be read (below). Settled by asking
  Blackmagic's press contact. Not needed if the page shows the installed
  program's own icon only.
- **How reliably the Uninstall key matches the catalog's programs.** Not a legal
  question. Settled by a test on a real machine with the catalog's programs
  installed in both per-user and per-machine forms.

## Sources that could not be reached

- https://learn.microsoft.com/en-us/windows/package-manager/winget/returncodes
  (and `returnCodes`): HTTP 404, no Internet Archive copy. The table was read
  from the winget-cli repository instead (2.7).
- https://wiki.documentfoundation.org/TDF/Policies/Trademark_Policy , its Logo
  Policy and Design/Branding pages: a bot-check page (Anubis), also in the
  Internet Archive's copies of September and October 2026. Read from the
  archive's 2025 and January 2026 copies (5.2).
- https://leginfo.legislature.ca.gov/ : HTTP 403 to direct requests, as on 10
  October; read in the built-in browser instead (2.10).
- https://www.audacityteam.org/trademark/ , https://www.blackmagicdesign.com/legal ,
  https://kde.org/trademark/ , https://ev.kde.org/trademarks/ ,
  https://musescore.org/en/about/trademark , https://obsproject.com/brand ,
  `/press` and `/trademark`: HTTP 404. Hoped for brand rules; none exist at
  those addresses.
- https://forum.blackmagicdesign.com/viewtopic.php?p=1149449 , `?p=960825` and
  `?p=962003` (threads on permission to use the Resolve logo): HTTP 403 or an
  empty answer, and no Internet Archive copies. Their content is unknown and
  not relied on.
- https://community.kde.org/Legal/Using_External_Trademarks : HTTP 403.
- https://wiki.freecad.org/Artwork and `/Logo`: a bot-check page. FreeCAD's
  logo rules were read from its association's handbook instead (5.3).
- https://support.audacityteam.org/basics/downloading-and-installing-audacity :
  HTTP 404 to the fetch; the same site's `llms-full.txt` was read instead (6.3).
- https://forum.handbrake.fr/viewtopic.php?f=33&t=36364 : HTTP 502; read from the
  Internet Archive copy of 22 December 2017 (6.3).
- https://boxy-svg.com/ : a page built entirely by script, with no readable
  text; its terms pages answered 404. Its licence was read from Flathub (2.9).
- Press reports that 7-Zip's author disowned a look-alike site, that Blender's
  chairman warned of a fake site in 2020, and that The Document Foundation
  disowned a paid Store copy in 2018 were found only in secondary sources and
  are not used.

## What it means for HumanityOS

### Certain (stated by the sources above)

- Microsoft documents the Uninstall key as what installers write and what Add
  or Remove Programs shows, and its own PowerShell documentation reads it to
  find installed software, while saying "There is no guaranteed way to find
  every application on a system." Per-user installs appear only for that user;
  `HKLM\Software` has a separate 32-bit view; packaged apps are listed through
  `PackageManager`. No source forbids another program reading any of these.
- winget's client and its community repository are MIT licensed. Microsoft
  documents a command line, a COM interface and a PowerShell module for driving
  it, and writes that third-party package managers "may leverage" its
  repository. Its client tells the person "This application is licensed to you
  by its owner." and that Microsoft grants no licence to third-party packages.
- Microsoft describes package agreements as something "the user" agrees to,
  and the switch that accepts them as suppressing that prompt, "beneficial when
  scripting". The COM interface accepts both kinds of agreement by default in
  its code. No Microsoft text says a program may accept for a person.
- Most packages carry no `Agreements` entry (only verified developers may add
  one), and winget's default install mode keeps an installer's own licence page
  from appearing. Paint.NET's licence says it "must" be accepted during setup.
- The Ninth Circuit holds a click to be assent only when the person is
  "explicitly advised" that clicking means agreeing to the terms.
- winget checks each installer against a SHA-256 hash, runs automated checks
  and antivirus scans on every submission, expects installers to come from the
  publisher's own release location, and allows anyone on GitHub to submit a
  manifest. Installers needing administrator rights bring up Windows' own UAC
  prompt; declining it makes the install fail. winget sends Microsoft usage
  data unless turned off.
- DaVinci Resolve, Aseprite, Ardour, Vectr and Boxy SVG are not in winget. The
  free DaVinci Resolve download requires Blackmagic's registration form. The
  catalog's licence labels for Paint.NET ("MIT"), Aseprite ("GPL (source)") and
  VS Code ("MIT") do not match the programs as downloaded, and Zed now has a
  Windows build the catalog does not list.
- California's act defines a covered application store as one that
  "distributes and facilitates the download of applications from third-party
  developers" (fragment), effective 1 January 2027.
- Windows documents `ShellExecuteEx` and `CreateProcess` for starting programs;
  the GPL leaves running unrestricted; Blender's Steam copy is App ID 365670.
- Windows documents reading a program's icon from its own file. Blender,
  Krita, Inkscape, LibreOffice and FreeCAD allow their logos to refer to their
  programs without asking, on conditions; GIMP's and HandBrake's are CC BY-SA
  4.0, Inkscape's CC BY-SA 3.0; VS Code's icon is allowed "to link to our
  website" and not as a "lock-up" with the name; OBS says "Do not use the OBS
  logo or IP without authorization"; Blackmagic publishes no rules.
- Microsoft, the FTC and the projects all say: get software from its maker or a
  source the maker names, and never from an advert. KeePassXC and Audacity name
  winget; many name the Microsoft Store; HandBrake names only its own site, its
  GitHub and Flathub.

### Judgement calls (the project's own reading; a lawyer could disagree)

1. **Finding what is installed: read the registry, never write it, and say
   *found* rather than *not installed*.** Read, with `KEY_READ` only as
   `src/games/registry.rs` already does: the `HKLM` Uninstall key in both views
   (`KEY_WOW64_64KEY` and `KEY_WOW64_32KEY`), the `HKCU` Uninstall key, and
   App Paths (both hives) for the program's own file; Steam's records (the
   Games page's readers) for Steam copies; and the current user's Store
   packages through `PackageManager.FindPackagesForUser` with an empty user,
   which needs no administrator rights. Skip entries marked as system
   components. Never use WMI's `Win32_Product` (it repairs installs as it reads
   them). Keep each program's matching rule (its winget identifier, a display
   name pattern, its publisher, its program file name) as data in
   `data/external/catalog.json`, not in code. Because Microsoft says there is no
   guaranteed way, the page should say *Installed* when it finds a program and
   *Not found on this computer* otherwise, never *Not installed* (our draft
   labels).
2. **Installing: yes through winget, on a click, and only after the person
   agrees in HumanityOS's own window; never pass an accept switch silently.**
   The working assumption is confirmed and widened. Before running anything,
   show one panel with: the program and its publisher; its licence by name with
   a link (the manifest's `License` and `LicenseUrl`, or the catalog's own
   licence link); every `Agreements` entry the manifest has, with its link;
   Microsoft's two lines verbatim ("This application is licensed to you by its
   owner." and "Microsoft is not responsible for, nor does it grant any licenses
   to, third-party packages."); that Windows may ask for permission; and that
   Microsoft's installer tool sends Microsoft usage data unless it is turned off.
   The button says what it does, in our draft wording (not a quotation): I
   agree to this licence. Install. Only after that click pass
   `--accept-package-agreements`, for that one package. Name the package exactly
   (`--id <id> --exact`, the identifier held in the catalog) and pin the source
   (`--source winget`), so the Store source and its Terms of Transaction never
   come into it; never pass `--accept-source-agreements`, and if winget ever
   reports a source agreement, show it to the person instead. If the COM
   interface is used rather than the command line, set both acceptance flags to
   false explicitly, since its defaults are true. For a program whose licence is
   not a free-software licence (VS Code, Paint.NET, Zed's terms), also run the
   installer with `--interactive`, so the maker's own licence page appears as
   the maker intended. Never run HumanityOS as administrator to avoid the UAC
   prompt, never pass `--ignore-security-hash`, and turn each return code in
   2.7 into a plain sentence. If winget is missing, say so and offer the
   maker's website. **DaVinci Resolve, Aseprite, Ardour, Vectr and Boxy SVG
   get no Install button**, only the maker's website, and Resolve's entry
   should say the maker asks for your name, address and phone number before the
   free download. Correct the catalog's licence labels (Paint.NET, Aseprite, VS
   Code) and add Windows to Zed.
3. **California: the Install button is fine now; decide before 1 January
   2027.** The act is not in force until then. Before that date the operator
   should decide, ideally with a lawyer's answer to the open question above,
   whether a page that runs winget "distributes and facilitates" downloads. The
   website-link and Start buttons are not in question.
4. **The alternatives: winget now, the maker's website always, no downloader of
   our own.** Fetching installers ourselves would make HumanityOS the thing that
   delivers the file, which is closer to "distributes" (2.10), and would make
   signature checking our job for 40 programs; winget already checks hashes and
   scans submissions. On macOS later, Homebrew casks through `brew` (BSD, a JSON
   interface, never as root), with the analytics notice passed on; on Linux
   later, Flathub with `--user` installs, showing Flathub's verified mark and
   licence. Each is a separate increment.
5. **Starting a program: yes, on a click, through the one launch path the Games
   page already has** (`src/games/launch.rs`): the program's own file with its
   full path given (`lpApplicationName` not empty, per Microsoft's security
   note) and its own folder as the working folder; a Steam copy through
   `steam://rungameid/<appid>` (Blender: 365670); a Store copy through Windows'
   activation interface when that is built. Nothing starts without a click.
6. **Icons: show each installed program's own icon, read in place, to this
   person only; ship no logo files.** Read the icon from the program's own file
   as the Games page does (`src/games/art.rs`), decode it into memory, never
   save a copy in any HumanityOS folder, never upload or sync it, and never put
   it on an in-world screen or anywhere other players can see. For a program
   that is not installed, draw a plain tile with its name in the app's own font
   and colours, as the Games page does. Do not bundle project logos with
   HumanityOS: shipping them is copying and distributing them with every
   download, which brings each project's conditions (credit and licence
   notices for GIMP, Inkscape and HandBrake; ® or ™ marks for Inkscape,
   LibreOffice and Audacity; equal sizes for FreeCAD; never a "lock-up" for VS
   Code) and runs into OBS's "without authorization" and Blackmagic's silence.
   If the operator later wants logos for programs that are not installed, use
   only those whose policies allow it, unaltered, with their credits on an
   About panel, and write to OBS and Blackmagic first. VS Code's own icon is on the
   list Microsoft's page captions "Do not use"; this finding's reading is that showing it
   from the installed program, as Windows does, is not the branding use that
   list is about; if Microsoft says otherwise, draw VS Code as a plain tile.
   Names stay plain words, and the page carries one quiet line, in our draft
   wording (not a quotation): HumanityOS is not made by, or affiliated with,
   the makers of these programs.
7. **Safety: say it plainly on the page.** Draft wording (ours, not a
   quotation): Get programs from their makers. The Website button goes to each
   maker's own site, and Install uses the maker's own installer through
   Microsoft's installer tool. Never download a program from an advert, even a
   top search result. Programs listed here as free are free from their makers'
   own sites; some makers also sell app-store copies to fund their work
   (Krita, LibreOffice) and say so on their own sites, and anyone else asking
   you to pay for one is not the maker. Link this finding from that screen. Check each
   catalog `url` against the maker's own statements once (the catalog's Wings
   3D link is plain `http://`).

- **Say what we will not do, and why, on the screen**, per the project's rule
  on laws, patents and platform holders' rules: no Install button for DaVinci
  Resolve because its maker offers it only through its own registration form
  (a maker's rule), and no project logos because each project sets its own
  rules for them (the makers' rules).

---

This document is a reading of public sources on 19 October 2026. It is not
legal advice, and nobody who wrote it is a lawyer. Package managers, licences
and project policies change often; check their dates before relying on any of
it.
