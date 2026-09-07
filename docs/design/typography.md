# Typography: one typeface, and why the dyslexia font is not it

Research record behind the typography decision. An 18-agent read-only
evaluation: three constraint passes over the repo, eight research subjects
with real sources fetched, four complete stack proposals, and three judges who
each argued against their own pick. The judges split three ways on the
typeface, which is recorded here rather than smoothed over.

## The state we are fixing

Nobody ever chose a typeface for this project. Twice. The native app renders
in Ubuntu-Light, Hack, NotoEmoji and emoji-icon-font because that is what egui
FontDefinitions::default() bundles through the epaint_default_fonts crate. The
website renders in whatever the operating system supplies, from the system
stack at web/shared/theme.css:144, so Windows gets Segoe UI, macOS gets San
Francisco and Android gets Roboto: three different typefaces for three
visitors to the same page, before you compare any of them to the app. There
are zero font files in the repo. data/gui/theme.ron carries four font SIZE
tokens and no family, so typeface sits outside the one-theme-source rule that
governs every colour.

## The evidence, which is one-sided

Ship a well-chosen legible face plus adjustable spacing, size and line-height
controls. Do not ship OpenDyslexic, Dyslexie, or any other "dyslexia typeface"
as a mode. The special-font hypothesis has been tested repeatedly and has
failed: the 2026 meta-analysis by Azzarello, Paek, Hodge, Jameson and Lewis in
Annals of Dyslexia pooled 15 studies, 91 effect sizes, N = 688, and found a
pooled Hedges g of -0.04, 95% CI [-0.15, 0.07], p = 0.5, that is,
indistinguishable from zero and if anything faintly negative. The individual
trials agree: Kuster, van Weerdenburg, Gompel and Bosman (2018, Annals of
Dyslexia 68:25-42) tested Dyslexie on 170 Dutch dyslexic children at text
level and 102 dyslexic plus 45 typical children at word level and found no
speed or accuracy gain, with only 11.6 percent of children in experiment 2
preferring Dyslexie against 38.1 percent for Arial. Wery and Diliberto (2017,
Annals of Dyslexia 67(2):114-127) found OpenDyslexic actively worse than Arial
and Times New Roman on both rate and accuracy, and no participant preferred
it. Joseph and Powell (2022, Dyslexia, 71 children aged 8-12, 37 with
dyslexia) found no difference for word or passage reading. Galliussi, Perondi,
Chia, Gerbino and Bernardis (2020, Annals of Dyslexia 70:141-152, 128 Italian
children) isolated letterform from spacing and found the dyslexia-friendly
letterform contributed nothing at all. The one credible positive result,
Franzen, Stark and Johnson's eye-tracking work with OpenDyslexic, improved
comprehension but left reading speed unaffected, and it did not control for
OpenDyslexic's larger rendered size and wider default spacing, which is
exactly the confound that sank the whole literature. That confound is the key
finding for us: Marinus, Mostard, Segers, Schubert, Madelaine and Wheldall
(2016, Dyslexia 22(3):233-244) found low-progress readers did read 7 percent
more words per minute in Dyslexie than in standard Arial, then took plain
Arial and matched it to Dyslexie's spacing, and the advantage vanished.
Whatever benefit these fonts deliver is spacing, and spacing is a CSS property
and an egui parameter, not a font file. So the correct product is a single
cohesive, legible, well-spaced face used everywhere, plus a text-spacing
control that a dyslexic reader, an older reader, or anyone on a bad screen can
turn up. That also fits Wallace et al. (2022, ACM TOCHI 29(4)), who found
reading speed varied 35 percent between an individual's fastest and slowest
font with no comprehension cost, that different fonts suit different people,
and, crucially, that readers' stated preferences do not predict which font
they actually read fastest in. There is no one best font, so build the knob,
not the mode.

### What the letterform evidence does and does not support

Split this into two claims that are usually conflated, because the evidence
treats them very differently. Claim one, that disambiguated letterforms make
text more legible at threshold, is supported. Beier and Larson (2010,
Information Design Journal 18(2):118-137) built font variants that altered
only the frequently misrecognised letters and tested them at distance and
short exposure, avoiding the confound of comparing letters across different
typefaces, and found real differences: a single-storey a is misread as o or q,
and a double-storey a performs significantly better. Beier and Oderkerk (2022,
Applied Ergonomics 101:103709, "Closed letter counters impair recognition")
showed participants had more trouble identifying a, c, e, r, s, t and f when
set with closed apertures. Oderkerk and Beier (2022, Ergonomics 65:753-761)
found wider letter shapes improve recognition in parafovea and periphery, and
the Typotheque and Beier reading-acuity experiment of 2023 (55 participants
across blurred vision, central vision loss, peripheral vision loss and normal
vision) found a strong letter-width effect in every group. So open apertures,
generous counters, a double-storey a, and wider letters are evidence-backed
selection criteria, and they are exactly what disambiguates b from d, I from l
from 1, O from 0 and rn from m. Claim two, that disambiguated letterforms help
dyslexic readers read faster or more accurately, is NOT supported and should
not be asserted. Galliussi et al. (2020) tested a letterform built
specifically to break b-d-p-q rotational symmetry, with mixed serif and
sans-serif letters and longer ascenders and descenders, and reported that the
data "failed to show any effect from the letterform" in either dyslexic or
typical children. Worse for the mirror-letter theory, Marinus et al. (2016),
as summarised in Kuster et al. (2018), found that Dyslexie's letters are
actually LESS distinct from one another than Arial's, and Kuster et al. also
demolished the designer's x-height claim, measuring Dyslexie's x-height at 63
percent of cap height against Arial's 73 percent. Three further legibility
findings are directly actionable here. First, weight: Burmistrov, Zlokazova,
Ishmuratova and Semenova (2016, NordiCHI) found light and ultra-light fonts
less legible than regular and bold across two contrast levels and both
polarities, and their oculomotor measures, mean fixation duration and saccade
amplitude, indicated higher cognitive load; their recommendation is to avoid
light and ultra-light for body text. src/gui/fonts.rs:29 takes
egui::FontDefinitions::default(), whose proportional face is Ubuntu-LIGHT, so
the app's entire UI is currently set in exactly the weight class that study
advises against. Second, style: Rello and Baeza-Yates (2013, ASSETS, 48 adults
and children with clinically diagnosed dyslexia, 12 fonts, eye-tracked;
extended to 97 participants in 2016, ACM TACCESS 8(4) article 15) found
sans-serif, roman and monospaced significantly improved performance over
serif, proportional and italic; Arial Italic had both the longest reading time
and the longest fixation duration and should be avoided; Courier gave the
shortest fixations and Verdana and Helvetica were the most preferred. Notably
OpenDyslexic sat mid-table on fixation duration and Verdana was significantly
preferred over it (p = 0.002). Third, and consistent with Wallace et al.
(2022), preference does not track performance: Rello found a Pearson
correlation of just -0.13 between preference rating and reading time, and
Kuster found font preference unrelated to reading performance. Do not let a
preference poll decide the face.

### Risks and numeric targets

1. The repo's existing dyslexia mode is under-dosed to the point of being
cosmetic. web/shared/theme.css:226-227 sets letter-spacing 0.03em and
word-spacing 0.08em. Every evidence-based figure is two to six times larger:
WCAG 1.4.12 requires content to survive letter-spacing 0.12em and word-spacing
0.16em; Zorzi et al. (2012) added 2.5 points at 14 point, which is 0.179em;
the British Dyslexia Association Style Guide 2023 asks for tracking "around 35
percent of the average letter width", roughly 0.17em for a typical sans, with
word spacing "at least 3.5 times the inter-letter spacing". Even Galliussi et
al.'s increase of +70/1000 em letter and +270/1000 em word, which produced NO
benefit, was more than twice the repo's dose. If we keep a dyslexia mode at
these values we are shipping a placebo. 2. Raising letter spacing without
raising word spacing proportionally actively harms reading. This is the single
most important design constraint and it is easy to get wrong. Galliussi et al.
(2020) found that the increased-letter, default-word condition significantly
slowed BOTH dyslexic and typical readers, because words stop segmenting when
the two gaps look alike. Any spacing control must move word spacing with
letter spacing, at the BDA's 3.5 to 1 or better. 3. The spacing benefit itself
is real but contested and smaller than headlines suggest. Zorzi et al.'s 20
percent speed gain and halved errors in 74 children aged 8 to 14 is the
strongest result, but Skottun and Skoyles (2012, PNAS) argued in a published
letter that the claimed dyslexia-specificity rested on a null in an
underpowered control group of 30, and that the dyslexic advantage might
reflect being poor readers rather than a dyslexia-specific crowding deficit.
Stagg and Kiss (2021, Research in Developmental Disabilities, 59 children aged
11 to 15) found extra spacing helped everyone, 13 percent for dyslexic
children and 5 percent for controls, which supports the argument for a
universal control rather than a dyslexia mode. Kuster et al. and Duranovic et
al. did not replicate a spacing benefit at smaller doses. Honest framing: this
is a real but modest, dose-dependent effect that helps all readers somewhat
and struggling readers more. 4. Changing the typeface breaks two existing test
gates. tests/icon_glyph_lint.rs encodes a BROKEN_GLYPHS list that is a
property of the CURRENT font's coverage (Math Operators U+2200-22FF, Dingbats
U+2700-27BF, U+FE0F, and U+2190 and U+2194 painted as shapes instead). A new
face changes which glyphs tofu, so that list must be re-derived from
screenshots, not assumed. tests/theme_editor_coverage.rs will fail the moment
a font-family token is added to src/gui/theme.rs without a matching editor row
in src/gui/pages/settings.rs. 5. There is no font-family token today.
data/gui/theme.ron carries only sizes (font_size_small 11.902083,
font_size_body 16.497604, font_size_heading 24.291668, font_size_title
32.455627 at data/gui/theme.ron:28-31) and no family, so
web/shared/theme.css:144 and src/gui/fonts.rs:29 diverge by construction and
cannot be reconciled without adding family and spacing tokens and regenerating
via scripts/gen-theme-css.js. 6. Comic Sans carries a social cost the evidence
does not require us to pay. The BDA 2023 guide does name it, but Rello and
Baeza-Yates (2013) pointed out that the BDA "does not disclose on the basis of
which evidence these recommendations are made". Dyslexia Scotland's current
guidance is blunter and more defensible: there is "no one-size-fits-all
'dyslexia-friendly' font", and "evidence does not show a consistent reading
advantage for specialised 'dyslexia fonts'". Comic Sans is not required by the
evidence; adequate spacing is. 7. Cite Dyslexia Scotland's numbers with care.
Their page says to increase letter and word spacing by "30% to 35% of the font
size" and in the same breath says to "increase tracking by +0.03em to
+0.035em". Those two statements differ by a factor of ten and cannot both be
right. Use the BDA PDF and WCAG values as the numeric anchors instead. 8. Do
not overclaim to users. If we add a legibility control, describe it as
adjustable spacing and size that many readers find easier, not as a dyslexia
treatment. The literature is unanimous that these are presentation aids, and
both Wery and Diliberto and Kuster et al. close by warning against diverting
attention from evidence-based reading instruction. 9. Web and native will
drift again unless the spacing control is a token. The dyslexia toggle today
is web-only (web/pages/settings.html:1365 into web/shared/shell.js:218 into
web/shared/theme.css:223-227) with nothing equivalent in the app, which is
precisely the dual-UI drift CLAUDE.md forbids. 10. One genuine piece of
counter-evidence exists and should be reported rather than buried: Franzen,
Stark and Johnson's eye-tracking study reported that OpenDyslexic improved
reading comprehension in both dyslexic and non-dyslexic adults, with larger
gains for dyslexics. But reading speed was unaffected, the eye-movement
results were mixed, and the size and spacing confound was not controlled.
Against a meta-analytic g of -0.04 across 15 studies it does not change the
recommendation. Concrete numeric targets, if useful downstream: body text at
16 to 19px (BDA: 12-14 point, 1-1.2em); line-height at least 1.5 (BDA, WCAG
1.4.8 and 1.4.12) with the current web value of 1.6 at web/shared/theme.css:74
and :147 already compliant; paragraph spacing at least 2x font size (WCAG
1.4.12); line length at most 80 characters, 40 for CJK (WCAG 1.4.8), with BDA
calling 60 to 70 optimal; left-aligned, never justified (WCAG 1.4.8 and BDA),
which the repo already satisfies since a grep for text-align: justify across
web/ and src/ returns nothing; avoid italics and underlining for emphasis, use
bold (BDA), which is also what Rello's Arial Italic result supports; avoid
all-caps for continuous text (BDA); headings at least 20 percent larger than
body (BDA); a spacing control whose maximum reaches letter-spacing 0.12em with
word-spacing moved to at least 3.5 times the letter increment; and dark text
on a light but not pure white ground, with cream or a soft pastel offered,
since the BDA notes white "can appear too dazzling".

### Sources

