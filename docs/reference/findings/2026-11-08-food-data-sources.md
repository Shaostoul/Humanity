# Food data: which ingredient, allergen, additive and food-safety sources the project may ship, and what it must not claim

**Research date: 8 November 2026** (Pacific time). Every source below was read that day by an AI
research agent working for the project, identifying itself as a research tool (a plain user agent
naming HumanityOS and its website). Pages were fetched to be read; GitHub repositories were read
through GitHub's API and Hugging Face models through its public API. Nothing was installed or run,
and no dataset was downloaded. Each source carries its own date where it shows one.

**I am not a lawyer, and this is not legal or medical advice.** It is a reading of public licences,
terms and regulators' pages, written so a later reader can check every word against the same
sources.

Asked for by the operator on 2026-11-08, on reading about label OCR: "Oh, is this how they do those
ingredient list checkers for like making sure food in a store is safe? This is something we want to
consider since we're actually putting all the various ingredients in our app so people can cook in
real life. Might as well let people know all the data, including health risks for all ingredients."
The plan this feeds is [`docs/design/ingredients-and-labels.md`](../../design/ingredients-and-labels.md).

## The question

Which authoritative sources of ingredient, nutrition, allergen, additive and food-safety data may
HumanityOS (a CC0, free, donation-funded project) ship in its release or fetch at runtime, under
what terms, and where is the line between giving people that information and giving medical advice?

## Short answer

- **Ship freely:** USDA FoodData Central (CC0 1.0, a citation requested, F1), USDA FSIS FoodKeeper
  storage data (CC0 on data.gov, F2), and FDA web text such as the allergen and additive pages
  (public domain; FDA asks that a copy carry the date it was copied and a link back, F3).
- **The allergen lists differ by country, so they are data, not code.** United States: nine major
  food allergens since sesame on 1 January 2023 (F4). European Union: fourteen, including celery,
  mustard, lupin, molluscs and sulphites (F10). Canada: its own priority list, including mustard,
  sulphites, molluscs and triticale (F12). Codex: the "big-8" plus sulphites as FAO described it in
  2022, and new precautionary ("may contain") labelling guidelines adopted 7 July 2026 (F11).
- **Legal status is not a health verdict, and it moves.** FDA proposed new GRAS oversight on
  10 August 2026 (F5); it revoked Red No. 3 for food from 15 January 2027 "as a matter of law" while
  saying claims of risk to people "are not supported by the available scientific information" (F6).
  A record must keep a substance's status (per country, dated, with an end date) apart from its
  risks.
- **Open Food Facts: usable, but it is share-alike and it is not for medical use.** The database is
  ODbL 1.0, each item's contents DbCL 1.0, the photos CC BY-SA (3.0 by the link). Anything the project
  derives from it and publishes must be offered under the ODbL with notices, and every screen that
  shows it needs the ODbL notice: the same two obligations `LICENSES.md` already carries for
  OpenStreetMap. Its own terms say the data "must not be used for medical purposes". Its API wants
  one call per real scan, never scraping (F8, F9).
- **Facts only, never text:** EFSA (re-use allowed with acknowledgement, but individual documents may
  carry their own terms, F10), FAO and Codex (non-commercial reproduction only, F11), WHO and JECFA
  (CC BY-NC-SA 3.0 IGO, and database use needs permission, F11). The project cites them as the
  authority and writes the fact in its own words (the `use = "facts"` rule in
  `data/sources/registry.json`). European Commission web content is CC BY 4.0 (F10).
- **Two clear non-US sources:** Health Canada's Canadian Nutrient File 2026 under the Open Government
  Licence, Canada 2.0 (commercial use allowed, an attribution line required, F12), and France's ANSES
  Ciqual table under the Licence Ouverte 2.0 (attribution with the source and its last update date,
  F13).
- **Information, not medical advice.** FDA's own list of software that is not a medical device names
  apps that "Provide information about gluten-free food products or restaurants" and apps that give
  patient-specific reference information for education (F7). Every source read disclaims accuracy.
  So the app may say what a label's words contain and what an authority says about an ingredient,
  with the source and its date; it must never say a food is "safe", "safe for you", "allergen-free"
  or that it found "no allergens" (the last section, "What the project must not claim").
- **OCR engines:** PaddleOCR's PP-OCRv6 models (Apache 2.0, the smallest 1.5M parameters, Chinese,
  English, Japanese and 46 Latin-script languages in one model) and Tesseract (Apache 2.0) are open
  and run on a CPU; Surya's weights are revenue-capped and so not open source; the `ocrs` Rust engine
  reads the Latin alphabet only and its published models are tagged CC BY-SA 4.0 (F14).

## Findings

### F1. USDA FoodData Central: CC0 1.0, with a citation requested

Source: FoodData Central API guide, Licensing section, https://fdc.nal.usda.gov/api-guide (no date
shown); FAQ, https://fdc.nal.usda.gov/faq (no date shown); download page,
https://fdc.nal.usda.gov/download-datasets (latest releases listed as October 2026).

> "USDA FoodData Central data are in the public domain and they are not copyrighted. They are
> published under CC0 1.0 Universal (CC0 1.0)"

