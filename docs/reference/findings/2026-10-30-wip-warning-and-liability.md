# An unfinished-software warning: what the licence already says, what a warning can and cannot do, and its words

**Researched 2026-10-30** (sources read on 30 October 2026, Pacific time). Every source below was
downloaded that day and its words copied from the download, not from memory.

**This is a reading of public sources by an AI working on the project. It is not legal advice, and
nobody who wrote it is a lawyer.** A lawyer should read this, and the warning's words, before
version 1.0, and before the project ever sells anything, charges for a service or forms a business.

## The question

Before version 1.0, what protection does HumanityOS already have against blame for harm its
unfinished software causes, what can a warning anchored to the bottom of every page add to it, what
can no warning do, and what should the warning say?

## Short answer

- **The licence already disclaims warranties, but it does not limit liability.** The project is
  CC0 1.0: the person releasing it "offers the Work as-is and makes no representations or
  warranties of any kind", "to the greatest extent permissible under applicable law" (finding 1).
  CC0 has no clause limiting liability for damages; the only liability words in it are Creative
  Commons disclaiming its own. The AGPL and CC BY-SA, under discussion in the licence vote, both
  have such a clause, and both say it gives way where the law requires (findings 2 and 3).
- **A warning can make the risk obvious, and that counts.** In each body of law read, what a
  product's maker tells people bears on whether the product was defective or unsafe: the EU
  directive weighs "the presentation" and "the instructions" (finding 6), Washington's product
  liability act asks whether "adequate warnings or instructions" were given (finding 10), and
  Washington's sales law treats words like "as is" as excluding implied warranties where that law
  applies (finding 8).
- **A warning cannot make anyone immune.** Where the UK statutes apply, nobody can exclude
  liability "for death or personal injury resulting from negligence", by contract or by "a notice
  given to persons generally", and knowing about the notice is not accepting the risk (findings 4
  and 5). The EU directive forbids limiting its liability by any contractual provision, and the
  UK's product liability act by any contract term or notice (findings 6 and 7). No statute read
  lets a notice excuse carelessness.
- **Several of those rules may not reach a free, open-source, donation-funded project at all,**
  and the texts do not settle it. The EU directive does not apply to free and open-source software
  "developed or supplied outside the course of a commercial activity" (finding 6); the UK rules
  bind a "business" or a "trader" (findings 4, 5, 7); Washington's sales warranties arise on a
  sale "for a price" and its product act covers sellers "engaged in the business of selling
  products" (findings 8 to 10). Ordinary negligence law is not limited that way.
- **So the warning is worth having, and is not a shield.** It is honest, it says plainly what the
  licence says, and it tells people where the danger is. What actually protects people, and the
  project, is not causing the harm: fixing safety errors fast, keeping warnings next to the risky
  content, and a lawyer's reading before 1.0.

## Findings

### 1. The project's licence: CC0 1.0 disclaims warranties and says nothing on liability

- Sources: the repository's `LICENSE` file (read 2026-10-30) and
  https://creativecommons.org/publicdomain/zero/1.0/legalcode.en (downloaded 2026-10-30; the page
  carries no date of its own, and its text is version 1.0). The words quoted below are identical in
  both.
- Section 4(b): "Affirmer offers the Work as-is and makes no representations or warranties of any
  kind concerning the Work, express, implied, statutory or otherwise, including without limitation
  warranties of title, merchantability, fitness for a particular purpose, non infringement, or the
  absence of latent or other defects, accuracy, or the present or absence of errors, whether or not
  discoverable, all to the greatest extent permissible under applicable law."
- The only liability words in the document belong to Creative Commons, about itself, in the notice
  above the legal code: "CREATIVE COMMONS MAKES NO WARRANTIES REGARDING THE USE OF THIS DOCUMENT OR
  THE INFORMATION OR WORKS PROVIDED HEREUNDER, AND DISCLAIMS LIABILITY FOR DAMAGES RESULTING FROM
  THE USE OF THIS DOCUMENT OR THE INFORMATION OR WORKS PROVIDED HEREUNDER." Section 4(d) adds that
  "Creative Commons is not a party to this document".
- Read whole, CC0 has four sections (the rights, the waiver, the fallback licence, the limitations
  and disclaimers). None limits the Affirmer's liability for damages. A search of both copies for
  "liab" and "damages" finds only the Creative Commons notice.