Fetched and read in full or in substantial part:
https://pmc.ncbi.nlm.nih.gov/articles/PMC5629233/ (Wery and Diliberto, OpenDyslexic, Annals of Dyslexia)
https://pmc.ncbi.nlm.nih.gov/articles/PMC5934461/ (Kuster, van Weerdenburg, Gompel and Bosman 2018, Dyslexie font does not benefit reading)
https://cdn.bdadyslexia.org.uk/uploads/documents/Advice/style-guide/BDA-Style-Guide-2023.pdf (British Dyslexia Association Dyslexia Style Guide 2023, text extracted directly from the PDF streams)
https://lbhfinspirehub.com/wp-content/uploads/2024/05/BDA-Style-Guide-2023.pdf (same guide, mirror actually parsed)
https://www.changedyslexia.org/publications/pdfs/2013-ASSETS-Good%20Fonts%20for%20Dyslexia.pdf (Rello and Baeza-Yates 2013, ASSETS, full method, results, preference tables and discussion)
https://www.superarladislexia.org/pdf/2016-Luz%20Rello-Fonts-taccess.pdf (Rello and Baeza-Yates 2016, ACM TACCESS 8(4) article 15, abstract and framing)
https://dsv.units.it/sites/dsv.units.it/files/ric_grpr/ManuscriptANDY2020.pdf (Galliussi, Perondi, Chia, Gerbino and Bernardis 2020, full methods with exact spacing values, results and discussion)
https://thereadabilityconsortium.org/wp-content/uploads/2023/07/Readability__TOCHI-1.pdf (Wallace et al. 2022, ACM TOCHI, abstract, related work, discussion and takeaways)
https://pmc.ncbi.nlm.nih.gov/articles/PMC3497831/ (Skottun and Skoyles 2012, PNAS letter, Interletter spacing and dyslexia)
https://www.w3.org/WAI/WCAG22/Understanding/text-spacing.html (WCAG 2.2 SC 1.4.12 Text Spacing)
https://www.w3.org/WAI/WCAG22/Understanding/visual-presentation.html (WCAG 2.2 SC 1.4.8 Visual Presentation)
https://dyslexiascotland.org.uk/dyslexia-friendly-typed-formats/ (Dyslexia Scotland guidance, quoted verbatim)
https://www.lifescience.net/publications/2130167/does-font-improve-reading-in-dyslexic-children-met/ (Azzarello, Paek, Hodge, Jameson and Lewis 2026 meta-analysis, Annals of Dyslexia, abstract mirror)
https://www.sciencedaily.com/releases/2021/09/210929212202.htm (Stagg and Kiss 2021, Research in Developmental Disabilities)
https://aes.amegroups.org/article/view/5209/html (Franzen, Stark and Johnson, OpenDyslexic conference abstract, Annals of Eye Science)
https://www.typotheque.com/research/designing-fonts-with-low-vision-readers-in-mind (Typotheque and Beier reading-acuity experiment, 2023)
https://github.com/antijingoist/opendyslexic (OpenDyslexic licence and repository status)

Located and relied on via search result summaries rather than full-text fetch, so treat exact wording as second-hand:
https://link.springer.com/article/10.1007/s11881-026-00389-8 (the meta-analysis of record, DOI 10.1007/s11881-026-00389-8, PMID 42536336; Springer and PubMed both blocked direct fetch)
https://onlinelibrary.wiley.com/doi/abs/10.1002/dys.1527 (Marinus et al. 2016, Dyslexia 22(3):233-244; Wiley returned 403, the spacing-matched Arial result is corroborated inside the Kuster and Galliussi papers I did read)
https://onlinelibrary.wiley.com/doi/full/10.1002/dys.1727 (Joseph and Powell 2022, Dyslexia; Wiley 403 and PMC reCAPTCHA)
https://www.pnas.org/doi/10.1073/pnas.1205566109 (Zorzi et al. 2012, PNAS 109(28):11455-11459; PNAS returned 403, the 2.5 point manipulation at 14 point Times-Roman, 74 children aged 8-14, 20 percent faster and half the errors come from search summaries and the Skottun letter)
https://eric.ed.gov/?id=EJ1195564 (Duranovic, Senka and Babic-Gavric 2018, Annals of Dyslexia 68:218-228)
https://dl.acm.org/doi/10.1145/2971485.2996745 (Burmistrov, Zlokazova, Ishmuratova and Semenova 2016, NordiCHI, light and ultra-light fonts)
https://www.jbe-platform.com/content/journals/10.1075/idj.18.2.03bei (Beier and Larson 2010, Information Design Journal 18(2):118-137; abstract not served)
https://www.sciencedirect.com/science/article/abs/pii/S0003687022000321 (Beier and Oderkerk 2022, Closed letter counters impair recognition, Applied Ergonomics 101:103709)
https://dl.acm.org/doi/abs/10.1145/2858036.2858204 (Rello, Pielot and Marcos 2016, CHI, Make It Big)
https://www.dyslexiefont.com/en/pricing-publisher/ (Dyslexie commercial licensing)

Repo facts cited above were verified locally by grep and sed: src/gui/fonts.rs:10-11, 29, 48, 62-63; web/shared/theme.css:74, 144, 147, 223-227; web/shared/shell.js:218; web/pages/settings.html:1365; data/gui/theme.ron:28-31; and a grep for text-align: justify across web/ and src/ returning no matches.

## Bugs this evaluation found in passing

These are independent of which typeface wins and are worth fixing regardless.

### 1. The right arrow is broken on Linux and macOS, at 111 sites

U+2192 appears 111 times across src/gui. It is absent from Ubuntu-Light (1194
codepoints) and from NotoEmoji-Regular (887). Hack-Regular does carry it, but
epaint puts Hack in Monospace only, never in Proportional
(epaint-0.31.1/src/text/fonts.rs:358-365). So on Windows it renders solely
from Segoe UI Emoji, and on Linux and macOS it is blank or tofu. Adding Hack
to the Proportional chain fixes those 111 arrows plus U+2190, U+2194 and
U+2500 for zero bytes and one line.

### 2. The snapshot rig renders a different font stack than the app, and the glyph lint is calibrated against the rig

src/gui/ui_snapshots.rs builds bare egui::Context::default() instances and
installs no fonts, while the running app installs the emoji fallback at
src/lib.rs:1420. So the 38-page snapshot suite has never rendered what a user
sees. tests/icon_glyph_lint.rs BROKEN_GLYPHS is derived from that environment:
its U+2190 entry reflects a codepoint genuinely absent from the snapshot
stack, and its comment at :49-51 records that U+2192 DOES work, which is a
Windows-only observation written into the lint as a cross-platform fact. This
is a check calibrated against an environment no user has. Fix it before
trusting any font diff.

### 3. Japanese and Chinese are tofu in the native app today

Not one of the four bundled faces contains a single CJK codepoint. data/i18n/
ships ja.json and zh.json, and the native GUI cannot render either. The web is
fine because the browser falls back. The fix is a system-font probe in the
same shape src/gui/fonts.rs:53-69 already uses for emoji, costing zero
redistributed bytes. Bundling CJK is not viable: static Noto Sans CJK is about
16.5 MB per weight per language, and the 8 MB variable file is useless because
epaint exposes no way to select a variation axis.

### 4. set_fonts is never called at all when no system emoji font is found

src/gui/fonts.rs:41-43 calls ctx.set_fonts inside the candidate loop and
returns; :46 falls through to a warn with no set_fonts at all. On a machine
with no emoji font at a hardcoded path, any newly installed face would
silently fail to install with it.

### 5. A font placed under assets/ would not ship

.github/workflows/build-desktop.yml:91 copies data/ wholesale, while :92-93
copy only assets/icons/ and assets/shaders/. Fonts and their OFL text must go
under data/, or the failure appears only in the released archive and never in
a dev run.

### 6. Colour emoji in the native app is structurally impossible

epaint 0.31.1 rasterises through a single outline_glyph call
(epaint-0.31.1/src/text/font.rs:275) into a one-channel coverage atlas. Not a
font choice, an engine constraint. Emoji in the app are line art tinted with
the text colour while the website shows the OS colour set. Plan around it
rather than for it.

## The four proposed stacks

### 1. Atkinson Everywhere

- **UI face:** Atkinson Hyperlegible Next (Braille Institute of America with Applied
  Design Works, googlefonts/atkinson-hyperlegible-next, OFL-1.1). Native
  ships two static instances, AtkinsonHyperlegibleNext-Regular.ttf (65,068
  bytes) and AtkinsonHyperlegibleNext-Bold.ttf (66,792 bytes), both fetched
  and byte-counted today. Web self-hosts the single variable file
  AtkinsonHyperlegibleNext[wght].woff2 (48,188 bytes, weights 200 to 800)
  plus AtkinsonHyperlegibleNext-Italic[wght].woff2 (52,692 bytes) only on
  pages that use italic. Statics on native because egui cannot select a
  variable axis: epaint 0.31.1 FontData exposes only font, index and tweak,
  and no set_variation call exists anywhere in epaint or egui, so a variable
  file would render at its default instance forever. I parsed the shipped
  binary myself: 362 codepoints, and contrary to one research thread it DOES
  contain U+03BC (Greek mu, used at src/gui/pages/cosmos.rs:1201-1202),
  U+00DE (the reaction pill at src/gui/pages/chat.rs:3832), U+221E, U+00B7,
  U+2026, U+00B0 and U+00B2. It does NOT contain U+00B5 (micro sign), U+2500
  (box drawing), any arrow, or any check mark, which the fallback chain
  covers.
- **Mono:** Atkinson Hyperlegible Mono (googlefonts/atkinson-hyperlegible-next-mono,
  OFL-1.1). Native ships AtkinsonHyperlegibleMono-Regular.ttf (52,020 bytes,
  verified); web self-hosts AtkinsonHyperlegibleMono[wght].woff2 (25,600
  bytes, verified). Not JetBrains Mono, and the reason is accessibility
  rather than typography: Atkinson Mono is the same superfamily as the UI
  face, so x-height, cap height and letterform logic match by construction,
  and a reader who has just tuned the text size does not get a second,
  differently-scaled face for keys, hashes, code fences and the relay
  console. Its 359-codepoint repertoire is a non-issue because Hack stays in
  the monospace chain behind it and I verified Hack carries U+2190, U+2192,
  U+2194, U+2500, U+25B8, U+25BE, U+21A9, U+221E, U+00B5, U+03BC, U+0414 and
  U+03B1. Hack does NOT carry U+2713, which is why the symbols face below
  exists. Existing web rules at web/pages/files.html:111,120,144,189 and
  web/pages/accord.html:64 already name fonts the repo does not ship and
  silently fall through to generic monospace; they get rewritten to
  var(--font-mono), which also fixes web/pages/roadmap.html:106 where
  var(--font-mono, monospace) references a variable defined nowhere.
- **CJK:** Ship zero CJK bytes on both surfaces and fix the existing tofu bug with a
  system-font probe, which is the same pattern src/gui/fonts.rs:53-69
  already uses for emoji and costs nothing to redistribute. Today native
  renders Japanese and Chinese as tofu: I parsed all four bundled faces and
  none contains U+4E00 or U+3042, while src/embedded_data.rs compiles
  ja.json and zh.json into the binary. Extend platform_emoji_paths into a
  general platform_fallback_paths that also probes, per OS, the machine's
  CJK face (Windows YuGothR.ttc, msgothic.ttc, simsun.ttc; macOS Hiragino
  and PingFang; Linux the distro Noto CJK paths), registers it as
  "system_cjk" and appends it to the tail of both families. On web the
  existing generic sans-serif tail already does this, so the CSS stack
  becomes 'Atkinson Hyperlegible Next', then the current system stack, then
  the emoji names. Bundling CJK is refused deliberately: a single pan-CJK
  weight is 15.70 MB and JP plus SC subsets are 12.26 MB, against 84 and 76
  codepoints in two 1 KB i18n stubs that the native GUI never reads (no
  translation lookup exists in src/gui/). Revisit only when native
  localisation actually ships, and then as an optional downloaded language
  pack loaded from data/ at runtime, never include_bytes.
- **Emoji:** Two honest, different answers, because egui structurally cannot do what a
  browser does. Native: colour emoji is impossible. epaint 0.31.1 rasterises
  through outline_glyph only (epaint-0.31.1/src/text/font.rs:275) into a
  single-channel coverage atlas (src/image.rs:274-282), and never calls
  ab_glyph's glyph_raster_image2. So Segoe UI Emoji renders as flat
  monochrome COLR base outlines on Windows while Noto Color Emoji on Linux
  is CBDT-only and returns no outline at all, producing invisible
  advance-width gaps rather than even a visible tofu box. Fix that by making
  the bundled monochrome NotoEmoji-Regular the primary emoji face, placed
  AHEAD of the system probe, so every platform gets the identical line-art
  emoji instead of Windows-only shapes. Add Noto Sans Symbols 2 Regular
  (notofonts/symbols via google/fonts, OFL-1.1, 1,233,128 bytes, 2,955
  codepoints, all verified by parsing the file) to close the seven
  codepoints no bundled face covers: I confirmed it carries U+2713, U+2715,
  U+26C8, U+1F327, U+1F32B, U+1F32A and U+1F5D1, all of which the UI uses
  today and which currently render on the operator's Windows machine only.
  The one straggler is U+263E in the weather HUD
  (src/gui/pages/hud.rs:208-216), absent from every candidate; change it to
  U+1F319, which I verified IS in the bundled NotoEmoji-Regular, for zero
  bytes. Web: keep OS colour emoji through the existing emoji names in the
  CSS stack. State the divergence plainly rather than pretending parity.