> "No permission is needed for their use, but we request that users list FoodData Central as the
> source of the data, and when possible, notify us of the product that uses the data so we may
> better track how FDC is being used in the public domain."

The suggested citation, as printed: "U.S. Department of Agriculture, Agricultural Research Service.
FoodData Central, 2019. fdc.nal.usda.gov." with the note that release numbers and years change.

The download table lists Foundation Foods 10/2026, SR Legacy 04/2018, FNDDS 2021-2023 10/2024 and
Branded 10/2026. The FAQ: "Data in Standard Reference Legacy will not be updated as April 2018 was
the final release." Branded Foods come from industry: "Information in Branded Foods is received from
food industry data providers." (about page, https://fdc.nal.usda.gov/about-us). Foundation Foods now
carry ingredient statements: "Foundation Foods now include, where available, distributor or packer
information, country of origin, organic, and ingredients as listed on the package."

On how far a number can be trusted, the FAQ: "These values are therefore a “snapshot in time,” and
may well change over time."

The API: "FoodData Central currently limits the number of API requests to a default rate of 1,000
requests per hour per IP address", with a key from api.data.gov.

The project already uses this: `data/food/crop_nutrition.ron` and `data/food/crop_micronutrients.json`
were built from the SR Legacy April 2018 release, and say so.

### F2. USDA FSIS FoodKeeper: CC0 on data.gov; the FSIS site itself refuses automated reading

Source: data.gov catalogue record "FSIS - FoodKeeper Data",
https://catalog.data.gov/dataset/fsis-foodkeeper-data (issued 2020-11-10, modified 2025-01-22).

> "FSIS’ FoodKeeper application educates users about food and beverages storage to help them
> maximize the freshness and quality of these items."

The record's licence field is `https://creativecommons.org/publicdomain/zero/1.0/`, and it lists the
data in English, Spanish and Portuguese. Its download links point at fsis.usda.gov, which returned
HTTP 403 to this research (see "Sources I could not reach"); nothing was downloaded.
`data/food_system.ron` already cites FoodKeeper ranges for its spoilage hours.

### F3. FDA web text: public domain, and FDA asks that copies carry their date

Source: FDA Website Policies, https://www.fda.gov/about-fda/about-website/website-policies (content
current as of 01/19/2024).

> "They are in the public domain and may be republished, reprinted and otherwise used freely by
> anyone without the need to obtain permission from FDA."

> "If a person, nonetheless, decides to copy content or images, FDA strongly recommends that the
> copied item lists the date that the material was copied and provides a link back to its source on
> the FDA website."

And the reason, which matters as much as the permission:

> "Providing consumers or health professionals with advice that is not fully up to date can lead to
> serious public health consequences."

### F4. The US major food allergens (FALCPA 2004, FASTER Act 2021)

Source: FDA, Food Allergies, https://www.fda.gov/food/food-labeling-nutrition/food-allergies
(redirects to `/food/nutrition-food-labeling-and-critical-foods/food-allergies`; content current as
of 03/11/2026).

> "The major food allergens are milk, eggs, fish, Crustacean shellfish, tree nuts, peanuts, wheat,
> soybeans, and sesame."

> "On April 23, 2021, the Food Allergy Safety, Treatment, Education, and Research (FASTER) Act was
> signed into law, declaring sesame as the 9th major food allergen recognized by the United States.
> The change was effective on January 1, 2023."

How a label must show them: "In parentheses following the name of the ingredient." or "Immediately
after or next to the list of ingredients in a “contains” statement." The law also requires the type
of tree nut and the species of fish and crustacean shellfish to be named.

On "may contain" and thresholds:

> "Such statements are not required by law"

> "At this time, the FDA has not established a threshold level for any allergens."

Beyond the nine: "More than 160 foods have been identified to cause food allergies in sensitive
individuals." and "Gluten, certain additives (for example, yellow 5, carmine, sulfites), and other
food allergens for which new science has emerged, are examples of other substances the FDA monitors
and, in some cases, requires specific labeling for."

The statute's definition, from FDA's copy of Public Law 108-282, Title II,
https://www.fda.gov/food/food-allergensgluten-free-guidance-documents-regulatory-information/food-allergen-labeling-and-consumer-protection-act-2004-falcpa
(content current as of 11/29/2022), section 201(qq) of the FD&C Act as FALCPA wrote it (before the
FASTER Act added sesame):

> "(1) Milk, egg, fish (e.g., bass, flounder, or cod), Crustacean shellfish (e.g., crab, lobster, or
> shrimp), tree nuts (e.g., almonds, pecans, or walnuts), wheat, peanuts, and soybeans."

and the exception that a matcher has to model, because "soybean oil" is not a soy declaration:

> "Any highly refined oil derived from a food specified in paragraph (1) and any ingredient derived
> from such highly refined oil."

Flavourings are not a hiding place for the nine, in section 403(w)(4) as FALCPA wrote it:

> "Notwithstanding subsection (g), (i), or (k), or any other law, a flavoring, coloring, or
> incidental additive that is, or that bears or contains, a major food allergen shall be subject to
> the labeling requirements of this subsection."

while other allergens in a spice, flavouring, colouring or incidental additive are disclosed only "as
determined by the Secretary by regulation" (section 403(x)).

### F5. US additive status: GRAS, the GRAS notice inventory, Substances Added to Food