- The website's footer today says "Public domain" and links CC0 1.0 (`web/shared/shell.js`); the
  desktop app shows nothing about warranty. Nothing a person sees in either app says "as is".

### 2. The AGPL, under discussion for the code: a disclaimer AND a limit, both bounded by law

- Source: https://www.gnu.org/licenses/agpl-3.0.en.html ("Version 3, 19 November 2007"; downloaded
  2026-10-30). Context: `2026-10-25-licence-options.md`, the community vote on the licence.
- Section 15: "THERE IS NO WARRANTY FOR THE PROGRAM, TO THE EXTENT PERMITTED BY APPLICABLE LAW."
- Section 16: "IN NO EVENT UNLESS REQUIRED BY APPLICABLE LAW OR AGREED TO IN WRITING WILL ANY
  COPYRIGHT HOLDER, OR ANY OTHER PARTY WHO MODIFIES AND/OR CONVEYS THE PROGRAM AS PERMITTED ABOVE,
  BE LIABLE TO YOU FOR DAMAGES".
- Section 17: "If the disclaimer of warranty and limitation of liability provided above cannot be
  given local legal effect according to their terms, reviewing courts shall apply local law that
  most closely approximates an absolute waiver of all civil liability in connection with the
  Program".

### 3. CC BY-SA 4.0, under discussion for the Library: the same shape, and it says it may fail

- Source: https://creativecommons.org/licenses/by-sa/4.0/legalcode.en (version 4.0; the page
  carries no date of its own; downloaded 2026-10-30).
- Section 5: "To the extent possible, in no event will the Licensor be liable to You on any legal
  theory (including, without limitation, negligence) or otherwise for any direct, special,
  indirect, incidental, consequential, punitive, exemplary, or other losses, costs, expenses, or
  damages arising out of this Public License or use of the Licensed Material". And: "Where a
  limitation of liability is not allowed in full or in part, this limitation may not apply to You."

### 4. UK: a notice cannot exclude liability for death or injury from negligence, for a business

- Source: Unfair Contract Terms Act 1977, sections 1 and 2, the revised text on legislation.gov.uk:
  https://www.legislation.gov.uk/ukpga/1977/50/section/1 and /section/2 (the Act commenced 1
  February 1978; section 2(4) was inserted by the Consumer Rights Act 2015 from 1 October 2015;
  legislation.gov.uk's record of the revised text is dated 17 May 2024; downloaded 2026-10-30).
- Section 2(1): "A person cannot by reference to any contract term or to a notice given to persons
  generally or to particular persons exclude or restrict his liability for death or personal injury
  resulting from negligence."
- Section 2(2): for "other loss or damage", only "in so far as the term or notice satisfies the
  requirement of reasonableness".
- Section 2(3): "a person's agreement to or awareness of it is not of itself to be taken as
  indicating his voluntary acceptance of any risk."
- Section 1(3), the limit on all of it: "sections 2 to 7 apply (except where the contrary is stated
  in section 6(4)) only to business liability, that is liability for breach of obligations or
  duties arising— (a) from things done or to be done by a person in the course of a business
  (whether his own business or another's)".
- Section 2(4): the section "does not apply to" a consumer contract or "a notice to the extent that
  it is a consumer notice", which the Consumer Rights Act 2015 covers instead (finding 5).

### 5. UK: the same rule for a trader's notice to consumers, and free digital content

- Source: Consumer Rights Act 2015, sections 2, 33 and 65:
  https://www.legislation.gov.uk/ukpga/2015/15/section/2, /section/33, /section/65 (in force 1
  October 2015; legislation.gov.uk's record dated 29 October 2026; downloaded 2026-10-30).
- Section 65(1): "A trader cannot by a term of a consumer contract or by a consumer notice exclude
  or restrict liability for death or personal injury resulting from negligence." Section 65(2):
  "a person is not to be taken to have voluntarily accepted any risk merely because the person
  agreed to or knew about the term or notice."
- Section 2(2): "“Trader” means a person acting for purposes relating to that person's trade,
  business, craft or profession".
- Section 33(1), the Act's rights about the quality of digital content: "This Chapter applies to a
  contract for a trader to supply digital content to a consumer, if it is supplied or to be
  supplied for a price paid by the consumer." Section 33(2) adds content "supplied free with goods
  or services or other digital content for which the consumer pays a price".

### 6. EU: the new Product Liability Directive covers software, cannot be contracted out of, and leaves out non-commercial free software

- Source: Directive (EU) 2024/2853 "of 23 October 2024 on liability for defective products and
  repealing Council Directive 85/374/EEC", Official Journal L series, 18.11.2024. Read from the
  Publications Office's repository, http://publications.europa.eu/resource/celex/32024L2853 (the
  Official Journal's own file, L_202402853EN.000101.fmx.xml, downloaded 2026-10-30; EUR-Lex itself
  refused the download, see Sources not reached).