- **Reading comfort:** Delete the font swap, keep and properly dose the spacing, and give native
  the control it has never had. The evidence is not close: the 2026 Annals
  of Dyslexia meta-analysis pooled 15 studies, 91 effect sizes, N = 688 and
  found g = -0.04, 95% CI [-0.15, 0.07]; Kuster et al. 2018 found no
  Dyslexie benefit in 170 children and measured its x-height at 63 percent
  of cap height against Arial's 73; Wery and Diliberto 2017 found
  OpenDyslexic worse than Arial and Times, with nobody preferring it;
  Galliussi et al. 2020 isolated letterform from spacing and the letterform
  contributed nothing; and Marinus et al. 2016 matched plain Arial to
  Dyslexie's spacing and the entire advantage vanished. Meanwhile
  web/shared/theme.css:225-227 forces "Comic Sans MS", "Trebuchet MS",
  Verdana, Tahoma, none of which have a fontconfig metric alias in the
  shipped 30-metric-aliases.conf and none of which Android ships, so on two
  of the four platforms the operator named the toggle changes only the
  spacing already. So: rename it Reading Comfort, drop the family override
  entirely (switching away from a face built by an accessibility institute
  TO Comic Sans is a regression), and expose three real tokens that both
  clients honour. Letter spacing 0 to 0.12em (WCAG 1.4.12's tested ceiling;
  the current 0.03em is a quarter of the studied dose and Zorzi et al. 2012
  used roughly 0.18em). Line height 1.2 to 2.0, default 1.5. Text size scale
  1.0 to 1.5, reaching 18 to 19 px body, which is where Rello, Pielot and
  Marcos 2016 found readability improving continuously and which Chatrangsan
  and Petrie replicated in Thai. Native can do all three: I confirmed epaint
  TextFormat carries extra_letter_spacing at text_layout_types.rs:259 and
  line_height at :268. There is no word-spacing field, and Galliussi showed
  letter spacing raised WITHOUT word spacing slows everyone down, so I
  checked the layout code: text_layout.rs:157-169 loops over every char with
  no special case for the space and adds extra_letter_spacing before each
  glyph, so a word gap receives two increments while a letter gap receives
  one. At 0.12em against Atkinson's roughly 0.25em space that is a 0.49em
  word gap to a 0.12em letter gap, about 4.1 to 1, comfortably past the
  British Dyslexia Association's 3.5 to 1 and structurally not Galliussi's
  harmful condition. Web adds word-spacing: 0.16em on top and lands higher
  still. Finally, adopt bold rather than colour for emphasis: egui's
  .strong() only brightens the text colour, which is invisible to a
  low-contrast or colourblind reader, so registering
  AtkinsonHyperlegibleNext-Bold as a named family gives emphasis a
  contrast-independent cue, which is what the BDA guide asks for. Say in the
  settings copy that this is adjustable spacing and size that many readers
  find easier, never that it treats dyslexia.
- **Shipped bytes:** Desktop archive: +1,417,008 bytes compiled into the exe (Atkinson Next
  Regular 65,068 + Bold 66,792 + Atkinson Mono Regular 52,020 + Noto Sans
  Symbols 2 Regular 1,233,128, every figure a real byte count from the
  file), plus roughly 30 KB of licence text placed under data/ so it
  actually travels. That is about +2.1 percent on a 68.3 MB exe and about
  +0.85 percent on the 166.7 MB release archive. The four epaint default
  faces (1,407,752 bytes: Ubuntu-Light 361,676, Hack 309,408, NotoEmoji
  418,804, emoji-icon-font 317,864) STAY, because they are the fallback tail
  that supplies arrows, box drawing, Greek, Cyrillic and emoji, so this is a
  genuine addition and not a swap. Do NOT set default-features = false on
  egui to reclaim them; FontDefinitions::default() then returns empty and
  src/gui/fonts.rs:29 would build a definition containing nothing but the
  runtime fallbacks. Web page load: 48,188 bytes for the UI face covering
  all seven weights, 73,788 bytes on pages that also use monospace, 126,480
  bytes worst case if italic is needed, all same-origin, immutable-cached
  and service-worker precached. Zero third-party requests:
  scripts/nginx/humanity.conf:53 already sets font-src 'self', so a CDN is
  blocked by the site's own policy and self-hosting is the only option that
  does not require weakening a header the privacy arc deliberately
  tightened. That also matters legally, since a Munich court awarded damages
  purely for transmitting a visitor IP to Google via hotlinked fonts, and
  Chrome has partitioned its HTTP cache since v86 so the old shared-cache
  argument for a CDN is dead anyway.
- **Gives up:** Colour emoji in the native app, permanently. epaint rasterises a single
  coverage channel and calls outline_glyph only, so every emoji in the app
  is line art tinted with the text colour while the website shows the OS
  colour set. egui PR #5784 (Parley) is an open draft whose author says
  Parley's layout model conflicts with egui's, so do not plan around it
  landing. Pixel-identical rendering between the two surfaces, which is not
  achievable at all and should never be promised: epaint rounds the raster
  size to whole pixels, rasterises each glyph once at origin, and snaps
  every glyph x position to a whole device pixel, while DirectWrite
  positions to a sixteenth of a pixel with subpixel RGB coverage, so the
  same string will not even measure the same width. Cohesion outside Latin.
  Atkinson is 362 codepoints, so Cyrillic, Greek beyond mu, arrows, box
  drawing, symbols and CJK all render from a different face in the chain. A
  Russian or Japanese speaker gets a coherent UI, not a coherent typeface,
  and the Latin-only cohesion claim must be stated that precisely. A
  guarantee that CJK renders at all: a machine with no system CJK font still
  shows tofu, and we are choosing that over 12 to 16 MB of bundled font for
  two 1 KB translation stubs the native GUI does not even read yet. About
  1.35 MB in the exe, dominated by Noto Sans Symbols 2 at 1.23 MB, bought to
  make seven glyphs (including the check mark and three weather icons)
  render on Linux and macOS instead of Windows only. Reject the cheaper
  alternative of drawing them all as shapes only if you are willing to
  accept that user-typed symbols in chat stay broken. Layout stability: 38
  snapshots change, every line box grows by roughly a quarter, the chat
  reaction popup geometry shifts because it measures the advance width of
  U+00DE at src/gui/pages/chat.rs:7817, and the four hand-tuned size tokens
  are being deliberately raised on top of that. And upstream maintenance:
  all three googlefonts Atkinson repositories are archived (last pushed
  2025-06-10), with open requests for Cyrillic, arrows and a missing Maori o
  that will not be actioned. The absence of a Reserved Font Name means we
  can patch and rebuild ourselves, but that becomes our job, not upstream's.
  Finally, honesty in the copy: this face was built for LOW VISION readers,
  not for dyslexia, and no controlled trial has measured a reading gain for
  Atkinson specifically. Write "designed for low vision readers, with
  letterform choices that also help many dyslexic readers" and never "proven
  to".

### 2. The Plex Stack: one superfamily, drawn shapes for icons, and a spacing knob instead of a dyslexia font

- **UI face:** IBM Plex Sans Regular (Mike Abbink with Bold Monday, OFL-1.1, from the
  IBM/plex repo, pinned to the @ibm/plex-sans 1.1.0 tag; I measured the
  master build at 200,500 bytes for IBMPlexSans-Regular.ttf, re-fetch from
  the tag before vendoring). One weight only on native, because egui
  resolves everything through FontFamily::Proportional and
  RichText::strong() changes colour rather than weight. Web additionally
  gets SemiBold so the browser does not synthesise a fake bold. Behind it in
  the egui fallback chain, in order: Hack, Ubuntu-Light, NotoEmoji-Regular,
  emoji-icon-font, then the system CJK font, then the system emoji font.
  Nothing is removed.
- **Mono:** IBM Plex Mono Regular (same superfamily, same OFL-1.1,
  IBMPlexMono-Regular.ttf, 173,052 bytes measured), with Hack retained
  directly behind it. I parsed both faces on disk: Plex Sans and Plex Mono
  share x-height 0.516 em and cap-height 0.698 em EXACTLY, both at 1000
  upem. That is the only exact sans-mono metric pairing in the entire
  candidate field. Source Sans 3 with Source Code Pro is the other
  superfamily and its x-height is 0.478 to 0.486 em, the smallest of every
  face measured, which is precisely wrong at the repo's 11.9 px
  font_size_small (data/gui/theme.ron:28). Plex Mono's zero is marked and
  Plex Sans's is not, which is the correct division of labour: I parsed the
  contours and Plex Mono's `0` has three, the third a 124 by 118 unit dot
  centred at x 238..362, y 290..408 inside a counter running x 143..457, y
  61..637, while its `O` has two. So 0 and O are unambiguous exactly where
  keys, hashes, code fences and the relay console live.
- **CJK:** Borrow the OS font, ship zero bytes, and treat this as fixing a live bug
  rather than adding a feature. I parsed the cmap of every face currently
  compiled into the exe and none of the four contains U+4E00: Ubuntu-Light
  1194 codepoints, Hack-Regular 1548, NotoEmoji-Regular 887, emoji-icon-font
  654, and no CJK or kana in any of them. Plex Sans does not have it either
  (891 codepoints). Meanwhile src/embedded_data.rs:116-117 compiles ja.json
  and zh.json into the binary and serves them at :264-265, so Japanese and
  Chinese UI strings ship today and are guaranteed tofu. The fix is to
  extend the existing platform_emoji_paths() at src/gui/fonts.rs:53 into a
  platform_fallback_paths() that also probes system CJK faces (Windows
  %WINDIR%\Fonts\YuGothR.ttc, msgothic.ttc, simsun.ttc, malgun.ttf; macOS
  /System/Library/Fonts/PingFang.ttc and Hiragino; Linux
  /usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc), appended AFTER
  the Latin faces so they never win for Latin. Cost: 0 MB shipped, 0 licence
  obligation, and it fixes real user-typed Japanese in chat, which is
  unbounded text no subset could ever cover. Bundling is the wrong answer at
  these prices: the honest measured figures are 4.53 MB for
  NotoSansJP-Regular.otf and 8.33 MB for NotoSansSC-Regular.otf as region
  subsets, 15.70 MB for one pan-CJK weight, and egui cannot select a
  variable axis so a second weight costs a second full file. On web, defer
  entirely until data/i18n stops being 37-string stubs, then self-host IBM
  Plex Sans JP and SC (the same studio drew them, which is the whole
  cohesion payoff) as unicode-range-sliced woff2 so Latin visitors download
  nothing. IBM ships SC and TC as woff2 only, which is exactly the format
  web needs.
- **Emoji:** Ship no emoji font at all, ever. egui structurally cannot render colour
  emoji: epaint 0.31.1 rasterises through a single outline_glyph call at
  epaint-0.31.1/src/text/font.rs:275 into a one-channel coverage atlas
  (epaint-0.31.1/src/image.rs:274), and ab_glyph's raster path
  (glyph_raster_image2) is never called. So a 10.7 MB CBDT Noto Color Emoji
  would render as literally nothing, and a 25.3 MB one would render as
  nothing more expensively. Keep the existing system-font borrow for
  user-typed chat content, where a monochrome outline from Segoe UI Emoji is
  better than a box. For UI chrome, finish the drawn-shape migration that
  src/gui/widgets/icons.rs already started: it holds 43 paint_* helpers,
  each added because a glyph failed. My cmap parse shows why that is the
  only cross-platform answer. Of the symbols Plex Sans lacks, Hack supplies
  U+2500 box drawing, U+25B8 and U+25BE triangles, but NOTHING in the
  bundled Latin set supplies U+26A0 warning, U+2139 info, U+2699 gear,
  U+2600 sun, U+263E moon, U+2744 snow, U+2B50 star, U+2764 heart or U+2705.
  Those come only from the runtime system emoji font, which means the
  weather HUD at src/gui/pages/hud.rs:194 and :208-216 is blank or tofu on
  Linux today. That is a live cross-platform bug independent of any typeface
  decision, and the fix is paint_* helpers, which take a Color32 and
  therefore consume theme tokens, which glyphs never could.
- **Reading comfort:** Delete the family override, keep and correct the spacing, and give native
  the mode it has never had. The toggle becomes "Reading comfort", not
  "Dyslexia-Friendly Font", and it moves four things: text size, line
  height, letter spacing and word spacing. It changes no typeface. Three
  reasons, in order of weight. First, the evidence: the 2026 Annals of
  Dyslexia meta-analysis pooled 15 studies, 91 effect sizes, N = 688 and
  found a Hedges g of -0.04, 95% CI [-0.15, 0.07], indistinguishable from
  zero. Wery and Diliberto found OpenDyslexic worse than Arial and Times,
  with nobody preferring it. Marinus et al. found low-progress readers 7
  percent faster in Dyslexie, then matched plain Arial to Dyslexie's spacing
  and the advantage vanished. The lever is spacing, and spacing is a token,
  not a font file. Second, the current implementation does nothing on two of
  the four platforms the operator named: fontconfig's shipped
  30-metric-aliases.conf carries entries for Arial, Times New Roman, Courier
  New, Georgia, Cambria, Calibri and Symbol and has NO entry for Comic Sans
  MS, Trebuchet MS, Verdana or Tahoma, and Android ships Roboto plus Noto,
  so on Linux and Android web/shared/theme.css:225 changes only the letter
  and word spacing. Third, that spacing is four to six times under dose:
  0.03em and 0.08em against Zorzi's studied approximately 0.18em and the
  BDA's approximately 0.17em. One constraint is non-negotiable and easy to
  get wrong: Galliussi et al. 2020 found that raising letter spacing WITHOUT
  raising word spacing proportionally significantly slowed both dyslexic AND
  typical readers, because words stop segmenting. So the control must move
  word spacing with letter spacing at the BDA's 3.5-to-1 or better. Web
  defaults become letter-spacing 0.06em, word-spacing 0.21em, line-height
  1.7, font-size 1.125em, with the slider maximum reaching 0.12em letter and
  0.42em word. Native gets size, line-height and only a modest
  letter-spacing increment, because epaint's TextFormat exposes
  extra_letter_spacing and line_height but has no word-spacing field, and
  applying letter spacing without word spacing is the shape the evidence
  says is harmful. That asymmetry is real and should be stated in the code
  comment rather than papered over. Finally, do not claim a treatment in
  user copy: call it adjustable spacing and size that many readers find
  easier, and note that Plex was chosen for legibility, not marketed as a
  dyslexia font.