Source: FDA, Generally Recognized as Safe (GRAS),
https://www.fda.gov/food/food-ingredients-packaging/generally-recognized-safe-gras (content current
as of 08/10/2026).

> "On August 10, 2026, the FDA proposed a rule to enhance oversight of uses of substances added to
> human and animal food that are claimed to be Generally Recognized as Safe."

(docket FDA-2025-N-3262). What GRAS means: under sections 201(s) and 409 of the Act, "any substance
that is intentionally added to food is a food additive, that is subject to premarket review and
approval by FDA, unless the substance is generally recognized, among qualified experts, as having
been adequately shown to be safe under the conditions of its intended use".

The GRAS Notice Inventory, https://www.fda.gov/food/generally-recognized-safe-gras/gras-notice-inventory
(content current as of 09/26/2018), lists each notice since 1998 with FDA's response letter: "We will
update this information approximately monthly."

Substances Added to Food (formerly EAFUS),
https://www.fda.gov/food/food-additives-petitions/substances-added-food-formerly-eafus (content
current as of 02/13/2025): it covers listed food and colour additives, GRAS substances in 21 CFR 182
and 184, prior-sanctioned substances, and prohibited and delisted ones, and warns:

> "It is important to note that the inventory is only a partial list of food ingredients. Inclusion
> in this inventory of information from non-FDA entities does not indicate an FDA approval or
> evaluation of this use."

The older Food Additive Status List page
(https://www.fda.gov/food/food-additives-petitions/food-additive-status-list, content current as of
07/02/2024) now only sends readers to that inventory.

### F6. Legal status is not a health verdict: FD&C Red No. 3

Source: FDA constituent update, "FDA to Revoke Authorization for the Use of Red No. 3 in Food and
Ingested Drugs",
https://www.fda.gov/food/hfp-constituent-updates/fda-revoke-authorization-use-red-no-3-food-and-ingested-drugs
(content current as of 01/15/2025).

> "The way that FD&C Red No. 3 causes cancer in male rats does not occur in humans."

> "Studies in other animals and in humans did not show these effects; claims that the use of FD&C
> Red No. 3 in food and in ingested drugs puts people at risk are not supported by the available
> scientific information."

> "Manufacturers who use FD&C Red No. 3 in food and ingested drugs will have until January 15, 2027
> or January 18, 2028, respectively, to reformulate their products."

The revocation follows from the Delaney Clause, which bars authorising an additive "found to induce
cancer in humans or animals". FDA's own consumer Q&A,
https://www.fda.gov/food/color-additives-information-consumers/color-additives-questions-and-answers-consumers
(content current as of 12/14/2023), still lists Red No. 3 among "nine certified color additives
approved by the FDA for use in food", which was true when written and stays true until the 2027
date. Two FDA pages, both correct on their dates, read differently today: the record needs dated
status with an end date, not a yes or no.

The same Q&A, on children: "The totality of scientific evidence indicates that most children have no
adverse effects when consuming foods containing color additives, but some evidence suggests that
certain children may be sensitive to them." And on carmine: "Because of potential allergic reactions
in some people, carmine/cochineal extract are required to be identified by name on food labels."

### F7. Where information stops and a medical device begins (US)

Source: FDA, "Examples of Software Functions That Are NOT Medical Devices",
https://www.fda.gov/medical-devices/device-software-functions-including-mobile-medical-applications/examples-software-functions-are-not-medical-devices
(content current as of 09/29/2022).

One category is software for patient education and reference information:

> "These apps can be patient-specific (that is, filters information to patient-specific
> characteristics), but are intended for increased patient awareness, education, and empowerment,
> and ultimately support patient-centered health care."

> "These are not devices because they are intended generally for patient education, and are not
> intended for use in the diagnosis of disease or other conditions, or in the cure, mitigation,
> treatment, or prevention of disease by aiding clinical decision-making"

Its examples include apps that "Provide information about gluten-free food products or
restaurants;". A second category, general wellness, includes apps that "Provide dietary logs,
calorie counters, or make dietary suggestions;" and "Provide meal planners and recipes;".

This is FDA's description, not a ruling on HumanityOS, and it covers the United States only; the EU's
medical device rules were not researched.

### F8. Open Food Facts: its terms

Source: Open Food Facts, "Terms of use, contribution and re-use",
https://world.openfoodfacts.org/terms-of-use (no date shown).

The terms name three licences: the Open Database License for the database, the Database Contents
License for its individual contents, and Creative Commons Attribution ShareAlike for product photos
(the link goes to CC BY-SA 3.0). On the photos:

> "For instance, the Creative Commons Attribution ShareAlike licence for images only covers the
> photo itself and the rights of the contributors who took the picture."

> "Other rights of third parties may apply, such as: copyright for the product design and graphical
> elements it contains (illustrations, pictures etc.), image rights of people (e.g. celebrities)
> featured on the packaging, trademark rights etc."

On accuracy, and the warning that matters most here:

> "Open Food Facts does not guarantee the accuracy of the information and data present on the site
> and in the database"

> "The information and data is provided only for indicative information. It can contain errors and
> must not be used for medical purposes."