- Article 4(1): "‘product’ means all movables, even if integrated into, or inter-connected with,
  another movable or an immovable; it includes electricity, digital manufacturing files, raw
  materials and software;"
- Article 2(1) and 2(2): "This Directive shall apply to products placed on the market or put into
  service after 9 December 2026." "This Directive does not apply to free and open-source software
  that is developed or supplied outside the course of a commercial activity."
- Recital 14: "In principle, the supply of free and open-source software by non-profit
  organisations should not be considered as taking place in a business-related context, unless
  such supply occurs in the course of a commercial activity. However, where software is supplied in
  exchange for a price, or for personal data used other than exclusively for improving the
  security, compatibility or interoperability of the software, and is therefore supplied in the
  course of a commercial activity, this Directive should apply."
- Article 15: "Member States shall ensure that the liability of an economic operator pursuant to
  this Directive is not, in relation to the injured person, limited or excluded by a contractual
  provision or by national law."
- Article 7(1) and 7(2)(a), where a warning counts: "A product shall be considered defective where
  it does not provide the safety that a person is entitled to expect or that is required under
  Union or national law." Among the circumstances weighed: "the presentation and the
  characteristics of the product, including its labelling, design, technical features, composition
  and packaging and the instructions for its assembly, installation, use and maintenance;".
- Article 6(1)(a) and (c), the harm covered: "death or personal injury, including medically
  recognised damage to psychological health;" and "destruction or corruption of data that are not
  used for professional purposes."
- Article 22(1): Member States must bring their laws into force "by 9 December 2026".
- The directive says nothing about donations. A different EU law of the same day does. Regulation
  (EU) 2024/2847, the Cyber Resilience Act (Official Journal L, 20.11.2024; read from
  http://publications.europa.eu/resource/celex/32024R2847, downloaded 2026-10-30), recital 15,
  lists among the signs of commercial activity "accepting donations exceeding the costs associated
  with the design, development and provision of a product with digital elements", and says:
  "Accepting donations without the intention of making a profit should not be considered to be a
  commercial activity." Whether a court would read the Product Liability Directive's "commercial
  activity" the same way is not answered by either text.

### 7. UK: product liability cannot be excluded either, with a defence for non-business supply

- Source: Consumer Protection Act 1987, sections 4 and 7:
  https://www.legislation.gov.uk/ukpga/1987/43/section/7 and /section/4 (Act of 15 May 1987;
  section 4(1)(a) amended from 1 January 2024; downloaded 2026-10-30).
- Section 7: "The liability of a person by virtue of this Part to a person who has suffered damage
  caused wholly or partly by a defect in a product, or to a dependant or relative of such a person,
  shall not be limited or excluded by any contract term, by any notice or by any other provision."
- Section 4(1)(c), a defence: "that the only supply of the product to another by the person
  proceeded against was otherwise than in the course of a business of that person’s; and ... that
  section 2(2) above does not apply to that person or applies to him by virtue only of things done
  otherwise than with a view to profit".
- Whether software is a "product" under this Act was not read today (see What is still unknown).

### 8. Washington: an "as is" disclaimer works where the sales law applies, which needs a sale for a price

- Sources: the Revised Code of Washington on the Legislature's site, RCW 62A.2-316, 62A.2-314,
  62A.2-102, 62A.2-105 and 62A.2-106, https://app.leg.wa.gov/RCW/default.aspx?cite=62A.2-316 (and
  the same address with each section's number). Each page's history line dates the section:
  2-316 last amended 2013 (2013 c 23 s 134), 2-314 and 2-105 from 1965, 2-102 and 2-106 last
  amended 2023 (2023 c 266). Downloaded 2026-10-30.