- **Shipped bytes:** Desktop: +373,552 bytes of outlines (IBMPlexSans-Regular.ttf 200,500 plus
  IBMPlexMono-Regular.ttf 173,052, both weighed on disk after download) plus
  roughly 10 KB of OFL.txt. Nothing is removed, because the epaint defaults
  stay as the fallback tail. Against a 68.3 MB exe that is +0.55 percent,
  and against the 166.7 MB linux-x64 release archive it is +0.22 percent.
  For scale, the exe already carries 1,407,752 bytes of font data nobody
  chose: Ubuntu-Light 361,676, Hack-Regular 309,408, NotoEmoji-Regular
  418,804, emoji-icon-font 317,864, all pulled in because Cargo.toml:67
  takes egui with default features. Web, self-hosted, complete charset, no
  subsetting: IBMPlexSans-Regular.woff2 63,020 bytes,
  IBMPlexSans-SemiBold.woff2 67,060, IBMPlexMono-Regular.woff2 49,248 (all
  three verified as genuine woff2 by their wOF2 magic bytes). A text-only
  page pays 63,020 bytes; a page using bold pays 130,080; the full set once
  cached across the site is 179,328. Zero third-party requests, which
  matters twice over: scripts/nginx/humanity.conf:53 already sets font-src
  'self', so a CDN link is blocked by the site's own policy today, and
  hotlinking would hand every visitor's IP to a third party immediately
  after an arc that removed the DM social graph, cut nginx logs to two days
  and shipped a Tor onion service. CJK adds 0 bytes on both surfaces under
  this plan.
- **Gives up:** 1. THE LINE BOX GROWS 13 PERCENT AND THIS IS THE REAL COST. I read the
  vertical metrics off both files. Ubuntu-Light has USE_TYPO_METRICS off and
  hhea 932/-189/28, so 1.149 em of default leading. Plex Sans has
  USE_TYPO_METRICS on with typo and hhea agreeing at 1025/-275/0, so 1.300
  em. Every row, button, tab and wrapped label gets 13 percent taller.
  Because x-height and set width match Ubuntu-Light almost exactly, the fix
  is NOT to retune the four font_size_* tokens at data/gui/theme.ron:28-31;
  it is to trim the spacing_* tokens at :32-36 if the app reads too airy.
  All 38 PNGs under tests/snapshots/ will change and the diff needs
  reviewing page by page, not accepting wholesale. 2. PLEX SANS'S
  PROPORTIONAL ZERO IS SLIGHTLY WORSE THAN WHAT WE HAVE. I measured it: two
  contours, no dot, no slash, ink 480 against the O's 592, a ratio of 0.811,
  where Ubuntu-Light's is 446 against 650, a ratio of 0.686. So 0 and O are
  proportionally CLOSER in Plex. Plex has a `zero` feature and ss03 for the
  slashed form and epaint can reach neither, since FontData carries only
  font, index and tweak and ab_glyph applies no GSUB. Mitigated three ways
  and I would not trade the rest of the stack for it: Plex Mono's zero is
  dotted, identifiers are rendered in mono, and the encodings this app
  actually shows do not contain the confusable set anyway (lowercase hex has
  no O or capital I, and base58 deliberately excludes 0, O, I and l). 3. b,
  d, p and q are NOT disambiguated. I measured them: Plex's b and d share an
  ink box of 445 by 752 and its p and q share 445 by 728, exact mirror
  twins, and Ubuntu-Light is the same. No mainstream body sans solves this,
  and the ones marketed as solving it do not deliver. This is precisely why
  the accessibility answer here is spacing rather than letterforms, and it
  should be said out loud rather than quietly hoped past. 4. Plex Mono has
  no Greek. I confirmed U+03BC is absent from its cmap. Greek in monospace
  falls through to Hack, which has it. Nothing in the repo needs it there
  today. 5. The reading-comfort mode is weaker on native than on web,
  permanently. epaint's TextFormat has extra_letter_spacing and line_height
  but no word-spacing field, and the evidence says letter spacing raised
  without word spacing actively slows readers, so native leads with size and
  line height and applies only a modest letter increment. Web gets the full
  dose. That asymmetry cannot be closed without upstream work. 6. Nobody
  gets an accessibility typeface. If the operator was picturing OpenDyslexic
  or Dyslexie, this plan does not deliver it, and the reason is a pooled
  effect of g = -0.04 across 15 studies and N = 688. It is the right call
  and it is still a disappointment worth naming up front rather than
  burying. 7. CJK on native rides on the user's machine. A box with no CJK
  font installed still shows tofu. Zero bytes bought a fix for the
  overwhelming majority, not for everyone, and there will be no build-time
  signal when it fails. 8. Colour emoji are gone from native forever, or at
  least until egui PR #5784 lands, which has been an open draft since March
  2025 and whose author says Parley's API conflicts with egui's model. Do
  not plan around it. 9. Plex is IBM's voice, and someone will say so. It is
  a corporate typeface with a strong identity and adopting it borrows a
  little of that identity. The counter is that it is OFL-1.1 with no
  restriction on unmodified redistribution, drawn by Bold Monday rather than
  assembled, and it is the only candidate here whose mono is a true sibling
  and whose CJK siblings exist at all. 10. App and web will still not be
  pixel-identical, and no font choice can fix that. epaint rounds the raster
  size to whole pixels (epaint-0.31.1/src/text/font.rs:118), rasterises each
  glyph once at the origin, and snaps every glyph x position to a device
  pixel (text_layout.rs:579), so there is no subpixel positioning at all,
  while DirectWrite positions to 1/16 pixel with ClearType. The same string
  will not even have the same measured width in the two surfaces. Promise
  the same typeface, never pixel parity, or a future snapshot-diff between
  them will fail forever.

### 3. Atkinson Prepend

- **UI face:** Atkinson Hyperlegible Next Regular, static TTF, 65,068 bytes, vendored
  from googlefonts/atkinson-hyperlegible-next at a pinned commit. Licence
  OFL-1.1, verified two ways: the GitHub API reports spdx_id OFL-1.1 for
  that repo, and the repo's OFL.txt opens "Copyright 2020-2024 The Atkinson
  Hyperlegible Next Project Authors" with NO Reserved Font Name declared, so
  a future subset or feature-freeze would not force a rename. The repo is
  archived (pushed 2025-06-10, archived true), which is a reason to vendor a
  pinned copy rather than track upstream, not a reason to avoid it. Ship
  Regular ONLY: the app has no bold face today (egui's default proportional
  chain is Ubuntu-Light alone, and RichText::strong() changes colour, not
  weight), so adding a weight would mean a second FontFamily and edits
  across the 100-plus FontId::proportional call sites. Web gets the variable
  woff2 instead, AtkinsonHyperlegibleNext[wght].woff2 at 48,188 bytes, one
  file covering weights 200 to 800, because browsers can select axes and
  egui cannot.
- **Mono:** Atkinson Hyperlegible Mono Regular, static TTF, 52,020 bytes, from
  googlefonts/atkinson-hyperlegible-next-mono, OFL-1.1, same
  no-Reserved-Font-Name copyright line. Web gets
  AtkinsonHyperlegibleMono[wght].woff2 at 25,600 bytes. It is chosen over
  keeping Hack for one measured reason: I parsed the OS/2 and hhea tables of
  all four faces and Atkinson Next and Atkinson Mono are metrically
  identical to each other (upem 1000, xHeight 496, capHeight 668, typo
  ascender 984, descender -316, USE_TYPO_METRICS set), whereas Hack sits at
  xHeight 0.547 em and capHeight 0.729 em against Atkinson's 0.496 and
  0.668. Pairing Atkinson Next with Hack would make monospace text render
  visibly larger than the UI text beside it at the same pixel size, which is
  the exact incoherence the operator is complaining about, just relocated.
  Hack is NOT deleted, it stays in the Monospace fallback chain behind
  Atkinson Mono, which is what keeps its 1,548-codepoint symbol coverage
  available.
- **CJK:** Ship zero CJK bytes on both surfaces. Native gets a fallback path instead
  of a font file: rename platform_emoji_paths() in src/gui/fonts.rs:53 to a
  general fallback list and append CJK candidates at the tail (Windows
  YuGothR.ttc / msgothic.ttc / simsun.ttc, macOS PingFang and Hiragino,
  Linux the Noto Sans CJK the locale already installs). Web keeps a system
  tail on the font-family list at web/shared/theme.css:144. This costs 0 MB,
  carries no licence obligation, and is the same read-do-not-redistribute
  pattern src/gui/fonts.rs:15-16 already uses for emoji. It is also a bug
  fix rather than a regression: none of the four faces currently compiled
  into the exe contains a single CJK codepoint, so ja and zh render as tofu
  in the native app today, and adding the CJK tail is the first time a
  Japanese user typing in chat sees their own text. The case for bundling
  does not survive the numbers. One pan-CJK weight is 15.70 MB
  (NotoSansCJKjp-Regular.otf) or 12.26 MB for JP plus SC region subsets, and
  since egui cannot select a variable axis, a second weight doubles it. That
  would be a 130-fold increase over this entire proposal to serve a demand
  that does not exist: data/i18n/ja.json and zh.json are 37-string stubs,
  and native reads no i18n at all (they are embedded at
  src/embedded_data.rs:113-117 and no GUI code loads them). On the web the
  honest deferral is easier still, because unicode-range slicing means a
  Latin page downloads none of it. Revisit when native localisation actually
  ships, and when it does, load CJK lazily from data/ rather than
  include_bytes!.
- **Emoji:** Unchanged, and deliberately so: ship zero emoji bytes and keep reading the
  OS font. The reason is structural, not a preference. epaint 0.31.1
  rasterises through one call, outline_glyph, and its atlas is a
  single-channel coverage image, so a colour emoji font renders as either
  flat monochrome outlines (COLR fonts like Segoe UI Emoji, which have real
  base glyf outlines) or as nothing at all (CBDT and sbix fonts like Noto
  Color Emoji and Apple Color Emoji, which have no outlines to draw).
  Shipping NotoColorEmoji would put between 10.7 MB and 25.3 MB into the
  archive to render literally nothing. Monochrome Noto Emoji at 1,982,596
  bytes would work but costs 17 times this whole proposal to replace a
  fallback that already functions on Windows. Two things must change in
  fonts.rs regardless: the current loop returns early at :43 on the first
  emoji font it finds and, if it finds none, warns at :46 and never calls
  ctx.set_fonts at all, so on a Linux box without an emoji font the new
  faces would silently not install. The rewrite must build FontDefinitions
  once and call set_fonts unconditionally. Long term the answer for UI
  chrome is already half built: src/gui/widgets/icons.rs holds 43 paint_*
  vector helpers including paint_arrow_left, paint_arrow_both and
  paint_arrow_right, added precisely because glyphs were unreliable. Finish
  that path for chrome and let the OS font serve user-typed content.
- **Reading comfort:** Stop swapping the family, raise the spacing, and rename the toggle.
  Concretely, at web/shared/theme.css:223-228 delete the font-family line
  and raise letter-spacing from 0.03em to 0.12em and word-spacing from
  0.08em to 0.16em, the WCAG 2.2 SC 1.4.12 anchors. The evidence is
  one-sided and I am not going to soften it: the 2026 Annals of Dyslexia
  meta-analysis pooled 15 studies, 91 effect sizes and N=688 and found a
  Hedges g of -0.04, indistinguishable from zero; Kuster et al. 2018 found
  no benefit from Dyslexie across 170 children and only 11.6 percent
  preferred it against 38.1 percent for Arial; Wery and Diliberto 2017 found
  OpenDyslexic worse than Arial on both rate and accuracy with nobody
  preferring it. What does replicate is spacing, and Marinus et al. 2016
  nailed the mechanism by matching plain Arial to Dyslexie's spacing and
  watching the advantage vanish. So the family half of the current rule is
  the unsupported half, and it is also the broken half: fontconfig's
  30-metric-aliases.conf has no entry for Comic Sans MS, Trebuchet MS,
  Verdana or Tahoma, so on Linux and Android, two of the four platforms
  named in the ask, the toggle changes nothing but the two spacing values it
  already sets at one quarter of the studied dose. Keeping it would also be
  backwards once the base face is Atkinson: switching AWAY from a face built
  by the Braille Institute for low-vision readers, TO Comic Sans, actively
  harms the user it targets. There is one caveat worth honesty: raising
  letter-spacing without raising word-spacing proportionally slowed BOTH
  dyslexic and typical readers in Galliussi et al. 2020, because words stop
  segmenting, so the two values must move together at roughly 3.5 to 1 as
  the British Dyslexia Association 2023 guide asks. The native mirror is a
  second increment, not this one: egui exposes extra_letter_spacing only per
  TextFormat, with no global hook, so a native letter-spacing toggle means a
  call-site helper across roughly 108 FontId sites. What native DOES get in
  this increment is the base face plus the four already-editable size
  tokens, and the letter-spacing debt gets logged in
  docs/design/in-app-ops.md rather than silently accepted.