On re-use: re-users "have to mention the licence and to attribute the authorship to Open Food Facts
with a link to https://openfoodfacts.org" (or the local version, or the product page), and
"Derivative works must be shared under the same conditions." The English text is a translation: "In
case of discrepancies, the original terms and conditions in French prevail." (the French page was not
read).

From the data page, https://world.openfoodfacts.org/data (no date shown), the API rules:

> "You are very welcome to use the API for production cases, as long as 1 API call = 1 real scan by
> a user."

> "Any attempt to scrape the database using the API will very likely be blocked, as full daily
> exports are available on this very page."

The same page's summary of share-alike goes further than the licence text:

> "Share-Alike: If you publicly use any adapted version of this database, or works produced from an
> adapted database, you must also offer that adapted database under the ODbL and share your
> modifications back."

and its checklist asks apps that let people add products or photos to "please send those
contributions back to Open Food Facts via our API or SDKs". The licence itself (F9, section 4.6)
asks the publisher to offer the adapted database to its own recipients, not to send it to Open Food
Facts; "share your modifications back" is the project's request, which the licence does not
contain. *Judgement: the licence governs; sending contributions back is a courtesy worth doing.*

The ingredient, additive and allergen taxonomies (multilingual name lists) live in the server's
repository, `openfoodfacts/openfoodfacts-server`, `taxonomies/` (read through GitHub's API). GitHub
reports the repository's licence as AGPL-3.0, and its README says Product Opener "is released under
the AGPL license". Neither the taxonomies' README nor the head of `additives.txt` states a licence
for the taxonomy files. Whether they count as the ODbL database or as AGPL code is not stated
anywhere read; either way they are share-alike and cannot go into a CC0 file.

### F9. What the ODbL obliges, in its own words

Source: Open Data Commons Open Database License (ODbL) v1.0,
https://opendatacommons.org/licenses/odbl/1-0/ (version 1.0; no date shown on the page). And the
Database Contents License (DbCL) v1.0, https://opendatacommons.org/licenses/dbcl/1-0/.

What counts as a derivative:

> "For the avoidance of doubt, Extraction or Re-utilisation of the whole or a Substantial part of the
> Contents into a new database is a Derivative Database and must comply with Section 4.4."

and "Substantial" is "substantial in terms of quantity or quality or a combination of both. The
repeated and systematic Extraction or Re-utilisation of insubstantial parts of the Contents may
amount to the Extraction or Re-utilisation of a Substantial part of the Contents."

Share-alike (4.4 a): "Any Derivative Database that You Publicly Use must be only under the terms of:"
this licence, a later similar version, or a compatible licence.

Access (4.6): "If You Publicly Use a Derivative Database or a Produced Work from a Derivative
Database, You must also offer to recipients of the Derivative Database or Produced Work a copy in a
machine readable form of:" the entire derivative database, or a file of all the alterations; "free
of charge if distributed over the internet".

The notice on anything shown (4.3), with the example text the licence accepts:

> "Contains information from DATABASE NAME, which is made available here under the Open Database
> License (ODbL)."

Where share-alike stops (4.5): a Collective Database (the unmodified database beside independent
ones) need not be ODbL, and "Using this Database, a Derivative Database, or this Database as part of
a Collective Database to create a Produced Work does not create a Derivative Database for purposes of
Section 4.4". And "Convey" excludes "interaction with a user through a computer network, or creating
and Using a Produced Work, where no transfer of a copy of the Database or a Derivative Database
occurs."

Each item's contents, under the DbCL (2.1): "The Licensor grants to You a worldwide, royalty-free,
non-exclusive, perpetual, irrevocable copyright license to do any act that is restricted by copyright
over anything within the Contents, whether in the original medium or any other."

**What share-alike would oblige this project to do**, against how `LICENSES.md` already handles
OpenStreetMap's ODbL ("ODbL creates **two separate obligations**"): (1) every screen that shows a
product's data looked up from Open Food Facts is a Produced Work and carries the 4.3 notice there,
not only on a credits page; (2) any file the project publishes that extracts a substantial part of
Open Food Facts (a product subset, an alias list built from its taxonomies) is a Derivative Database:
it must be offered under the ODbL, carry the notices of 4.2, be kept in its own files so the CC0
files beside it stay CC0 (a Collective Database, 4.5 a), and be offered whole, machine-readable and
free (4.6). Looking up one product per scan at runtime and showing it, without the app building and
handing on a copy, transfers no database copy; a cache the person's device keeps for itself is not
public use (4.5 c speaks of use within an organisation; a person's own device is the nearest
reading, which is a judgement).

### F10. The European Union: EFSA, E-numbers, the fourteen allergens

Source: EFSA, Food additives topic, https://www.efsa.europa.eu/en/topics/topic/food-additives (its
newest listed item is dated 23 October 2026).

> "Food and drink labels must identify both the function of the additive in the finished food (e.g.
> colour or preservative) and the specific substance used, either by referring to the appropriate E
> number or its name (e.g. E 415 or Xanthan gum)."

> "The European Commission and EU Member States decide on the approval of food additives on the EU
> market and their conditions of use (such as labelling and maximum permitted limits)."

> "We have so far re-evaluated more than 70% of the 315 food additives that were approved in the EU
> before 20 January 2009."