- RCW 62A.2-316(3)(a): 'Unless the circumstances indicate otherwise, all implied warranties are
  excluded by expressions like "as is," "with all faults," or other language which in common
  understanding calls the buyer's attention to the exclusion of warranties and makes plain that
  there is no implied warranty;'. Section 2-316(2) asks that a written exclusion of
  merchantability "must be conspicuous".
- RCW 62A.2-316(4), the consumer exception: for goods "purchased primarily for personal, family, or
  household use", "disclaimers of the warranty of merchantability or fitness for particular purpose
  shall not be effective to limit the liability of merchant sellers except insofar as the
  disclaimer sets forth with particularity the qualities and characteristics which are not being
  warranted."
- When the warranty arises at all, RCW 62A.2-314(1): "a warranty that the goods shall be
  merchantable is implied in a contract for their sale if the seller is a merchant with respect to
  goods of that kind." RCW 62A.2-102(1): the Article "applies to transactions in goods". RCW
  62A.2-106(1): 'A "sale" consists in the passing of title from the seller to the buyer for a
  price (RCW 62A.2-401).' RCW 62A.2-105(1): '"Goods" means all things (including specially
  manufactured goods) which are movable at the time of identification to the contract for sale'.

### 9. Washington: limiting damages for injury to the person is presumed unconscionable for household goods

- Source: RCW 62A.2-719, https://app.leg.wa.gov/RCW/default.aspx?cite=62A.2-719 (last amended
  1974; downloaded 2026-10-30).
- Section 2-719(3): "Limitation of consequential damages for injury to the person in the case of
  goods purchased primarily for personal, family or household use or of any services related
  thereto is invalid unless it is proved that the limitation is not unconscionable."

### 10. Washington: the product liability act asks whether adequate warnings were given

- Sources: RCW 7.72.010 and 7.72.030, https://app.leg.wa.gov/RCW/default.aspx?cite=7.72.010 and
  cite=7.72.030 (7.72.010 last amended 1991, 7.72.030 in 1988; downloaded 2026-10-30).
- RCW 7.72.030(1): "A product manufacturer is subject to liability to a claimant if the claimant's
  harm was proximately caused by the negligence of the manufacturer in that the product was not
  reasonably safe as designed or not reasonably safe because adequate warnings or instructions were
  not provided." Section (1)(c) adds a duty after manufacture "where a manufacturer learned or where
  a reasonably prudent manufacturer should have learned about a danger connected with the product",
  satisfied "if the manufacturer exercises reasonable care to inform product users".
- Who it covers, RCW 7.72.010(1): '"Product seller" means any person or entity that is engaged in
  the business of selling products'. 7.72.010(2): '"Manufacturer" includes a product seller who
  designs, produces, makes, fabricates, constructs, or remanufactures the relevant product'.
  7.72.010(3): '"Product" means any object possessing intrinsic value, capable of delivery either
  as an assembled whole or as a component part or parts, and produced for introduction into trade
  or commerce.'
- 7.72.010(5): "A claim may be asserted under this chapter even though the claimant did not buy the
  product from, or enter into any contractual relationship with, the product seller." So a claim
  here does not depend on a sale to the person harmed.

## What is still unknown, and what would settle it

- **Whether a donation-funded, volunteer, open-source project is a "business", a "trader", in "the
  course of a commercial activity", or "engaged in the business of selling products".** Each
  regime above turns on it and none of the texts read answers it for this project. A lawyer in each
  place settles it, at the cost of a consultation; the EU question may also be answered by guidance
  under the directive or the Cyber Resilience Act, not read today.
- **Whether software is "goods" under Washington's sales law, or a "product" under its product
  liability act or the UK's 1987 Act.** That is case law, not read today. Settled by a Washington
  lawyer, or by reading the state appellate decisions on software as goods.
- **Whether a court enforces a notice that limits liability for ordinary negligence when no
  statute above applies** (in Washington, the state's case law on exculpatory clauses, including
  the rule usually reported that such a clause cannot cover gross negligence; not read today). This
  is what decides how much the details page's "to the extent the law allows" sentence can do.
  Settled by a Washington lawyer.