- **Shipped bytes:** Release archive: 117,088 bytes of TTF compiled into HumanityOS.exe via
  include_bytes!, plus roughly 9 KB of OFL text under data/. Call it about
  126 KB, and less than that after the zip and tar.gz at
  .github/workflows/build-desktop.yml:102-104 compress it. Against a
  v0.1299.0 linux zip of 166.7 MB that is under 0.08 percent, and it is
  genuinely invisible next to the roughly 147 MB data bundle. Web page load:
  73,788 bytes on a first visit for both variable faces covering every
  weight, self-hosted from web/fonts/, then served from cache and precached
  by the service worker. If the mono face is deferred on web, where only
  about five CSS rules name a concrete mono stack, it is 48,188 bytes. For
  scale, that is smaller than most single images on the site. Worth stating
  plainly because it inverts the usual expectation: the exe already carries
  1,407,752 bytes of font nobody chose (Ubuntu-Light 361,676, Hack-Regular
  309,408, NotoEmoji-Regular 418,804, emoji-icon-font 317,864, all arriving
  through egui's default_fonts feature at Cargo.toml:67). If you later
  flipped default-features = false and cut the fallback chain, this stack
  would make the binary about 1.29 MB SMALLER than it is today. I am not
  proposing that in this increment, because keeping the chain is the entire
  shippability argument, but it means the size ceiling on this decision is
  negative.
- **Gives up:** 1. Latin only, and the fallback chain stays visible. Atkinson Next maps
  362 codepoints and Atkinson Mono 359: no Greek, no Cyrillic, no arrows, no
  dingbats, no check mark, no CJK. So roughly 27 symbol codepoints the
  native UI uses, including the right arrow at 111 sites and the check at
  12, will still render from Ubuntu-Light, Hack or the OS emoji font, in a
  different design. Most are icon-sized and few readers will notice, but the
  honest statement is "one family for text, a fallback chain for symbols",
  not "one font". 2. Text gets about 4 percent smaller and rows about 13
  percent taller. Measured, not predicted: x-height 0.496 em against
  Ubuntu-Light's 0.517, and a 1.300 em line box against 1.149. Dense
  surfaces reflow. Web escapes this because line-height is pinned at 1.6, so
  the two clients will not be pixel-identical even before rasteriser
  differences. 3. Pixel parity is impossible and should never be promised.
  epaint rounds the raster size to whole pixels, rasterises each glyph once
  at the origin into a grayscale coverage atlas, and snaps every glyph x
  position to a device pixel, so egui has no subpixel positioning at all,
  while DirectWrite positions to 1/16 pixel with ClearType and FreeType
  hints on Linux and Android. The same string will not even have the same
  measured width in the app and the browser. What you buy is "recognisably
  the same face", and a snapshot diff between the two surfaces would fail
  forever. 4. Japanese and Chinese still will not match across operating
  systems. The system-fallback answer costs nothing and fixes today's tofu,
  but a Windows user sees Yu Gothic and a macOS user sees Hiragino. Full
  cohesion for ja and zh costs 12 to 16 MB per weight in the exe and is
  deferred, not solved. 5. Colour emoji remains impossible in the app. That
  is an egui 0.31 limitation and no font choice touches it. The upstream
  fix, egui PR 5784 for Parley, has been an open draft since March 2025 and
  should not be planned around. 6. No bold. Regular only, matching today's
  behaviour. Emphasis stays colour-and-size until someone does the
  second-family work across the FontId call sites. 7. Upstream is archived.
  Both repos were archived on 2025-06-10 with open requests for Cyrillic,
  arrows and a missing Maori macron. Vendoring a pinned copy is the right
  move anyway, but it means any future glyph gap is your job to patch, and
  the absence of a Reserved Font Name is what makes that legally possible.
  8. The mono face loses ground on x-height. Atkinson Mono at 0.496 em is
  meaningfully smaller than Hack's 0.547 at the 11.9 px font_size_small used
  for key rows and log transcripts. I chose it anyway because metric
  agreement with the UI face is the whole point and the mismatch would
  otherwise be permanent, but at that size it is a real trade and the
  snapshot review should look hard at src/gui/pages/relay_control.rs and
  src/gui/pages/files.rs before accepting it. 9. Claims I am NOT making.
  There is no peer-reviewed controlled trial showing a measured reading gain
  for Atkinson Hyperlegible specifically; its disambiguation is real and
  measurable in the outlines but "designed to" is the defensible phrasing
  and "proven to" is not. It was built for LOW VISION readers, not for
  dyslexia, and any user-facing copy should say so. And I did not render
  either face at 11.9 px and look at it, so every legibility statement here
  is outline geometry and table data, not observed rasterisation. The
  snapshot pass is where that gets settled.

### 4. Prepend one face, change nothing else

- **UI face:** Atkinson Hyperlegible Next Regular, static TTF, from
  googlefonts/atkinson-hyperlegible-next at the commit google/fonts pins
  (7925f50f, served by Google Fonts as v7). Native: one file,
  AtkinsonHyperlegibleNext-Regular.ttf, INSERTED AT INDEX 0 of egui's
  Proportional family, with Ubuntu-Light, NotoEmoji-Regular and
  emoji-icon-font left in the list behind it exactly as they are today.
  Regular only, no Bold: egui's RichText::strong() is colour, not weight
  (egui-0.31.1/src/widget_text.rs:207-210, applied as strong_text_color() at
  :439-440), so a bold file would be unreachable without inventing a named
  family and editing every call site. Web: the same design as a self-hosted
  variable woff2 (wght 200-800 in one file, since CSS does use real
  weights), prepended to the existing stack at web/shared/theme.css:144
  rather than replacing it.
- **Mono:** Keep Hack-Regular 3.003 exactly as it ships today, compiled in by
  epaint_default_fonts 0.31.1. Change nothing on native. On web, leave
  monospace as a system stack, and normalise the five declarations that name
  fonts nobody ships (web/pages/files.html:111, :120, :144, :189 and
  web/pages/accord.html:64 all ask for 'JetBrains Mono', 'Fira Code' or
  'Cascadia Code' while the repo contains zero font files, so they already
  fall through) to a plain ui-monospace, SFMono-Regular, Menlo, Consolas,
  monospace. This asymmetry is deliberate. A UI face at 11.9 to 16.5 px
  carries the brand and does the legibility work; a monospace face is doing
  one job, fixed advance width for hashes, code fences and file listings,
  where the reader compares characters rather than perceiving a typeface.
  Hack also has the best symbol coverage of anything currently in the tree,
  and swapping it would churn all 38 snapshots and re-open
  tests/icon_glyph_lint.rs for no measurable gain.
- **CJK:** Ship nothing. Japanese and Chinese render exactly as they do today, and
  this proposal does not claim to fix them. The honest state, verified by
  the coverage agent parsing the four bundled binaries: not one of
  Ubuntu-Light, Hack-Regular, NotoEmoji-Regular or emoji-icon-font contains
  a single CJK, kana or Hangul codepoint, so native CJK is tofu right now;
  web falls through to whatever the OS supplies via the sans-serif tail of
  web/shared/theme.css:144. Nothing in this change makes that worse, because
  prepending a Latin face cannot affect codepoints it does not contain.
  Bundling a fix costs roughly 12 to 16 MB natively (measured by the
  coverage agent: NotoSansJP-Regular.otf 4.53 MB plus NotoSansSC-Regular.otf
  8.33 MB, or 15.70 MB for one pan-CJK weight) and buys nothing today,
  because no GUI code reads data/i18n/ at all: the files are embedded at
  src/embedded_data.rs:113-117 and consumed only by web/shared/i18n.js.
  Native localisation does not exist. When it does, the answer is the trick
  src/gui/fonts.rs already uses for emoji, read the OS CJK font off the
  user's machine and redistribute nothing, at 0 MB and 0 licence obligation.
  That is a separate increment with its own evidence, and folding it into a
  font-cohesion change would multiply the risk by ten for a language nobody
  has asked for.
- **Emoji:** Unchanged. Keep src/gui/fonts.rs reading the user's system emoji font at
  runtime and redistributing nothing, and keep painting UI chrome as vector
  shapes via the 43 paint_* helpers in src/gui/widgets/icons.rs. Colour
  emoji is not a decision available to us: epaint rasterises every glyph
  through outline_glyph into a single-channel coverage atlas, so a bundled
  colour font would render as flat monochrome outlines at best and as
  invisible advance-width gaps at worst, and would cost 10 to 25 MB for the
  privilege. The one change worth making here is a comment fix rather than a
  file: src/gui/fonts.rs:1-16 promises the OS emoji set "renders properly",
  which holds on Windows, where Segoe UI Emoji is COLR over real outlines,
  and very likely fails on macOS and Linux, where sbix and CBDT are
  bitmap-only.
- **Reading comfort:** Keep the toggle, delete the font, raise the spacing, rename the label, and
  stop calling it a treatment. DELETE the family override at
  web/shared/theme.css:226. It is a no-op on two of the four platforms the
  operator named: fontconfig's shipped 30-metric-aliases.conf carries metric
  aliases for Arial, Times New Roman, Courier New, Georgia, Cambria, Calibri
  and Symbol and has no entry for Comic Sans MS, Trebuchet MS, Verdana or
  Tahoma, so a stock Linux box falls through to generic sans-serif, and
  Android ships Roboto plus Noto and has none of the four either. On Linux
  and Android the toggle today changes only the spacing. Once Atkinson is
  the base face, a mode that switches AWAY from a face built by an
  accessibility institute TO Comic Sans is actively worse for the person who
  turned it on. KEEP the rule, the toggle at web/pages/settings.html:1363,
  and the wiring at web/shared/shell.js:218. Keep letter-spacing and
  word-spacing, which are the half of that rule the evidence supports. RAISE
  the dose and couple the two values. Current is letter 0.03em, word 0.08em,
  a 2.7:1 ratio. Galliussi et al. (Annals of Dyslexia 2020, 128 Italian
  children) found that raising letter spacing WITHOUT raising word spacing
  proportionally significantly slowed both dyslexic and typical readers,
  because words stop segmenting when the two gaps look alike; the British
  Dyslexia Association Style Guide 2023 asks for word spacing at least 3.5
  times the inter-letter spacing. So make it a single slider, letter spacing
  0 to 0.12em (the WCAG 1.4.12 robustness ceiling), with word spacing
  computed at 3.5x rather than authored separately, defaulting near the
  middle instead of at today's near-placebo 0.03em. One control, one ratio,
  impossible to set wrong. RENAME it "Letter and word spacing" or "Easier
  reading spacing". It is not a dyslexia intervention and must not be
  described as one in shipped copy. The 2026 meta-analysis in Annals of
  Dyslexia (15 studies, 91 effect sizes, N = 688) puts the pooled effect of
  dyslexia-specific typefaces at Hedges g = -0.04, 95% CI [-0.15, 0.07].
  Marinus et al. (Dyslexia 2016) matched plain Arial to Dyslexie's spacing
  and the whole advantage vanished. The spacing is the mechanism; the font
  never was. NATIVE gets no letter-spacing control in this increment, and I
  will not pretend otherwise. epaint exposes extra_letter_spacing per
  RichText only (egui-0.31.1/src/widget_text.rs:134,
  epaint-0.31.1/src/text/text_layout_types.rs:259) with no global in Style,
  so mirroring it means touching every text site in src/gui/. There is no
  word-spacing field at all, so that half can never be mirrored. What native
  DOES already have is the other evidence-backed lever, sitting unlabelled:
  the four size tokens at data/gui/theme.ron:28-31 are already user-editable
  sliders in the Settings Fonts card, and Rello, Pielot and Marcos (CHI
  2016, 104 participants, eye-tracked) found readability improved
  continuously up to 18pt. Label that card as the accessibility control it
  already is, and log the native letter-spacing gap in
  docs/design/in-app-ops.md rather than half-building it. And the largest
  legibility win in this whole proposal needs no toggle at all: today's
  default Proportional face is Ubuntu-LIGHT, and Burmistrov et al. (NordiCHI
  2016) found light and ultra-light weights less legible than regular at
  both contrast levels and both polarities, with oculomotor measures
  indicating higher cognitive load. Replacing a light weight with a Regular
  drawn by the Braille Institute for low-vision readers lands for every
  user, on every platform, with nothing switched on.
- **Shipped bytes:** Desktop archive: +65,068 bytes for AtkinsonHyperlegibleNext-Regular.ttf,
  plus roughly 4 KB of OFL.txt, so about 69 KB. The font is compiled into
  the exe via include_bytes!, so the exe grows by 65 KB against its current
  68.3 MB, and the release download grows by the same against 166.7 MB,
  which is 0.04 percent. Nothing is removed: the 1,407,752 bytes of epaint
  default fonts (Ubuntu-Light 361,676, Hack-Regular 309,408,
  NotoEmoji-Regular 418,804, emoji-icon-font 317,864, all present in
  ~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/epaint_default_fonts-0.31.1/fonts/)
  stay, because they are the fallback tail that makes this change safe. Web
  page load: 33,996 bytes for the latin variable woff2 slice covering
  weights 200 to 800 in one file, plus 19,092 for latin-ext served under a
  separate unicode-range so English, Spanish and French pages fetch only the
  first. So 34 KB on a typical first load, then cached, then precached by
  the service worker for the installed Android PWA. No mono, no CJK, no
  emoji, no second weight, no CDN request. Byte figures for the Atkinson
  files and the Google Fonts slices come from the specialist agents' fetches
  of the GitHub contents API and fonts.gstatic.com in this workflow, not
  from my own measurement. The epaint font sizes and the repo facts I
  verified myself.
- **Gives up:** It gives up the phrase "one font". This is one Latin face on top of the
  existing pile, not a replacement for it. Atkinson maps 362 codepoints and
  has no Greek, no Cyrillic, no arrows, no check mark and no warning sign,
  so most of the app's symbol glyphs still come from Ubuntu-Light, NotoEmoji
  and the user's OS emoji font, exactly as today. That is the reason the
  change is safe and it is also the reason it is not the clean sweep it
  sounds like. If anyone describes this as "we now ship one typeface", they
  are wrong. It gives up CJK. Japanese and Chinese stay tofu on native and
  OS-dependent on web. Two of the five shipped locales get nothing. It gives
  up monospace cohesion. Hack in the app, a system mono on the site. Code
  fences and hashes will not match between the two surfaces. It gives up
  emoji cohesion permanently, not by choice. egui 0.31 cannot draw colour
  emoji, so the site will keep showing colour emoji that the app shows as
  flat monochrome or, on macOS and Linux, quite possibly as invisible gaps.
  It gives up pixel parity, which is worth stating before someone files it
  as a bug. egui rounds raster size to whole pixels, rasterises each glyph
  once at the origin into a grayscale coverage atlas, and snaps every glyph
  x position to a whole device pixel; browsers position to fractional pixels
  with hinting and subpixel coverage. The same string at the same size will
  not even have the same measured width. Shared files buy the same
  letterforms and the same proportions, which reads as the same product.
  They do not buy the same pixels, and no font choice can. It gives up user
  font choice, deliberately. No token, no picker, no per-user family. If
  that is wanted later it costs the four pieces of plumbing listed in step
  11. It gives up the claim that we have addressed dyslexia. What it
  actually delivers on that front is a better-drawn face for everyone plus a
  spacing control set to a dose the literature supports, which is the honest
  maximum available. Anyone who wants "we ship a dyslexia font" will be
  disappointed, and should be. It costs 38 snapshot PNGs of churn that need
  real review, and it makes the pre-existing disagreement between the
  snapshot harness and the runtime visible for the first time, which is a
  fix disguised as a cost. And a provenance caveat I want on the record:
  every claim in this proposal about Atkinson Hyperlegible Next itself, its
  byte sizes, its 362-codepoint cmap, its OFL 1.1 terms with no Reserved
  Font Name, its outline geometry, and the Google Fonts slice sizes, comes
  from the specialist agents' fetches and binary parsing in this workflow,
  not from my own measurement. I verified the repo facts myself, line by
  line. Before this ships, someone should download the file and confirm its
  size and cmap independently, because the whole prepend-is-safe argument
  rests on that 362-codepoint number being right.

## The judges split three ways

### Operator lens, picked: The Plex Stack: one superfamily, drawn shapes for icons, and a spacing knob instead of a dyslexia font

I verified the repo claims myself before scoring, and one measurement reorders
things. THE FREE WIN NOBODY SIZED CORRECTLY. I parsed the cmap tables of the
three fonts actually in the Proportional chain. Ubuntu-Light carries 1194
codepoints and has NO U+2190, U+2192, U+2194, U+2713, U+26A0 or U+2500.
NotoEmoji-Regular carries 887 and has U+2194 and U+26A0 but not U+2192 or
U+2713. Hack-Regular carries 1548 and HAS U+2190, U+2192, U+2194 and U+2500.
And epaint-0.31.1/src/text/fonts.rs:358-365 puts Hack in Monospace only, never
in Proportional. So the right arrow, which grep counts at exactly 111 uses
across src/gui, is absent from every font in the proportional chain and
renders solely from the operator's Segoe UI Emoji. It is blank or tofu on
Linux and macOS today. Worse, tests/icon_glyph_lint.rs:49-51 states in its own
comment that "U+2192 right arrow DOES work (in wide use)", which is a
Windows-only observation written into the lint as a cross-platform fact.
Adding Hack to the Proportional chain fixes 111 arrows, plus U+2190, U+2194
and U+2500, for zero bytes, zero licence obligation and one line. Stacks 1, 2
and 3 include it. Stack 4 does not, and that is what drops it. WHY PLEX WINS
ON THE CRITERION HE SAID HE WOULD CHECK. He asked for cohesive,
cross-platform, good-looking, and dyslexia-served. Three of the four stacks
are within noise of each other on cohesion and cross-platform, because all
three ship files instead of borrowing from the OS, and all four converge on
the same correct dyslexia answer. So the decision falls to "looks good", and
that is where the field separates. Plex Sans is a drawn typeface with a
double-storey g, an engineered voice, and a true monospace sibling at
identical metrics, and it reads like control-room software on a black ground
with an orange accent, which is what this product is. Atkinson is a signage
face from an accessibility institute; it is competent and it is not handsome
at 32.5 px titles, and its own advocates concede its lowercase i and g were
controversial. I am not going to hand him a spec sheet and call the aesthetic
question answered. Plex also happens to be the cheaper, lower-reflow option:
x-height 0.516 em against Ubuntu-Light's 0.517 and n advance 568 against 569
means the page keeps its exact horizontal rhythm, drawn with a Regular stem
instead of a Light one. THE WEIGHT POINT IS THE REAL ACCESSIBILITY WIN AND IT
IS UNIVERSAL. Nobody chose Ubuntu-LIGHT. It arrived free with egui's
default_fonts feature. Light weights measure worse than regular on legibility
with higher oculomotor load, and that penalty lands on every user, on every
platform, with nothing toggled. Replacing a light weight with a properly drawn
regular does more for the dyslexic reader, the tired reader and the
47-year-old reader than any font labelled "dyslexia" does, and the
meta-analytic effect of dyslexia typefaces is g = -0.04 across 15 studies and
N = 688, which is zero. All four stacks got that right and deserve credit for
it. WHAT I AM TRANSPLANTING INTO THE PICK. Stack 2's weakest paragraph says
native must settle for "a modest letter increment" because epaint has no
word-spacing field and raising letter spacing alone slows readers. I checked,
and Stack 1 is right and Stack 2 is wrong:
epaint-0.31.1/src/text/text_layout.rs:165-168 adds extra_letter_spacing once
per adjacent glyph pair, so a word gap crosses two pairs and gains twice what
a letter gap gains, landing above the 3.5:1 ratio the guidance asks for.
Native can therefore have a genuine spacing control, which it has never had,
and that closes the dual-UI drift where web/pages/settings.html:1363 into
web/shared/shell.js:218 into web/shared/theme.css:223-227 gives the web a
toggle the app has no equivalent of. Take Plex's face, Stack 1's u8 token (u8
is already in PRIMITIVES at tests/theme_editor_coverage.rs:140, so the editor
row is enforced with no test edit and no new regex in
scripts/gen-theme-css.js, whose parser at :31-58 has only an RGBA and a
numeric branch and would silently drop a quoted string), and Stack 1's spacing
mechanism. THE STRUCTURAL FIX ALL FOUR FOUND, AND ITS PROOF.
src/gui/ui_snapshots.rs builds five bare egui contexts and installs no fonts,
while src/lib.rs:1420 installs the emoji fallback in the running app. So the
38-page snapshot suite has been rendering a different font stack than the
product. That is not theoretical: the U+2190 entry in
tests/icon_glyph_lint.rs:52 is sourced from a snapshot, and my cmap parse
shows U+2190 is genuinely absent from the snapshot's three-font stack while
Segoe supplies it in the running app, and the U+2194 entry at :53 flags a
codepoint that IS present with real coverage in NotoEmoji-Regular. The lint is
calibrated against an environment no user has. Fix that before trusting any
diff from this work. Also worth doing regardless of face, and cheap:
src/gui/fonts.rs:41-43 returns on the first emoji font it finds and, at :46,
warns and never calls set_fonts at all, so on a machine with no emoji font any
new face would silently fail to install. And put the fonts plus the OFL text
under data/, because .github/workflows/build-desktop.yml:91 copies data/
wholesale while :92-93 copy only assets/icons/ and assets/shaders/, so
assets/fonts/ would ship nothing and the failure would surface only in the
released archive.

**Its own case against:** Plex Sans's zero is measurably worse than what ships today, and this is a
wallet. By Stack 2's own contour measurement, Plex Sans's proportional zero
has two contours with no dot and no slash, ink 480 against the O's 592, a
ratio of 0.811, where Ubuntu-Light's is 446 against 650, a ratio of 0.686. So
0 and O move CLOSER together in the face I just picked. Plex ships a zero
feature and an ss03 alternate that would fix it, and epaint cannot reach
either, because FontData exposes only font, index and tweak and ab_glyph
applies no GSUB. There is no runtime escape. Stack 2's defence is that the
identifiers this app displays are lowercase hex and base58, and base58
deliberately excludes 0, O, I and l while lowercase hex contains none of them.
That defence is narrower than it sounds. Base64 group invite tickets at
src/gui/pages/chat.rs:6448 contain the full confusable set today. And betting
an accessibility criterion on an encoding invariant is exactly the kind of
assumption that a future format change breaks silently, in the one product
surface where a misread character costs someone their identity. Atkinson's
slashed zero, three contours with two overlapping counters, is present in the
DEFAULT instance and needs no feature the engine cannot reach, and its capital
I is 12 points where a bare stem is 4. That is the one criterion where
Atkinson is strictly, structurally better, and it is a criterion the operator
explicitly asked about. So if he weighs "dyslexic people served" above "looks
good", I am wrong and Stack 1 is the pick. My honest read is that he asked for
both and named looks first, that the spacing and size controls are where the
dyslexia evidence actually lives and both stacks deliver those identically,
and that Plex Mono's dotted zero covers every surface where a zero is actually
read character by character. But I would not fight him on it, and if he says
the wallet is the deciding surface then Stack 1 wins on its merits, not as a
consolation. A second, smaller case against: Plex declares a Reserved Font
Name, so any future subset or feature-freeze must be renamed, where Atkinson
declares none and can be modified freely. If we ever want to bake that slashed
zero in ourselves, which is the only way to get it, Atkinson permits it and
Plex does not.

**Corrections found:** Errors I found, verified in this checkout unless marked otherwise. 1. WRONG
LINE NUMBER, three stacks. Stacks 1, 2 and 4 all place the fifth bare egui
context in src/gui/ui_snapshots.rs at :661. Grepping the file returns exactly
five matches for Context::default(), at 328, 470, 533, 602 and 1626. There is
nothing at 661. Stack 3 is the only one that got this right. Minor, but three
agents converging on the same wrong number suggests it was copied rather than
checked. 2. THE LINT CONTAINS A WINDOWS-ONLY CLAIM PRESENTED AS FACT.
tests/icon_glyph_lint.rs:49-51 comments that "U+2192 right arrow DOES work (in
wide use)". I parsed the cmaps: U+2192 is absent from Ubuntu-Light (1194
codepoints), absent from NotoEmoji-Regular (887), and Hack is not in the
Proportional family per epaint-0.31.1/src/text/fonts.rs:359-364. It renders
only via the operator's Segoe UI Emoji. That is 111 usages across src/gui that
are blank or tofu on Linux and macOS. This is a repo defect, not a proposal
error, but Stack 4's plan preserves it while claiming to solve cross-platform
cohesion, which makes it an error in that proposal. 3. STACK 2 IS WRONG ABOUT
NATIVE LETTER SPACING, AND IT MATTERS. It states native must settle for "a
modest letter increment" because epaint has no word-spacing field and
Galliussi showed letter-without-word spacing harms readers.
epaint-0.31.1/src/text/text_layout.rs:165-168 adds extra_letter_spacing once
per adjacent glyph pair, so the gap around a space accumulates two increments
while a letter gap accumulates one. The word-to-letter ratio therefore rises
automatically with the slider and clears 3.5:1 at a realistic dose. Stack 1's
version of this claim is correct. Fix the paragraph, do not fix the plan. 4.
STACK 3 PROBABLY MIS-ROUTES THE GREEK MU. It says Atkinson has no Greek and
that the mu at src/gui/pages/cosmos.rs:1201-1202 "falls through to
Ubuntu-Light", while Stack 1 says Atkinson contains U+03BC. The coverage
research reported Greek at 4 of 144 codepoints, described as strays such as
mu, which supports Stack 1. Harmless either way, since Ubuntu-Light does carry
Greek, but the two stacks cannot both be right and neither flagged the
conflict. 5. UNRESOLVED SOURCE CONFLICT ON PLEX BYTE SIZE. Stack 2 gives
IBMPlexSans-Regular.ttf as 200,500 bytes from IBM/plex master; the
Inter-versus-Plex research measured 218,236 from google/fonts. Different
builds, both plausibly real. Pin one source before vendoring, and prefer
IBM/plex so the exe and the browser carry identical outlines. 6. STACK 2's
LINT REMOVAL LIST IS PART SPECULATION. It nominates U+2261, U+22A0 and U+25A4
as removal candidates on the strength of Hack's coverage. I did not probe
those three and neither did it, in a rendered sense. Its own instruction to
re-derive BROKEN_GLYPHS from PNGs rather than cmap tables is the right one and
should override its own table. 7. PROVENANCE, stated plainly because the brief
demands it. Everything I assert about Ubuntu-Light, Hack-Regular and
NotoEmoji-Regular coverage I measured myself this session by parsing their
cmap tables out of the epaint_default_fonts-0.31.1 crate, and the 111 and 12
glyph counts are my own greps. Everything about Atkinson Hyperlegible, IBM
Plex, Inter, Lexend and Noto, their byte sizes, contour counts, licences and
codepoint totals, comes from the specialist agents' fetches, not from my own
measurement, and I did not recall any of it. Before this ships, someone should
download the chosen face and confirm its size and cmap independently, because
the whole prepend-is-safe argument rests on the fallback chain covering what
the new face does not. 8. NOT AN ERROR BUT NOBODY COSTED IT. Stack 1 spends
1,233,128 bytes on Noto Sans Symbols 2 to close seven codepoints. Once Hack is
in the Proportional chain, the remainder is roughly the check mark at 12 uses
plus a handful of weather glyphs at src/gui/pages/hud.rs.
src/gui/widgets/icons.rs already holds 43 paint_* vector helpers written for
precisely this, and they take a Color32 so they consume theme tokens, which a
glyph never can. That is 1.2 MB for about 20 usages that the repo can already
draw.