Its news of 10 September 2026: "EFSA concludes the salt of aspartame-acesulfame (E 962) is safe at
current exposure levels". The authorised list itself is the European Commission's food additives
database, https://food.ec.europa.eu/food-safety/food-improvement-agents/additives/database_en, which
"is based on the Union list of food additives." (Annex II of Regulation (EC) No 1333/2008).

EFSA's food colours page, https://www.efsa.europa.eu/en/topics/topic/food-colours: "The following
table lists food colours authorised for use in the EU and the status of EFSA’s respective safety
assessments." It lists E 102 Tartrazine, "Re-evaluation completed in 2009".

EFSA's legal notice, https://www.efsa.europa.eu/en/legalnotice (no date shown):

> "Copyright © for the entire content of this website unless otherwise stated: European Food Safety
> Authority – EFSA. Re-use is authorised, provided that EFSA is acknowledged as the source of the
> material."

> "For individual documents, the general principle of re-use outlined above may be subject to
> specific conditions as indicated in individual copyright notices contained therein."

> "EFSA accepts no responsibility or liability (including, but not limited to, any direct or
> consequential loss or damage that might occur to you and/or any other third party) arising out of,
> or in connection with the information on this site, including information relating to the
> documents it publishes."

The European Commission's legal notice, https://commission.europa.eu/legal-notice_en ("© European
Union, 1995-2026"): EU-owned content is licensed CC BY 4.0, and "This means that reuse is allowed,
provided appropriate credit is given and changes are indicated." It adds: "Software or documents
covered by industrial property rights, such as patents, trade marks, registered designs, logos and
names, are excluded from the Commission's reuse policy and are not licensed to you."

The fourteen allergens, from the Commission's factsheet "Allergen labelling in the EU: Ensuring
consumer safety" (February 2026, linked from
https://food.ec.europa.eu/food-safety/labelling-and-nutrition/food-information-consumers-legislation_en,
itself CC BY 4.0):

> "There are 14 allergens that must be clearly indicated if they are used as ingredients in food
> products sold in the EU:"

They are cereals containing gluten, crustaceans, eggs, fish, peanuts, soybeans, milk, nuts (almonds,
hazelnuts, walnuts, cashews, pecans, Brazil nuts, pistachios, macadamia or Queensland nuts), celery,
mustard, sesame seeds, sulphur dioxide and sulphites ("when used as preservatives in concentrations
above 10 mg/kg or 10 mg/L"), lupin and molluscs. On "may contain": "However, these statements must
be based on a risk assessment and are not a substitute for good manufacturing practices."

Regulation (EC) No 1333/2008 and Regulation (EU) No 1169/2011 were not read in their own text:
EUR-Lex returned an empty challenge response (see "Sources I could not reach").

### F11. Codex Alimentarius, FAO and WHO

Codex's site, https://codex.fao.org/ (its pages carry "© FAO and WHO, 2026"), links its terms to
FAO's: https://www.fao.org/contact-us/terms/en/ (no date shown).

> "All other content on the FAO website (except where otherwise indicated), may be copied, printed
> and downloaded for private study, research and teaching purposes, and for use in non-commercial
> products or services, provided that appropriate acknowledgement of FAO as the source and copyright
> holder is given and that FAO's endorsement of users' views, products or services is not stated or
> implied in any way."

Translation, adaptation and commercial rights must be requested. And: "Information on this site is
provided on an "as is" and "as available" basis."

The Codex priority allergens, as FAO described them in "Five things science tells us about food
allergens",
https://www.fao.org/newsroom/story/Five-things-science-tells-us-about-food-allergens/en (dated
04/03/2022 as printed):

> "These priority allergens include the “big-8”: milk, egg, fish, crustacean shellfish, tree nuts,
> peanuts, wheat, soybeans, in addition to sulphites (in concentrations of 10 mg/kg or more)."

> "Countries should investigate in their national contexts which pre-packaged foods should be
> labelled and at what quantity food allergens become harmful, taking into account the country’s
> food habits."

The 2026 "may contain" guidelines, FAO news release,
https://www.fao.org/newsroom/detail/global-food-safety-standards-body-codex-adopts-new-guidance-on--may-contain--allergen-labels/en
(07/07/2026):

> "The new Codex guidelines, adopted as an annex to the General Standard for the Labelling of
> Pre-packaged Foods (CXS 1-1985), establish a harmonized, science- and risk-based approach to the
> use of precautionary allergen labelling."

> "However, the use of labels such as "may contain" varies widely across products and countries and
> remains unregulated in many parts of the world."

> "While Codex texts are voluntary, they serve as internationally recognized benchmarks that support
> national legislation and facilitate international trade."

The standard's own text (CXS 1-1985) and the International Numbering System for additives (CXG
36-1989) could not be read (see below), so whether the current Codex list includes sesame was not
confirmed from Codex itself.

WHO publishes JECFA's evaluations of additives,
https://apps.who.int/food-additives-contaminants-jecfa-database/ ("Includes all updates up to the
102nd JECFA meeting (June 2026)."). WHO's copyright page,
https://www.who.int/about/policies/publishing/copyright: its CC BY-NC-SA 3.0 IGO licence "allows
users to freely copy, reproduce, reprint, distribute, translate and adapt the work for non-commercial
purposes, provided WHO is acknowledged as the source", and among the uses that need WHO's
permission: "licensing of materials or other technical information in electronic database products
and services."