- **How each EU Member State transposed the directive** by its 9 December 2026 deadline. Settled
  by reading the national laws listed against the directive on EUR-Lex after that date.
- **Whether an in-app notice is shown clearly enough to count**, where a notice must be brought to
  a person's attention. Settled by a lawyer reading the finished screen.

## Sources not reached

- https://eur-lex.europa.eu/legal-content/EN/TXT/HTML/?uri=CELEX:32024L2853 and
  https://eur-lex.europa.eu/eli/dir/2024/2853/oj/eng: refused the download with an automated-client
  challenge (HTTP 202, "x-amzn-waf-action: challenge", no body). The same Official Journal text was
  read from the Publications Office's repository instead (finding 6), so nothing was lost, but a
  later reader using EUR-Lex should expect the same.
- http://data.europa.eu/eli/dir/2024/2853/oj: returned an empty body to the same request.
- Washington case law (findings above): not looked for today; no court site was tried.

## What it means for this project

**Certain (from the sources):**
- Today the only text about the software's own warranty or liability found in the project is
  CC0's warranty disclaimer (a search of `web/pages/`, `docs/user/`, `docs/accord/` and the README
  for "warranty", "liable" and "liability" found the words only about other things: battery and
  car warranties, and Good Samaritan law in the first-aid guide). It has no liability limit, and
  neither app tells a person the software comes as is.
- Where the UK's rules apply, no notice can exclude liability for death or personal injury caused
  by negligence, and a person knowing about the notice does not change that.
- From 9 December 2026, the EU directive covers software, and where it applies, its liability
  cannot be limited by any contract term. It does not apply to free and open-source software
  supplied outside a commercial activity; supplying it for a price or for personal data beyond the
  listed purposes is commercial.
- In every regime read, the warnings and instructions given with a product bear on whether it was
  defective or reasonably safe. Washington's act adds a duty to warn about a danger learned after
  the product went out.
- A licence change (the vote) would add a limitation-of-liability clause, and that clause says it
  yields to the law.

**Judgement calls (an AI's reading; a lawyer could disagree with any of it):**
- The warning is worth building now. It costs nothing, it is the plain truth, it makes the
  licence's "as is" conspicuous, and the duty-to-warn rules reward exactly this. It does not make
  the operator or anyone else immune, and the wording below never claims it does.
- Saying "we cannot be held liable for any harm" would be untrue in some places and could read as
  misleading, so the warning says what the software is and what not to rely on it for, and the
  details page says "to the extent the law allows".
- The strongest protection is the same as the strongest kindness: do not teach unsafe things.
  BUG-216 (the game's recipes taught unsafe food handling, found 2026-10-30) is the kind of harm
  a warning does not cure; fixing it does.
- Keeping the project non-commercial (no sale, no price, no personal data taken as payment)
  keeps it outside several of these regimes on their plain words. Any step toward selling,
  charging, advertising or a business entity is a moment to ask a lawyer first.

## The warning's words

Plain words, true to the findings above, in `data/i18n/en.json` under `wip.`:

- **The line** (bottom of every app page, every website page):
  "Unfinished software: it can be wrong, break, or lose your work. Do not rely on it for health,
  safety, food or water. It comes as is, with no [[warranty]]."
- **The link**: "What this means", opening the Library page
  `docs/user/rights/unfinished_software.md`.
- **The version**, beside it: "Version 0.1547" (the running version), so "unfinished" has a number
  and a reader can see it end at 1.0.
- **The details page** says: what unfinished means here (changes every day, mistakes found and
  fixed in the open, saves that may not carry over before 1.0); what not to rely on it for (health
  and medicine, food safety and wild food, drinking water, fire, electricity, gas and building
  safety, money, your only copy of anything); what "as is, with no warranty" means; one sentence
  limiting responsibility "to the extent the law allows", with the plain note that in some places
  the law does not let anyone exclude responsibility for death or injury caused by carelessness,
  and nothing on the page tries to; when the warning goes away (version 1.0); how to report a
  problem; and the emergency line: in danger, call the local emergency number.

Words to avoid on any screen: "safe", "guaranteed", "certified", "medical advice", and "we are not
liable for anything".

**Again: this is not legal advice, and nobody who wrote it is a lawyer.** A lawyer should read it
before version 1.0.