### Accessibility lens, picked: Atkinson Everywhere

Judged on accessibility delivered rather than typeface chosen, and all four
candidate faces are defensible so the face is not the deciding variable. Every
proposal clears the first bar: none repeats the dyslexia-font claim, all four
cite the failed literature, and all four delete or neutralise the Comic Sans
override at web/shared/theme.css:226. That is the right call, and the current
rule is worse than the proposals allow, because on Linux and Android the
family override is already a no-op, so today's toggle is 0.03em letter and
0.08em word spacing (web/shared/theme.css:226-227) presented to the user as a
dyslexia feature. That is roughly a quarter of the studied dose wearing the
label of an intervention.\n\nSo the question becomes which proposal moves the
most load-bearing non-font levers, on the most surfaces. Atkinson Everywhere
wins because it delivers spacing, line height and size to both clients, and
because it is the only one that verified the native spacing mechanism instead
of assuming it. That verification is the crux. epaint adds the letter-spacing
increment at each glyph boundary
(epaint-0.31.1/src/text/text_layout.rs:163-166), so an inter-word gap
accumulates two increments plus the base space advance while an inter-letter
gap accumulates one. Native therefore scales word spacing implicitly and lands
in the both-increased condition, which is the condition Zorzi and Stagg and
Kiss found helpful, not the letters-only condition Galliussi found actively
slowed both dyslexic and typical readers. P2 assumed the opposite and
deliberately weakened its own native mode as a result; P3 and P4 declined to
build the native control at all. One person read the code and it changed the
design.\n\nIt is also the only proposal to treat emphasis as an accessibility
question. egui's strong() adjusts colour, not weight, which means emphasis in
the entire native app is currently conveyed by colour alone, and a shipped
bold face is the fix. Nobody else proposed one.\n\nTwo conditions on the pick.
First, take P4's coupled slider instead of P1's three independent tokens: word
spacing computed at a fixed ratio from letter spacing cannot be misconfigured
later, whereas three free tokens can be driven straight into the harmful shape
by a well-meaning future edit. Second, add the lever all four missed, which is
measure. WCAG 2.2 SC 1.4.8 caps line length at 80 characters and 40 for CJK,
the BDA 2023 guide calls 60 to 70 optimal, and this repo already has the
control: a content-width slider at web/pages/settings.html:782-785 applied at
web/pages/settings-app.js:227. It defaults to 0, meaning Full
(settings.html:782, reset at settings-app.js:285), and its selector list at
web/shared/theme.css:158 does not match the Library page, which wraps content
in #page-app at 1300px (web/pages/library.html:14). So the longest-form prose
in the product runs unbounded and cannot be constrained even by a user who
finds the slider. Folding measure into the reading-comfort control is cheaper
than any font decision here and is better supported than the letterform
argument every proposal leans on.