### F12. Canada: the Canadian Nutrient File 2026 and the priority allergens

Source: Health Canada, Canadian Nutrient File, about page,
https://www.canada.ca/en/health-canada/services/food-nutrition/healthy-eating/nutrient-data/canadian-nutrient-file-about-us.html
(date modified 2026-06-19):

> "The Canadian Nutrient File (CNF) is Health Canada's bilingual food composition database that
> reports nutrient amounts in foods commonly consumed in Canada. It includes data on 5993 foods."

Its open-data record, "Canadian Nutrient File, 2026",
https://open.canada.ca/data/en/dataset/1b6139bd-ed7e-4043-bc28-ff00e10f3109 (published 2026-05-14,
record modified 2026-10-04): licence "Open Government Licence - Canada".

The licence, https://open.canada.ca/en/open-government-licence-canada ("This is version 2.0 of the
Open Government Licence – Canada."):

> "The Information Provider grants you a worldwide, royalty-free, perpetual, non-exclusive licence to
> use the Information, including for commercial purposes, subject to the terms below."

Where no other attribution is specified, or several providers are combined, the required statement
is "Contains information licensed under the Open Government Licence – Canada." Exempt: personal
information, third-party rights, and the provider's names, crests and logos. And:

> "The Information is licensed “as is”, and the Information Provider excludes all representations,
> warranties, obligations, and liabilities, whether express or implied, to the maximum extent
> permitted by law."