**Its own case against:** The strongest case against my pick is that every accessibility gain in it is
separable from the typeface swap, and Atkinson Everywhere welds them together.
The spacing tokens, the coupled ratio, the size increase, the relabelled
toggle and the measure fix all work identically under Ubuntu-Light. What the
font swap adds is 1.4 MB, a 13 percent taller line box on every native row, 38
snapshot diffs, a deliberate raise of four hand-tuned size tokens on top of
that reflow, and a Latin-only face whose 362 codepoints leave the arrows,
checks and warning glyphs rendering from the old chain anyway. If any of that
destabilises layout and the increment gets reverted, the accessibility work
goes out with it, because it is in the same commit. P4 saw this and refused
the bundle on purpose, and P4 is right that making the current choice explicit
and attributed is most of the value at a fraction of the risk.\n\nThere is a
second, sharper version. The letterform-disambiguation argument that justifies
Atkinson over the alternatives is supported for low vision and threshold
legibility (Beier and Larson 2010, Beier and Oderkerk 2022), and specifically
NOT supported for dyslexic reading: Galliussi et al. 2020 built a letterform
to break b-d-p-q symmetry and measured no effect in either dyslexic or typical
children. So on the operator's stated dyslexia goal, the face contributes
approximately nothing that spacing and size do not contribute more of. If I am
ranking by evidence per unit of risk rather than by evidence delivered, P4
wins, and I would not argue hard against someone who picked it.\n\nA third,
narrower objection: I am rewarding P1 partly for reading the epaint layout
loop, but its resulting ratio claim is overstated, so the depth of
verification is better than the arithmetic it produced.

**Corrections found:** 1. The Plex Stack asserts that native cannot raise word spacing and
under-doses the native control accordingly. That is wrong. epaint applies
extra_letter_spacing once per glyph boundary
(epaint-0.31.1/src/text/text_layout.rs:163-166, inside the per-char loop,
skipped only for the first glyph and where font_impl is None). A space
character sits between two boundaries, so the visual inter-word gap grows by
two increments plus the base space advance while an inter-letter gap grows by
one. Native is therefore NOT in Galliussi's harmful letters-only condition,
and the proposal weakened its own accessibility mode to avoid a harm that does
not occur.\n\n2. Atkinson Prepend's dose contradicts the guidance it cites. It
quotes the British Dyslexia Association's requirement that word spacing be at
least 3.5 times the inter-letter spacing, then prescribes letter-spacing
0.12em with word-spacing 0.16em, a ratio of 1.33 to 1. It also treats WCAG 2.2
SC 1.4.12's 0.12em and 0.16em as a recommended reading dose. They are not:
1.4.12 is a robustness criterion stating that content must remain usable WHEN
a user applies those values, not a prescription of what to apply.\n\n3.
Atkinson Everywhere's 4.1 to 1 word-to-letter ratio is flattering arithmetic.
It compares the full inter-word gap (a roughly 0.25em space advance plus two
0.12em increments) against the tracking increment alone, omitting the natural
sidebearings that form part of every inter-letter gap. Counting sidebearings
at a typical 0.05 to 0.08em, the real ratio is nearer 2.7 to 1, which is below
the BDA's 3.5 to 1. The conclusion that native avoids the harmful condition
survives; the number should not be quoted in a commit message or in user
copy.\n\n4. Correction to the research brief rather than to a proposal: the
Atkinson research states that because the x-height to cap-height ratio is
similar (0.74 against Ubuntu-Light's 0.75), \"switching will not visually
shrink the text\". At an equal pixel size the governing figure is x-height per
em, and 496 against 517 per 1000 upem is about 4 percent smaller. Atkinson
Prepend caught this and contradicted the brief correctly; Atkinson Everywhere
independently assumes the sizes need raising, which is the right
response.\n\n5. A repo fact none of the four proposals found, and the most
consequential omission in the set. Line length is unaddressed everywhere, yet
the control already exists: a content-width slider at
web/pages/settings.html:782-785, applied to --content-width at
web/pages/settings-app.js:227. It ships defaulted to 0, meaning Full
(web/pages/settings.html:782, reset to 0 at web/pages/settings-app.js:285), so
prose is unbounded out of the box. Its selector list at
web/shared/theme.css:158 covers only .page-content, #settings-main, #messages
and main, while the Library page wraps its content in #page-app at max-width
1300px (web/pages/library.html:14) and matches none of them, so the
longest-form reading surface in the product cannot be constrained even by a
user who finds the slider. WCAG 2.2 SC 1.4.8 caps at 80 characters and 40 for
CJK, which is directly relevant given data/i18n/ja.json and
data/i18n/zh.json.\n\n6. A second shared omission on the display condition.
data/gui/theme.ron:2 sets bg_primary to pure black and :11 sets text_primary
to (0.91, 0.91, 0.918), so the native app is near-white on pure black at
maximum contrast. The BDA 2023 guide explicitly warns against maximum-contrast
white grounds and asks for off-white or cream, and the inverted case produces
exactly the halation The Plex Stack describes. Only that proposal mentions
halation, and only as an argument for a heavier stem, not as a reason to offer
a softened ground. Since the theme is fully token-driven this is a default
value, not new code, and a reading-comfort mode that adjusts spacing while
leaving the ground at maximum halation is doing half the job.\n\n7. Minor, and
it cuts across three proposals. Atkinson Prepend and Prepend one face both
correctly identify that egui's RichText::strong() changes colour rather than
weight, and both then accept it. That is emphasis conveyed by colour alone,
which is a real accessibility defect for low-contrast and
colour-vision-deficient readers, and only Atkinson Everywhere proposes
shipping a bold face to fix it.\n\n8. Framing note rather than an error: the
letterform-disambiguation evidence that all four proposals lean on (slashed
zero, serifed I, non-mirrored b and d) comes from low-vision and
threshold-legibility work, and Galliussi et al. 2020 tested a letterform
designed specifically to break b-d-p-q symmetry and found no effect in
dyslexic or typical children. Atkinson Prepend and The Plex Stack both state
this distinction cleanly; any shipped copy must not let the low-vision
evidence do rhetorical work for a dyslexia claim.

### Engineering lens, picked: Prepend one face, change nothing else

Judged on what has to change in this tree, not on which typeface is
nicer.\n\nThe four gates the brief names resolve almost entirely in favour of
the smallest proposal.\n\nicon_glyph_lint coupling: this is decided by prepend
versus replace, and every proposal gets that right, but only the minimal one
keeps the consequence at zero. tests/icon_glyph_lint.rs BROKEN_GLYPHS is 12
entries derived from operator screenshots, and its own header says to remove
entries when a font upgrade fixes them. Prepending a 362-codepoint Latin face
behind which the whole existing chain survives cannot regress it. Every
proposal that also moves Hack into the Proportional chain, swaps the mono
face, or adds a symbols font is claiming lint removals it cannot prove from a
cmap table, and the lint's own comment already contradicts the simplest
version of that story: it records U+2192 as working while U+2190 and U+2194
tofu, which cannot be explained by \"Ubuntu-Light lacks arrows\"
alone.\n\ntheme_editor_coverage: I read the test. PRIMITIVES has no String
case and extract_size_fields matches on a type substring, so pub font_family:
String passes with no editor row, which is the exact silent failure the test
exists to prevent. Two proposals dodge it (u8, or no token at all), one edits
the test to add String, one declines the token. Editing a guardrail inside a
font change is the worst of those, even though I confirmed it is harmless
today because theme.rs has zero String fields.\n\nToken flowing to both
clients: parseTheme in scripts/gen-theme-css.js handles an RGBA tuple and a
numeric scalar and nothing else, and the numeric regex needs a trailing comma
or paren. Adding a family token means either a new parse path, a new data
registry with a new reader, or a numeric enum. That is real plumbing, and it
is the single thing most likely to turn a one-session change into a
three-session one. The honest position is that hardcoding one face in two
places is a drift risk, and adding a token is a scope risk, and with exactly
one face shipping the drift risk is near zero while the scope risk is
not.\n\nCJK bytes: all four correctly refuse to bundle. 4.53 MB for JP plus
8.33 MB for SC, against data/i18n/ja.json and zh.json that no GUI code reads
(they are embedded at src/embedded_data.rs and consumed only by
web/shared/i18n.js), is an obvious no. Three proposals then add a system-font
CJK probe anyway. That is a good idea and it is a separate increment: it
changes what a Japanese user sees, it needs its own platform path testing on
three operating systems, and folding it into a cohesion change means a bug in
either one is attributed to the other.\n\nRelease archive and OFL: this is
where the minimal proposal earns its top score. data/ is copied wholesale at
build-desktop.yml:91, assets/icons/ and assets/shaders/ are copied
individually, and nothing else ships. There are zero font files tracked in git
today and no licence text of any kind in the archive, while the exe already
redistributes four faces through epaint_default_fonts. data/credits.ron has no
font row and src/credits.rs:110 will fail the build if an attribution-required
source names no surface. That gate turning red is the mechanism that makes the
licence obligation real, and it costs about 4 KB and one RON row. That work is
correct whatever typeface wins, and it is the part of this that is genuinely
overdue.\n\nThe finding all four share and that should ship regardless:
src/gui/fonts.rs:41-43 calls ctx.set_fonts inside the candidate loop and
returns, and falls through to a warn at :46 with no set_fonts at all when no
emoji font is found; and src/gui/ui_snapshots.rs creates five bare
egui::Context::default() instances and installs no fonts, so the headless
snapshot rig and the running app already render different font stacks today.
Fix both before touching a typeface, or the 57-PNG diff you review afterwards
is not evidence.

**Its own case against:** The strongest case against my pick is that it does not answer the question
that was asked.\n\nThe operator asked for one cohesive typeface across web and
app. What this ships is one Latin face prepended onto the existing pile, with
monospace still divergent (Hack in the app, a system stack on the site),
symbols still coming from three different faces, CJK untouched, and no theme
token. If someone describes the result as \"we now ship one font\", they are
wrong, and the proposal says so itself. That is honest, but honesty about not
solving the problem is not solving the problem.\n\nThe token refusal is the
part I am least comfortable defending. CLAUDE.md names fonts explicitly as a
theme.ron token class, and this proposal hardcodes the family in
src/gui/fonts.rs and again at web/shared/theme.css:144 with nothing binding
them. That is two sources of truth for a design decision, which is precisely
the failure mode the rule was written to stop, and it is being accepted to
save maybe sixty lines. The next session that wants a second face or a
user-facing picker has to build the plumbing anyway, under pressure, without
the context this workflow just generated.\n\nAnd the Plex Stack is a genuinely
better answer to the literal ask for a comparable amount of work. A real
superfamily whose sans and mono share x-height 0.516 and cap-height 0.698
exactly, that measures within 0.4 percent of Ubuntu-Light on both x-height and
set width so the four size tokens survive untouched, and that has
purpose-drawn CJK siblings by the same studio for when ja and zh stop being
37-string stubs. For 373 KB. It ships two faces instead of one and it also
declines a family token, so its lint exposure is barely worse than my pick's.
If the operator's actual priority is cohesion rather than shipping something
today, the ranking inverts.\n\nMy pick wins on \"what has to change\", which
is the lens I was told to use. It loses on \"what was asked for\".

**Corrections found:** Verified against the tree, read-only.\n\n1. The fifth bare
egui::Context::default() in src/gui/ui_snapshots.rs is at line 1626, not 661.
Grep returns exactly five sites: 328, 470, 533, 602, 1626. \"Atkinson
Everywhere\", \"The Plex Stack\" and \"Prepend one face\" all cite :661; only
\"Atkinson Prepend\" has it right. Minor, but three proposals would send
someone to a line that has no context construction on it.\n\n2. All four say
38 snapshot pages, taken from CLAUDE.md. tests/snapshots/ actually holds 57
PNG files. The review burden after a font change is about 50 percent larger
than every proposal budgets for, and reviewing that diff page by page is the
only thing that catches the 13 percent line-box growth.\n\n3. \"Atkinson
Everywhere\" lists 10 hardcoded 'Segoe UI' stacks. There are 11. It misses
web/chat/chat-profile.js:673. \"Prepend one face\" lists all 11 correctly,
including profile.html:1648, which a naive grep drops because the quotes there
are backslash-escaped inside a JS string.\n\n4. \"Atkinson Prepend\" step 10
proposes adding String to PRIMITIVES in tests/theme_editor_coverage.rs without
stating the blast radius. I checked: src/gui/theme.rs has zero String fields
today, so the edit turns nothing red. The conclusion holds, the proposal just
did not do the check that makes it safe to assert.\n\n5. A trap none of them
names: the scalar regex in parseTheme (scripts/gen-theme-css.js) is
/(\\w+)\\s*:\\s*([\\d.eE+-]+)\\s*[,)]/g, so it requires a trailing comma or
closing paren. A numeric token added as the final field of a RON struct
without a trailing comma is silently dropped, same failure class as the string
problem they did correctly identify.\n\n6. \"Atkinson Everywhere\" claims that
once Hack joins the Proportional chain, U+2190 and U+2194 start working and
src/gui/widgets/icons.rs paint_arrow_left and paint_arrow_both can retire.
Unproven. tests/icon_glyph_lint.rs:50-52 records that U+2192 DOES work and is
in wide use, while U+2190 and U+2194 tofu'd in a snapshot. If Ubuntu-Light
lacks all three (as the research claims), the working U+2192 is already
arriving from the system emoji font, so \"Hack is missing from Proportional\"
does not explain the observed split and may not be the cause. Re-derive from
rendered PNGs, and note that fonts.rs is only installed on the app path, so
any explanation involving the system emoji font also means the snapshot rig
cannot reproduce it until that gap is fixed.\n\n7. \"The Plex Stack\" claims
five BROKEN_GLYPHS entries become removal candidates (U+2190, U+2194 from Plex
Sans; U+2261, U+22A0, U+25A4 from Hack). The Hack half is not a consequence of
adopting Plex, it is a consequence of adding Hack to the Proportional chain,
which is an independent change that could be made today with no new font at
all. Worth separating, because it is free.\n\n8. Both \"The Plex Stack\" and
\"Prepend one face\" vendor the TTF into data/ and then include_bytes! it from
there. That works, and it makes the licence text travel automatically, but it
ships the same bytes twice: once compiled into the exe and once copied to
dist/HumanityOS/data/ by build-desktop.yml:91. Harmless at 65 to 200 KB, and
worth stating rather than discovering.\n\n9. Not an error, but it changes how
a claim should be worded: the CSP at scripts/nginx/humanity.conf:53, :67 and
:210 does set font-src 'self'. So the CDN option is already closed by the
deployed policy, and self-hosting is not a recommendation, it is the only
thing the server permits without a header change.\n\n10. Every claim in all
four proposals about a font binary's byte size, cmap contents, contour counts
or vertical metrics comes from the specialist agents' downloads, not from
anything in this repo. I could not verify any of them here and I am not
treating them as established. The prepend-is-safe argument rests on Atkinson's
362-codepoint cmap being correct; someone should confirm that from the file
before it ships, because if that number is wrong in the other direction it
changes nothing, but if the fallback chain is disturbed it changes everything.



---

# Second evaluation: widening the search, and what it overturned

The operator asked whether anything better existed than the two faces offered.
A second 15-agent pass parsed real binaries for Noto, Fira, the SIL faces,
Source Sans 3, Public Sans, Lexend, Recursive, Luciole, Intel One Mono, B612,
APHont, Tiresias, Lexica Ultralegible, Inclusive Sans and nine monospaces, and
re-measured the incumbents as a parser control. One agent wrote a TTF parser
and validated it by reproducing every incumbent baseline exactly before
trusting a single new number.

It overturned three things from the first evaluation, and surfaced one defect
that outranks the whole font question.

## THE FINDING: the most dangerous string in the app is set in a proportional Light face

`src/gui/pages/settings.rs:612` renders the 24-word BIP39 seed phrase as a bare
`ui.label(RichText::new(&phrase).color(theme.warning()).size(theme.font_size_small))`,
with no `.monospace()`, inside a warning-framed box whose own comment calls it
"the single most dangerous string in the app". Forty lines of onboarding away,
`src/gui/pages/main_menu.rs:474` renders the SAME 24 words with
`.monospace().size(13.0)`. Same content, two fonts, one product.

Eight call sites render a string a human must read character by character, and
none of them selects a monospace face:

- `settings.rs:531` public key
- `settings.rs:612` the 24-word seed phrase
- `settings.rs:732` the recovery TextEdit, which needs
  `.font(FontId::monospace(..))` because TextEdit takes no RichText
- `settings.rs:752` recovered-key confirmation
- `profile.rs:174` public key
- `chat.rs:5808` member key
- `chat.rs:6272` and `chat.rs:6455` the base64 group invite ticket, plain
  `TextEdit::multiline`

`settings.rs` and `profile.rs` contain zero `.monospace()` calls today.

The base64 ticket is the ONLY string type in this product where 0, O, I and l
all coexist, and it is proportional. Hack is already fonts[0] of the Monospace
family, already has a dotted zero, a serifed I and a tailed l in its default
instance, and costs zero bytes. Routing those eight sites is a larger
real-world legibility win than any typeface swap in either evaluation, and it
is a handful of lines.

## Overturned 1: the slashed-zero test measured a confusion this product cannot express

The first evaluation implicitly required the PROPORTIONAL face to carry a
slashed zero, and that single assumption promoted Atkinson, Lexica, Andika,
Luciole, APHont and Tiresias, weakened IBM Plex, and eliminated Inter. It was
the wrong test. DIDs and the Solana address are base58
(`src/relay/core/did.rs`), whose alphabet omits 0, O, I and l by construction.
Public keys are lowercase Dilithium hex, so no O, I or l can occur. The BIP39
wordlist (`src/net/bip39_wordlist.rs`) is pure lowercase a to z. The only
surface carrying the full confusable set is the base64 invite ticket, and the
fix for that is monospace routing using a face already shipped.

Worse, the faces that test promoted carry a cost nobody measured: PROPORTIONAL
default digits. Atkinson 59.2 percent digit advance spread, Lexica 61.2, Public
Sans 59.2, Fira Sans 28.9, with tabular figures locked behind the `tnum`
feature, which is exactly as unreachable as the slashed zero. This app renders
live numbers constantly, so those faces would fix an impossible confusion while
introducing permanent horizontal shimmer in every readout and slider value.
Ubuntu-Light (564 flat), IBM Plex Sans (600 flat) and Noto Sans (572 flat) are
tabular by default.

**Tabular figures was the constraint that should have occupied the slashed-zero
slot.**

## Overturned 2: IBM Plex Sans is a script REGRESSION against what ships today

Measured, not quoted. IBM Plex Sans carries 891 to 895 codepoints against
Ubuntu-Light's 1194. Cyrillic 192/256 against Ubuntu-Light's 214/256, so FEWER
than the face it would replace. Greek Extended 0/256 against Ubuntu-Light's
233/256. Cyrillic Supplement 2/48. Latin Extended-B 33/208. A bare Plex swap
loses scripts. It is only safe because epaint family lists are an ordered
per-glyph fallback chain, so keeping Ubuntu-Light as the SECOND entry backfills
at zero cost, since it is already compiled in.

Noto Sans measures 2965 codepoints: Cyrillic 256/256, Cyrillic Supplement
48/48, Greek 121/144 plus Greek Extended 233/256, Latin Extended-B 208/208,
Latin Extended Additional 256/256, complete Vietnamese. It is also OFL-1.1 with
NO Reserved Font Name, verified in both the upstream OFL.txt and the binary's
name table, where IBM Plex reserves "Plex". That matters concretely: zero.slash
already exists as glyph 2251 in the shipped Noto static, so a one-time cmap
remap of U+0030 gives a slashed zero in the default instance with no GSUB and
no renaming. The same edit on Plex would legally require a rename and a
permanent fork.

What Plex still wins, and it is the axis that gates a one-session ship: set
width. Plex n advance 568 against Ubuntu-Light's 569, a 0.2 percent match, a
true horizontal drop-in. Noto is 618, plus 8.6 percent. `src/gui/` holds roughly
340 fixed-width call sites (133 `desired_width`, 93 `allocate_exact_size`, 72
`min_size`, 42 `set_min_width`, 19 `columns`) plus `sidebar_width` 200.36 and
`modal_width` 300.0 in theme.ron. Vertical growth is a slider and is
recoverable; horizontal set width is not a slider and moves all of them.

## Overturned 3: IBM Plex Mono should be retired from the recommendation entirely

Hack, the incumbent that arrives free with `epaint_default_fonts`, beats it
outright and the margin is not close.

- **Hack:** 1548 codepoints, Arrows 109/112, Math Operators 177/256, Box
  Drawing 128/128, line box 1.1641 em (tightest in the field), dotted zero,
  serifed I at ink 405 against a tailed l at 427.
- **IBM Plex Mono:** about 1049 codepoints, Arrows 22/112, Math Operators
  13/256, line box 1.300 em, exactly ONE codepoint in the entire Greek block,
  its good slashed zero locked behind the unreachable `zero` feature, its
  dot-zero the faintest of nine monos measured (0.097 against Hack's 0.257),
  and its capital I and lowercase l rendering as effectively the same bitmap at
  11px (separation 0.118, worst of nine).

The F2/F3/F4 debug overlays and cosmos.rs live on box drawing and arrows. Every
mono swap measured trades symbol coverage for a taller row, for a property Hack
already has.

## The spacing knobs that were available the whole time

egui 0.31.1 exposes `RichText::extra_letter_spacing(f32)` at
`egui-0.31.1/src/widget_text.rs:134` and `RichText::line_height(Option<f32>)` at
`:148`, both backed by TextFormat fields in epaint `text_layout_types.rs:259`
and `:268`. Letter spacing and line height are precisely what the accessibility
evidence supports, they are per call site, and they need no font change, no
licence question and no reflow. Two evaluations and eight research agents
discriminated between typefaces on an unusable property while these sat unused
in a crate already compiled in.

## The install site must be fixed before any of this

`src/gui/fonts.rs` is the ONLY `ctx.set_fonts` call in the tree
(`src/lib.rs:1420` its only caller). It builds a fresh
`FontDefinitions::default()` INSIDE the per-path loop, inside the
`if let Ok(bytes)` success branch, calls `set_fonts` and returns. Any font
registered elsewhere is discarded, and on a machine where no system emoji font
is found at a hardcoded path, `set_fonts` is never called at all and a new face
silently does not install. Hoist the construction and the `set_fonts` call out
of the loop, register the UI face unconditionally, and append the emoji font
only when one was actually read.

## The decision, as it now stands

**Settled, and free.** Route the eight character-by-character sites to
`FontFamily::Monospace`. Keep Hack as the monospace face, unchanged. Fix the
`fonts.rs` install site. Raise `font_size_small` from 11.902083, which is
currently the size used for the highest-stakes string in the app. Add
`extra_letter_spacing` to the key and seed-phrase rows. Extend the existing
system-font probe in `fonts.rs` with CJK paths so Japanese and Chinese stop
rendering as tofu, for zero redistributed bytes. None of this needs a typeface
decision and all of it outranks one.

**Open, and a genuine fork.** The proportional face:

- **IBM Plex Sans** ships in one session with zero horizontal reflow, and needs
  Ubuntu-Light kept behind it because on its own it loses scripts.
- **Noto Sans** is the correct face for a project whose stated mission is all
  humans: three times the coverage, complete Cyrillic and Greek, no Reserved
  Font Name so the zero can be fixed in place. It costs a global 8.6 percent
  set-width reflow across roughly 340 fixed-width call sites, which is a
  verification burden rather than a risk, and which gets more expensive the
  longer it is deferred.

The trigger is dated and clear: the day this project ships a Cyrillic or Greek
UI, Noto is the answer and the reflow has to be paid anyway.