Canada's priority food allergens, Health Canada, Food allergies,
https://www.canada.ca/en/health-canada/services/food-nutrition/food-safety/food-allergies-intolerances/food-allergies.html
(date modified 2026-05-28): "These substances are often referred to as priority food allergens."
The list: eggs, milk, mustard, peanuts, crustaceans and molluscs, fish, sesame seeds, soy, sulphites,
tree nuts, wheat and triticale. (That page's own licence was not checked; the list is used as fact.)

### F13. France: ANSES's Ciqual table, Licence Ouverte 2.0

ANSES, "The Ciqual nutritional composition table",
https://www.anses.fr/en/article/ciqual-nutritional-composition-table (19/11/2025): the table gives
nutrient content "of 3484 foods representative of those consumed in France", and "All the data are
provided via the Ciqual website and available as OpenData".

data.gouv.fr lists "Table de composition nutritionnelle des aliments Ciqual 2020",
https://www.data.gouv.fr/datasets/table-de-composition-nutritionnelle-des-aliments-ciqual-2020, with
"Licence Licence Ouverte / Open Licence" and "Dernière mise à jour 3 juillet 2020". Whether a newer
Ciqual release exists was not settled: ciqual.anses.fr was down for maintenance (HTTP 503) on the
research date.

The Licence Ouverte / Open Licence 2.0 (April 2017,
https://www.etalab.gouv.fr/licence-ouverte-open-licence/, read as its PDF), the one condition:

> "mentionner la paternité de l’ « Information » : sa source (au moins le nom du « Concédant ») et
> la date de dernière mise à jour de l’ « Information » réutilisée."

(credit the source, at least the licensor's name, and the date the information was last updated)
and its compatibility clause names the UK's OGL, CC BY and ODC-BY: "La présente licence a été conçue
pour être compatible avec toute licence libre qui exige au moins la mention de paternité".

### F14. Reading a label on the person's device: OCR engines and models, licences as read

Read through GitHub's API (licence field and README) and Hugging Face's model API on 2026-11-08.

| Engine or model | Licence as reported | Notes from its own README or card |
|---|---|---|
| Tesseract (`tesseract-ocr/tesseract`), its `tessdata_fast` and `tessdata_best` models | Apache-2.0 | The classic CPU engine; language models per language |
| tesseract.js (`naptha/tesseract.js`) | Apache-2.0 | "Pure Javascript OCR for more than 100 Languages" (repository description), runs in a browser |
| PaddleOCR (`PaddlePaddle/PaddleOCR`), PP-OCRv6 (released 2026-06-11 in PaddleOCR 3.7.0) | Apache-2.0 for the code; all 18 PP-OCRv6 model repositories on Hugging Face tagged `license:apache-2.0`, ONNX versions included | "tiny (1.5M) / small (7.7M) / medium (34.5M) for edge, mobile, and server deployment"; "Single model covers Chinese, English, Japanese, and 46 Latin-script languages"; an official browser SDK, PaddleOCR.js, runs PP-OCRv5 |
| RapidOCR (`RapidAI/RapidOCR`) | Apache-2.0 | PaddleOCR's models packaged for ONNX Runtime and others |
| ocrs (`robertknight/ocrs`), on RTen (`robertknight/rten`) | Apache-2.0 for the code (both); the model repository on Hugging Face is tagged `license:cc-by-sa-4.0` | "ocrs currently recognizes the Latin alphabet only (eg. English)." and "ocrs is currently in an early preview. Expect more errors than commercial OCR engines." |
| Surya (`VikParuchuri/surya`) | Apache-2.0 for the code | "The model weights use a modified AI Pubs Open Rail-M license (free for research, personal use, and startups under $5M funding/revenue)." A revenue cap is not an open-source licence |
| Unlimited OCR 2 (`baidu/Unlimited-OCR`) | MIT, code and weights | About 3B parameters; NVIDIA GPUs documented only (the 2026-11-08 AI tools scan) |
| ONNX Runtime (`microsoft/onnxruntime`) | MIT | A runtime for the ONNX models above, native and in a browser |
| zxing-cpp (`zxing-cpp/zxing-cpp`) | Apache-2.0 | Barcode reading |

Two platform facts for the website: MDN's getUserMedia page (last modified Nov 30, 2025) says the
camera is "available only in secure contexts (HTTPS)", and MDN's BarcodeDetector page (last modified
Jun 11, 2026) says "This feature is not Baseline because it does not work in some of the most
widely-used browsers." On the desktop, Windows has its own on-device OCR (`Windows.Media.Ocr`,
introduced in Windows 10 10.0.10240.0, whose engine "Returns a collection of all available OCR
languages on the device.", Microsoft Learn).

The training data behind PP-OCRv6's weights was not checked; the PP-OCRv6 cards were read only for
their licence tags.

## A check the project's own prototype fails

`data/items/foods/` (v0.116 to v0.117, April 2026) holds 33 ingredient rows, three ingredient files
with toxicology and two branded products. No code reads it, but it ships in the data folder. Read
against today's sources:

- `ingredients.csv` says Yellow 5 (Tartrazine) is "banned in some EU contexts". EFSA's colour table
  lists E 102 Tartrazine among "food colours authorised for use in the EU" (F10). **Refuted as
  written.**
- `ingredients/caffeine.ron` cites "FDA caffeine guidance" and "Veterinary Toxicology" as its
  sources: neither is a citation a reader can open, and its child line ("AAP: NO caffeine for
  children under 12") names a body without a document. **Unverifiable as written.**

This is the same failure the species records showed (48 of 60 refuted on review, almost all on the
fields that can hurt someone), and the reason the plan makes a second, adversarial check of every
risk line a gate rather than a hope.

## What is still unknown, and what would settle it

- **Whether Open Food Facts' taxonomies are ODbL or AGPL.** One email to `reuse@openfoodfacts.org`
  (the address its pages give for licence questions) settles it, at no cost. Either answer keeps them
  out of CC0 files.
- **Whether the French original of Open Food Facts' terms differs from the English.** Read
  https://fr.openfoodfacts.org/conditions-d-utilisation, which the English page names as the one in
  force; an hour's careful reading.
- **The Codex texts themselves** (CXS 1-1985 as revised, with the 2026 annex; CXG 36-1989, the INS
  numbers; the GSFA). They sit behind a sign-in or a browser challenge for automated readers. A
  person can open them in a browser at no cost; the facts (INS numbers, the list) are then cited as
  facts.
- **Regulation (EC) No 1333/2008 and Regulation (EU) No 1169/2011 in their own words**, and EUR-Lex's
  re-use notice. A person opens EUR-Lex in a browser; or the Publications Office's other endpoints are
  tried by a later agent.
- **Japan's and China's allergen labelling rules.** The app carries Japanese and Chinese words
  (`data/i18n/ja.json`, `zh.json`), and only secondary reports were seen this session (they say Japan
  made walnut mandatory from 31 March 2025 and cashew from 1 April 2026). The Consumer Affairs Agency's
  notices and China's GB 7718 must be read before either regime is shown; a research task of a few
  hours.
- **Whether the EU's medical device rules reach a label reader.** Not researched; FDA's view (F7) is
  US only. A finding on the EU MDR's rule 11 and its guidance would settle the EU reading.
- **Who may be named as a reviewer of health lines.** Not a legal question but a gate: see the plan.
- **Whether a newer Ciqual than 2020 exists, and the Canada.ca page licence** for the allergen list.
  Both cheap re-checks.

## Sources I could not reach

- https://www.fsis.usda.gov/food-safety/safe-food-handling-and-preparation/food-safety-basics/foodkeeper-app
  and https://www.foodsafety.gov/keep-food-safe/foodkeeper-app: HTTP 403. Hoped for FoodKeeper's own
  description and terms; the data.gov record (F2) stood in. Not worked around.
- https://ods.od.nih.gov/factsheets/list-all/: HTTP 403, a Cloudflare block page ("Sorry, you have
  been blocked"). Hoped for the Office of Dietary Supplements' upper limits for nutrients. Not worked
  around.
- https://eur-lex.europa.eu/content/legal-notice/legal-notice.html: HTTP 202 with an empty body (a
  challenge). Hoped for EUR-Lex's re-use terms and the regulations' text.
- https://www.fao.org/gsfaonline/index.html: HTTP 403, "Just a moment..." (a browser challenge).
  Hoped for the Codex General Standard for Food Additives.
- https://workspace.fao.org/sites/codex/Standards/CXS%201-1985/CXS_001e.pdf and the CXG 36-1989 PDF:
  both redirect to a Microsoft sign-in page. Hoped for the labelling standard and the INS list.
- https://ciqual.anses.fr/: HTTP 503, maintenance. Hoped for the current Ciqual release and its date.
- https://www.food.gov.uk/safety-hygiene/food-additives and the FSA allergen pages: redirected to
  gov.uk publication listings rather than the advice itself. Hoped for the UK's additive warnings.
- https://www.developmentaid.org/ (a syndicated copy of the FAO release): HTTP 403; the FAO original
  was read instead.

## What it means for this project

### Certain

- FoodData Central (F1), FoodKeeper (F2) and FDA's web text (F3) may ship inside the release, the
  CC0 bundle included. FDC asks for a citation; FDA asks that copied text carry its date and a link.
  Both already fit the project's habit of dating every source.
- The allergen classes are law in each place and they differ (F4, F10, F12, F11). Nine in the United
  States, fourteen in the European Union, Canada's own list. A matcher built on one list misses
  celery, mustard, lupin or molluscs for a European reader, and mustard or sulphites for a Canadian.
- "May contain" statements are voluntary in the US (F4), must rest on a risk assessment in the EU
  (F10), and are "unregulated in many parts of the world" (F11). Their absence proves nothing.
- FDA has set no allergen threshold (F4). The US exempts highly refined oils from the definition
  (F4). Labels exempt from FALCPA include raw agricultural commodities and most unpackaged food sold
  at retail or food service (F4's source).
- Regulatory status changes on dates (the GRAS proposal of 10 August 2026, Red No. 3 for food until
  15 January 2027) and is not a statement of health risk (F5, F6).
- Open Food Facts data is ODbL with DbCL contents and CC BY-SA photos, carries third-party rights in
  the photos, says it "must not be used for medical purposes", and asks for one API call per real
  scan (F8). Publishing anything substantially extracted from it obliges the project to offer that
  under the ODbL (F9).
- EFSA re-use needs acknowledgement (F10); FAO and Codex content is non-commercial unless permission
  is granted (F11); WHO's is CC BY-NC-SA and database use needs permission (F11). None of these
  texts can go into the CC0 bundle as text.
- The Canadian Nutrient File 2026 (OGL-Canada 2.0) and Ciqual (Licence Ouverte 2.0) may be
  redistributed, including commercially, with their attribution (F12, F13).
- Surya's weights are not open source (F14); ocrs's models carry a share-alike tag (F14).
- The prototype's Yellow 5 line is contradicted by EFSA's list (section above).

### Judgement calls

- **Open Food Facts belongs at runtime and opt-in, never in the bundle.** One lookup per scan, on the
  person's click, shown with the ODbL notice on that screen. That respects its API rule, keeps the
  CC0 data CC0, and tells the person plainly that the barcode leaves their device. Contributing
  corrections back is a courtesy worth building, not an obligation.
- **EFSA, FAO, Codex and WHO are `use = "facts"`.** EFSA's site permits re-use with acknowledgement,
  but its documents may carry their own terms, so the safe course is the registry's: cite the body,
  restate the fact (an ADI, an E-number's status) in the project's words.
- **The label reader is information, not a device, in the US** when it says what the words on a
  label contain and what an authority says, cites both, and never tells a person what to eat or that
  a food is safe for them. That is a reading of FDA's examples (F7), not a ruling.
- **The two-obligation shape of `LICENSES.md`** is the right template if any Open Food Facts derived
  file is ever published: the notice where it is shown, and the file offered under the ODbL in its
  own folder.
- **PP-OCRv6 tiny or small, run through ONNX Runtime, is the best-fitting open OCR model for labels**
  in every language the app carries; Tesseract is the proven fallback. A decision for the plan, not a
  licence fact.

## What the project must not claim, in plain terms

Every source read disclaims what a careless app would promise: Open Food Facts ("must not be used for
medical purposes"), EFSA ("accepts no responsibility or liability"), FAO ("as is" and "as
available"), the Canadian licence ("as is"), and FDA, which warns that advice "not fully up to date
can lead to serious public health consequences". The project's own warning finding
([`2026-10-30-wip-warning-and-liability.md`](2026-10-30-wip-warning-and-liability.md)) adds that no
notice makes anyone immune. So the app never says, on any screen, in any language:

- that a food or product is **"safe"**, **"safe for you"**, **"allergen-free"**, **"gluten-free"**
  (a term FDA defines for labels) or **"OK to eat"**;
- that it found **"no allergens"**: only that it did not find a named allergen **in the words it
  could read**, with the reasons that can be wrong (a misread, an incomplete label, a changed recipe,
  cross-contact, "may contain" rules that differ by country);
- anything that **diagnoses, treats or prevents** a condition, sets a dose for a person, or tells a
  person to stop or change a medicine;
- that a substance is **dangerous because a regulator banned it**, or **harmless because it is
  permitted**: status and risk are reported separately, each with its source and date;
- a risk line **without its source, the source's date, the date it was read and the review it
  passed**.

What it may say: what the label's words are, what each recognised ingredient is, which allergen
classes the law in each place assigns it, what a named authority says about it (with who is at risk
and at what amount, quoted or restated, with the link and the date), and that the person should read
the label themselves and ask a doctor, pharmacist or dietitian about their own health.

**I am not a lawyer, and this is not legal or medical advice.** A lawyer should read the Open Food
Facts reading (F8, F9) before any Open Food Facts derived file is published, and the EU device
question should be researched before the label reader is offered there as a feature.
