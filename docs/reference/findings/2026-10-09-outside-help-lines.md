# Where to send someone outside the app when a child, or anyone, is in danger

**Research date: 9 October 2026** (pages read between about 21:30 and 22:10
Pacific time, which is 04:30 to 05:10 UTC on 10 October 2026). Everything
below was read on that day. If you are reading this later, check each source's
own date against anything that has changed since: hotlines move, rename and
change their report forms.

This is a reading of public sources by an AI working on the project. It is not
legal advice, and nobody who wrote it is a lawyer or a child-protection
professional.

## The question

When someone in HumanityOS reports that a child may be in danger, or that
someone is in danger, what should the report dialog show them before the report
goes to the server's volunteer admins: the emergency number of their country,
and the official place in their country to report a child being sexually
exploited or abused online? (Design: `docs/design/blocking-and-safe-mode.md`
sections 8.3 and 10e.)

## Short answer

All 18 countries asked about have an emergency number confirmed from an official
source, and all 18 have a body that takes reports of online child sexual
exploitation or abuse, confirmed from that body's own site. The results are in
`data/safety/outside_help.json`, one entry per country, and every entry in that
file is backed by a section below.

The bodies are not all the same kind of thing, and the app should not pretend
they are:

- **Police or government:** Australia (ACCCE, led by the Australian Federal
  Police), United Kingdom (CEOP, a law enforcement agency), New Zealand
  (Department of Internal Affairs), France (PHAROS, Ministry of the Interior),
  Italy (Polizia Postale), India (National Cyber Crime Reporting Portal,
  Ministry of Home Affairs), Germany (jugendschutz.net, the joint competence
  centre of the federal government and the states, acting under a legal
  mandate), Spain (INCIBE, a company dependent on a government ministry),
  Japan (Internet Hotline Center, run on commission from the National Police
  Agency), South Africa (Film and Publication Board, which calls itself the
  content regulatory authority).
- **Hotlines that their own pages do not describe as part of government**,
  each a member of INHOPE (the international network of these hotlines):
  United States (NCMEC's CyberTipline), Ireland (Hotline.ie), Netherlands
  (Offlimits), Mexico (Te Protejo México), Brazil (SaferNet Brasil),
  Philippines (ECPAT Philippines, working with a government cybercrime
  centre), Nigeria (ACSAI, a registered NGO). Canada's Cybertip.ca is run by
  the Canadian Centre for Child Protection and is named by the Government of
  Canada and the RCMP as the national tipline; it is **not** in INHOPE's
  member directory as read on this date.

On the pages read, 10 of the 18 bodies tell a person to call the emergency
number first if someone is in immediate danger (United States, Canada, United
Kingdom, Ireland, Australia, New Zealand, France, Italy, Netherlands, Japan);
the pages read for the other eight did not say it, which does not mean they
would disagree. Most of these services are for reporting images, videos and
online contact, not for sending help; two say plainly that a report to them is
not a formal complaint to the police (Spain, Italy). Hours vary: Cybertip.ca,
for example, is staffed on weekdays only.

For any other country, the fallback is INHOPE's member directory at
https://www.inhope.org/EN/our-members, together with the plain instruction to
call the local emergency number. No single number works everywhere, and this
research did not establish one.

## Summary table

| Country | Emergency number | Body for online child sexual exploitation | Kind |
|---|---|---|---|
| United States (US) | 911 | NCMEC CyberTipline | INHOPE member |
| Canada (CA) | 911 (written 9-1-1) | Cybertip.ca | Named by Government of Canada and RCMP; not in INHOPE directory |
| United Kingdom (GB) | 999 | CEOP Safety Centre (also IWF for images) | Law enforcement (IWF: charity, INHOPE member) |
| Ireland (IE) | 112 or 999 | Hotline.ie (Irish Internet Hotline) | INHOPE member |
| Australia (AU) | 000 (112 from a mobile) | ACCCE | AFP-led (eSafety is the INHOPE member) |
| New Zealand (NZ) | 111 | Department of Internal Affairs, Digital Safety Group | Government (Netsafe is the INHOPE member) |
| Germany (DE) | 112; police 110 | jugendschutz.net | Federal and state competence centre, INHOPE member |
| France (FR) | 112; police 17; child in danger 119 | PHAROS | Ministry of the Interior (Point de Contact is the INHOPE member) |
| Spain (ES) | 112 | INCIBE child sexual abuse material reporting line | INHOPE member |
| Italy (IT) | 112 or 113 | Polizia Postale, Segnala online | State Police (INHOPE members: Save the Children Italy, Telefono Azzurro) |
| Netherlands (NL) | 112 | Offlimits Meldpunt | INHOPE member |
| Mexico (MX) | 911 | Te Protejo México | Foundation's platform, INHOPE member |
| Brazil (BR) | 190 (police) | SaferNet Brasil | INHOPE member (government line: Disque 100) |
| India (IN) | 112 | National Cyber Crime Reporting Portal | Ministry of Home Affairs |
| Philippines (PH) | 911 | ECPAT Philippines, eProtectKids hotline | INHOPE member, with DICT-CICC |
| Japan (JP) | 110 (police) | Internet Hotline Center Japan | Commissioned by the National Police Agency, INHOPE member |
| South Africa (ZA) | 10111 (police) | Film and Publication Board Hotline | Content regulator, INHOPE member |
| Nigeria (NG) | 112 (where the state centre runs) | ACSAI | Registered NGO, INHOPE member since 2024 |

## How each finding is recorded

For each country: the number or body, the page it comes from, that page's own
date where it shows one, and a short quotation. Every quotation and quoted date
below was checked by script against the live page on the research date, with
HTML tags removed and spaces collapsed (99 checks, all matched), except two
read by hand from raw source: the commented-out notice on the CEOP page and the
PHAROS footer logo title, which is in the application's JavaScript. Where a page shows no date, that
is said. Where the only date available is the server's `Last-Modified` header
and it falls within hours of the research, it is almost certainly the time the
page was generated rather than edited, and is reported as no date.

---

## International fallback: INHOPE's member directory

- Source: https://www.inhope.org/EN/our-members (no date on the page).
- INHOPE describes itself on that page as "The global network fighting child
  sexual abuse material online."
- The directory's own instruction: "If you encounter suspected child sexual
  abuse material, make an anonymous report. Find your local hotline via the
  drop-down or map."
- What the directory showed for the 18 countries (each hotline's own site is
  quoted in its country's section):

  | Country | Listed hotline(s) | Listed report link |
  |---|---|---|
  | United States | Cybertipline (joined 1999) | https://report.cybertip.org/ |
  | Canada | **not listed** | |
  | United Kingdom | Internet Watch Foundation | https://report.iwf.org.uk/en |
  | Ireland | Irish Internet Hotline | https://hotline.ie/report |
  | Australia | Cyber Report (eSafety, joined 2016) | a forms.esafety.gov.au form |
  | New Zealand | Netsafe | https://report.netsafe.org.nz/hc/en-au/requests/new |
  | Germany | eco; FSM; jugendschutz.net | three separate links |
  | France | Point de Contact (joined 1998, founding member) | https://www.pointdecontact.net/cliquez-signalez/ |
  | Spain | INCIBE (joined 2019) | https://www.incibe.es/menores/hotline |
  | Italy | Save The Children Italy; Telefono Azzurro | two separate links |
  | Netherlands | ATKM; Offlimits | https://meldpunt-kinderporno.nl/ (Offlimits) |
  | Mexico | Fundacion Pas | https://www.teprotejomexico.org/ |
  | Brazil | Safernet | https://new.safernet.org.br/denuncie |
  | India | Reporting Portal (an IWF portal) | https://report.iwf.org.uk/in |
  | Philippines | ECPAT Philippines (joined 2020) | http://ecpat.org.ph/report/ |
  | Japan | Internet Hotline Center Japan (Pole to Win) | https://www.internethotline.jp/ |
  | South Africa | Film Publication Board (joined 2009) | https://apps.fpb.org.za/hotline |
  | Nigeria | ACSAI Nigeria (joined 2024) | https://acsaing.org/report.html |

- 44 entries in the directory's drop-down are named "Reporting Portal" and link
  to `report.iwf.org.uk`: these are portals the UK's Internet Watch Foundation
  runs for countries without a hotline of their own (India is one of them).
- The directory is about **material** (images and videos). It is not an
  emergency service and does not list emergency numbers.

---

## United States

**Emergency number: 911**

- Source: https://www.911.gov/ (NHTSA's National 911 Program; no date on the
  page).
- Quote: "If you're experiencing an emergency, call 911 immediately."

**Online child sexual exploitation: NCMEC CyberTipline, https://report.cybertip.org/**

- Source: https://www.missingkids.org/gethelpnow/cybertipline (no date on the
  page).
- Quote: "NCMEC’s CyberTipline is the nation’s centralized reporting system for
  the online exploitation of children."
- The report site itself, https://report.cybertip.org/ (no date): "The
  CyberTipline is the place to report child sexual exploitation." and "If you
  or someone you know is in immediate danger, please call 911 or your local
  police immediately."
- INHOPE lists it for the United States (joined 1999).

## Canada

**Emergency number: 911 (Canadian sources write it 9-1-1)**

- Source: https://rcmp.ca/en/child-sexual-exploitation/online-child-sexual-exploitation
  (Royal Canadian Mounted Police; "Date modified: 2025-05-09").
- Quote: "If you know about a child who is in immediate danger or risk, call
  9-1-1 or your local police."

**Online child sexual exploitation: Cybertip.ca, https://www.cybertip.ca/en/report/**

- Same RCMP page: "To anonymously report online sexual exploitation of a
  child, please complete the Cybertip.ca Report Form."
- Public Safety Canada backgrounder,
  https://www.canada.ca/en/public-safety-canada/news/2018/02/funding_for_the_canadiancentreforchildprotection.html
  (dated 2018-02-07): Cybertip.ca is "Canada’s national tipline for the public
  to report suspected cases of online sexual exploitation of children."
- Cybertip.ca's own report page (no date) describes itself as "Canada’s
  national tipline for reporting the online sexual exploitation of children"
  and says: "Cybertip.ca is currently operating from 8:30am to 4:00pm Central
  Time, Monday to Friday, excluding holidays". Reports outside those hours are
  read the next business day, so the emergency number matters more here.
- **Not in INHOPE's member directory on 9 October 2026.** Why was not
  researched. Canada's entry rests on the RCMP and Public Safety Canada pages,
  not on INHOPE.

## United Kingdom

**Emergency number: 999**

- Source: https://www.ceop.police.uk/ceop-reporting/ (no date).
- Quote: "If you feel you are in danger and need help straight away, please
  call the police on 999."
- Note: the same page's HTML contains a notice "We are sorry but we can't take
  any reports at the moment", but it is inside an HTML comment and is not shown
  to visitors; the report form was live when read.

**Child at risk online: CEOP Safety Centre, https://www.ceop.police.uk/Safety-Centre/**

- Same report page: "CEOP is a law enforcement agency and is here to keep
  children and young people safe from sexual exploitation and abuse."
- Safety Centre page (no date): "Are you worried about online sexual abuse or
  the way someone has been communicating with you online?"

**Images and videos: Internet Watch Foundation, https://report.iwf.org.uk/en**

- That address redirected to https://www.iwf.org.uk/en/uk-report/ (no date;
  copyright 2026; footer "Registered Charity Number: 1112398"). Quote:
  "Anonymously report suspected child sexual abuse images or videos".
- IWF is the INHOPE member for the United Kingdom. The data file uses CEOP,
  because the report reason in the app is a child in danger, not a picture;
  see "Judgement calls" below.

## Ireland

**Emergency number: 112 or 999**

- Source: https://hotline.ie/ (no date on the page).
- Quote: "Where there is an immediate safety risk or you become aware of one,
  call 112 or 999."
- The European Commission's page on 112,
  https://digital-strategy.ec.europa.eu/en/policies/112 ("Last update 23 June
  2026"): "112 is the European emergency phone number, available everywhere in
  the EU, free of charge." This page backs 112 for every EU country below.

**Online child sexual abuse material: Hotline.ie, https://hotline.ie/report/**

- Hotline.ie home page: "Irish national centre combatting illegal content
  online".
- The report page's metadata gives `article:modified_time` 2026-09-29.
- INHOPE lists "Irish Internet Hotline" for Ireland.

## Australia

**Emergency number: 000 (Triple Zero); 112 also works from a mobile phone**

- Source: https://www.infrastructure.gov.au/media-communications/phone/triple-zero
  (reached via https://www.triplezero.gov.au/; Department of Infrastructure,
  Transport, Regional Development, Communications, Sport and the Arts; no date).
- Quotes: "Triple Zero (000) is Australia's main emergency number for life
  threatening or time critical situations." and "In Australia, you can also
  dial the international standard emergency number (112) from a mobile phone".

**Online child sexual exploitation: ACCCE, https://www.accce.gov.au/report**

- Report page (server `Last-Modified` 7 October 2026): "Is the child in
  immediate danger? Call Triple Zero 000 or call your local police".
- ACCCE home page, https://www.accce.gov.au/, in a news item dated 14 September
  2026: "The AFP-led Australian Centre to Counter Child Exploitation (ACCCE)".
- The INHOPE member for Australia is the eSafety Commissioner's Cyber Report.
  https://www.esafety.gov.au/report (no date): "eSafety can direct the removal
  of illegal online content, such as child sexual abuse material and terrorist
  material." The data file uses ACCCE, because it takes reports about a child
  being groomed or exploited, not only about content.

## New Zealand

**Emergency number: 111**

- Source: https://www.police.govt.nz/contact-us/111-police-emergency (New
  Zealand Police; no date).
- Quote: "Call 111 and ask for Police when: people are injured or in danger;"

**Online child exploitation material: Department of Internal Affairs,
https://www.dia.govt.nz/Digital-Safety-Report-Online-Child-Exploitation-material**

- That page (server `Last-Modified` 11 January 2026): "Note: If you or someone
  you know is in immediate danger, call 111 now." and "To report online child
  exploitation material, complete a content complaint form". The form itself
  is a Microsoft Forms page linked from there.
- Its scope is narrower than the others: "DIA only investigates instances of
  image-based offending." The page points to a PDF ("How to Report Online Child
  Exploitation") for the other organisations; that PDF was not read.
- The INHOPE member for New Zealand is Netsafe (report link
  https://report.netsafe.org.nz/hc/en-au/requests/new); Netsafe's own site was
  not read.

## Germany

**Emergency number: 112; police 110**

- European Commission 112 page (see Ireland): "You can call 112 from fixed and
  mobile phones to contact any emergency service: an ambulance, the fire
  brigade or the police."
- Bundesnetzagentur (the federal network regulator),
  https://www.bundesnetzagentur.de/DE/Fachthemen/Telekommunikation/Unternehmenspflichten/Notruf/start.html
  ("Stand: 22.08.2018"; the page itself says "Die Informationen sind nicht auf
  dem aktuellen Stand und werden in Kürze aktualisiert."): "Die
  Telefondiensteanbieter wandeln die Kurzwahlnummern 110 und 112 in die Nummer
  des Notrufanschlusses der örtlich zuständigen Notrufabfragestelle".
- That 110 is the police number comes from a state police page, Polizei Berlin,
  https://www.berlin.de/polizei/service/so-erreichen-sie-uns/notruf/ (metadata
  date 2026-09-16): "Wenn Sie in Not oder Gefahr sind, dann wählen Sie den
  kostenfreien polizeilichen Notruf 110." No federal page saying the same was
  found.

**Online content harmful to children: jugendschutz.net, https://www.jugendschutz.net/verstoss-melden**

- https://www.jugendschutz.net/ueber-uns/wer-wir-sind (no date): it "handelt
  mit gesetzlichem Auftrag" and acts as the "gemeinsames Kompetenzzentrum von
  Bund, Ländern und Landesmedienanstalten für den Schutz von Kindern und
  Jugendlichen im Internet".
- The report form asks: "Sie sind im Internet auf etwas gestoßen, das Sie für
  illegal, jugendgefährdend oder entwicklungsbeeinträchtigend halten?" and its
  content types include "Kinder- und Jugendpornografie".
- English page, https://www.jugendschutz.net/en/hotline: "Internationally,
  jugendschutz.net works closely with the networks INHOPE and INACH."
- Germany has two other INHOPE members, eco and FSM, which share one reporting
  site, https://www.internet-beschwerdestelle.de/en/: "Both organisations have
  their own hotlines and divide the reports received via this site according
  to their specific remits".

## France

**Emergency number: 112; police 17; child in danger 119**

- Source: https://www.service-public.gouv.fr/particuliers/vosdroits/F33954
  (the government's public service site; "Vérifié le 25 août 2026").
- Quotes: "Signaler une infraction : le 17 (Police secours)", "Infraction
  (violences, agression, vol, cambriolages) : 17 ou le 112" and "Enfance en
  danger (violences sur mineurs) : 119 et 116 111".

**Illegal online content, including harm to minors: PHAROS, https://www.internet-signalement.gouv.fr/**

- The site is a JavaScript application; its wording lives in
  https://internet-signalement.gouv.fr/assets/i18n/lang.json (server
  `Last-Modified` 29 May 2026), from which these strings are quoted: "En cas
  d'urgence, composez le 17", "atteintes aux mineurs", and "je ne partage pas,
  je signale à PHAROS !". The application's footer logo is titled "ministère
  de l'intérieur" (Ministry of the Interior).
- The INHOPE member for France is the association Point de Contact,
  https://www.pointdecontact.net/ (no date): "Point de Contact vous permet de
  signaler anonymement, simplement et gratuitement tout contenu ou situation
  potentiellement illégal rencontré sur internet."

## Spain

**Emergency number: 112**

- European Commission 112 page (see Ireland). No Spanish government page was
  read for this.

**Child sexual abuse material: INCIBE, https://www.incibe.es/menores/hotline**

- The page (no date; it refused a plain request and was read with a browser's
  headers) is titled "Línea de Reporte de Contenido de Abuso Sexual Infantil
  (CSAM)".
- What INCIBE is, from
  https://www.incibe.es/incibe/informacion-corporativa/que-es-incibe (no date):
  it "es una sociedad dependiente del Ministerio para la Transformación Digital
  y de la Función Pública".
- Quote: "La gestión de esta línea de reporte se realiza en el marco de las
  funciones de INCIBE-CERT y de la red INHOPE". It also says reports go to the
  Spanish police and INHOPE, and: "En ningún caso estos reportes tienen
  carácter de denuncia." A formal complaint has to be made to the police.
- INHOPE lists INCIBE for Spain (joined 2019).

## Italy

**Emergency number: 112 or 113**

- Source: https://www.commissariatodips.it/segnalazioni/segnala-online/index.html
  (Polizia Postale; no date).
- Quote: "se avete la necessità di contattare urgentemente le forze
  dell'ordine, comporre il numero telefonico Europeo 112 o 113."

**Online child sexual abuse: Polizia Postale, Segnala online (same URL)**

- The form's topics include "Pedofilia", with the field "URL della pagina che
  contiene immagini pedopornografiche". A report must be confirmed through a
  link sent by email, and the page says the form is not for formal complaints
  ("querele, denunce").
- The national centre behind it,
  https://www.commissariatodips.it/profilo/centro-nazionale-contrasto-pedopornografia-on-line/index.html
  (no date): "la legge istitutiva individua nel Centro il punto di raccordo per
  la trattazione delle segnalazioni".
- Italy's INHOPE members are Save the Children Italy and Telefono Azzurro;
  their sites were not read.

## Netherlands

**Emergency number: 112**

- Source: https://www.politie.nl/ (no date). Quote: "Bij spoed: 112".
- The hotline's own page, https://meldpunt.offlimits.nl/wat-melden: "Is een
  kind in direct gevaar? Bel dan het alarmnummer 112."

**Child sexual abuse material: Offlimits Meldpunt, https://meldpunt.offlimits.nl/**

- The INHOPE link https://meldpunt-kinderporno.nl/ redirects here (no date).
  Quote: "Beelden van seksueel kindermisbruik tegengekomen? Meld het bij ons."
- https://meldpunt.offlimits.nl/over-ons: "Internationaal is Offlimits
  onderdeel van het INHOPE-netwerk".
- INHOPE also lists ATKM for the Netherlands; not read.

## Mexico

**Emergency number: 911**

- Source: https://www.gob.mx/911 (Government of Mexico; newest dated item on
  the page is 25 October 2025).
- Quote (a document title on that page): "Número Único de Emergencias 911".

**Sexual violence against minors online: Te Protejo México, https://teprotejomexico.org/**

- (Server `Last-Modified` 9 October 2026; copyright line 2024.) Quote: "puedes
  reportar situaciones de violencia sexual en contra de personas menores de 18
  años de edad, de manera anónima y gratuita". Its categories include material,
  commercial sexual exploitation, and "Mensajería, Sextorsión, Grooming".
- "Te Protejo México forma parte de: INHOPE". INHOPE lists it as "Fundacion
  Pas"; the site's footer names Fundación Personas con Abuso Sexual de
  Guadalajara A.C.
- A government channel (the cyber police number 088) appears in news reports
  but was **not confirmed from a government page**, so it is not in the file.

## Brazil

**Emergency number: 190 (Polícia Militar); also 192 (SAMU), 193 (fire)**

- Source: https://www.gov.br/mcom/pt-br/noticias/noticias_alt/2026/setembro/emergencia-voce-sabe-para-quem-ligar-quando-precisa-de-ajuda
  (Ministry of Communications; "Publicado em 11/09/2026").
- Quote, on 190: "Número destinado ao atendimento de ocorrências relacionadas à
  segurança pública, como crimes em andamento ou situações que ofereçam risco à
  população." The same list continues "192 – SAMU" and "193 – Corpo de
  Bombeiros".

**Online crimes against children: SaferNet Brasil, https://new.safernet.org.br/denuncie**

- (No date. The page answers in English to an English-language browser; the
  Portuguese text is quoted.) Quote: "A SaferNet Brasil oferece um serviço de
  recebimento de denúncias anônimas de crimes e violações contra os Direitos Humanos na
  Internet". INHOPE lists Safernet for Brazil.
- The government's own line is Disque 100,
  https://www.gov.br/pt-br/servicos/denunciar-violacao-de-direitos-humanos
  ("Última Modificação: 15/12/2025"): the ministry receives "denúncias de
  violações de direitos de crianças e adolescentes", reachable "bastando discar
  100". It is a reporting line, not an emergency service.

## India

**Emergency number: 112**

- Source: https://www.mha.gov.in/en/commoncontent/emergency-response-support-system-erss
  (Ministry of Home Affairs; footer "Last Updated: 13 Sep 2024").
- Quote: "a nationwide, unified emergency response system with a single
  emergency number ‘112’". The page says 112 calls go to police, health, fire,
  women and children helplines among others.
- 112.gov.in itself refused the connection on the research date.

**Women and child related cyber crime: National Cyber Crime Reporting Portal, https://cybercrime.gov.in/**

- FAQ, https://cybercrime.gov.in/Webform/FAQ.aspx (footer "Last Updated:
  02/02/2024"): "This portal is an initiative of Government of India to
  facilitate victims/ complainants to report cyber crime complaints online." It
  covers "Child Sexual Exploitative and Abuse Material (CSEAM)" and allows
  anonymous reports. The home page footer reads "Last Updated: 31/08/2026" and
  has a "Women/Children Related Crime" section.
- INHOPE's entry for India is an IWF Reporting Portal
  (https://report.iwf.org.uk/in), not an Indian organisation.

## Philippines

**Emergency number: 911**

- Source: https://calabarzon.dilg.gov.ph/one-number-for-all-emergencies-unified-911-to-launch-nationwide/
  (Department of the Interior and Local Government, regional office; "Posted on
  September 5, 2025").
- Quote: "Beginning September 11, Filipinos facing emergencies will only need
  to dial one number: 911."

**Child sexual abuse and exploitation material: ECPAT Philippines, https://ecpat.org.ph/report/**

- (No date.) Quote: "the eProtectKids CSAEM Hotline receives reports of Child
  Sexual Abuse and Exploitation Material (CSAEM) on the internet", run "In
  tripartite partnership with INHOPE" and the Department of Information and
  Communications Technology's Cybercrime Investigation and Coordination Center
  (DICT-CICC).
- The same page calls INHOPE "a global network of 47 Internet Hotlines", which
  is out of date, so the page has not been revised recently.
- INHOPE lists ECPAT Philippines (joined 2020). Government channels (police,
  NBI, CICC) were not researched.

## Japan

**Emergency number: 110 (police)**

- Source: https://www.pref.aichi.jp/police/anzen/110/index.html (Aichi
  Prefectural Police; no date). Quote: "「１１０番」は事件・事故の緊急通報のための専用ダイヤルで".
- The hotline's own page, https://www.internethotline.jp/: "For emergent cases
  endangering human lives, please directly call 110."
- 119 (fire and ambulance) was not confirmed from a source and is not in the
  file. A national police agency page for 110 was not found; the Aichi page is
  a prefectural police page.

**Illegal online content, including child sexual abuse material: Internet Hotline Center Japan, https://www.internethotline.jp/**

- The report categories include child sexual abuse materials, described as
  "nude images of minors" among others.
- https://www.internethotline.jp/about/construction (committee list "2026年9月現在"):
  "ホットラインセンターは運営状況を、業務委託元である警察庁に報告している。" (The
  centre reports on its operations to the National Police Agency, which
  commissions it.)
- INHOPE lists "Internet Hotline Center Japan (Pole to Win)".

## South Africa

**Emergency number: 10111 (police)**

- Source: https://www.saps.gov.za/ (South African Police Service; newest dated
  news item 21 September 2026). Quote: "Dial 10111".
- The site's TLS certificate chain was incomplete (the web fetch tool refused
  it; a command-line fetch that skipped the chain check read it). 112 from
  mobiles was not confirmed and is not in the file.

**Child sexual abuse material: Film and Publication Board Hotline, https://apps.fpb.org.za/hotline/**

- (No date.) Quote: "Report Child Sexual Abuse Material (CSAM), And any form of
  harmful or violent content online".
- https://www.fpb.org.za/ (copyright 2026): "The Film and Publication Board
  offers a hotline for reporting Child Sexual Abuse Material and provides
  immediate psychosocial support." The site's title calls it the "Content
  Regulatory Authority of South Africa".
- INHOPE lists it (joined 2009).

## Nigeria

**Emergency number: 112, where the state's emergency centre is running**

- Source: https://ncc.gov.ng/node/3132 (Nigerian Communications Commission;
  "Last Updated February 25, 2025").
- Quote: "All telecom operators will be mandated to route emergency calls
  through the dedicated three-digit toll free number, 112, from each state to
  the emergency centre within that state." The page says "Numerous centres have
  been built", not that every state has one. Whether 112 answers in every state
  today is **unknown**.

**Online sexual abuse, including of children: ACSAI, https://acsaing.org/report.html**

- (Server `Last-Modified` 10 July 2026.) Quotes: "Action Against Child Sexual
  Abuse Initiative is a registered NGO dedicated to online sexual abuse
  protection and trauma recovery." and "We have adopted the INHOPE TDN for the
  ACSAI hotline."
- INHOPE lists ACSAI Nigeria (joined 2024). A government channel (police,
  NAPTIP) was not researched.

---

## What is still unknown, and what would settle it

- **Whether these hotlines want links from an app.** None was asked. A short
  email to each, or to INHOPE once, would settle it and might also produce
  better links (some hotlines publish deep links for platforms).
- **Government channels for Mexico, the Philippines and Nigeria.** Each entry
  rests on an INHOPE-member NGO. A government page for Mexico's 088, the
  Philippines' CICC or police, and Nigeria's police or NAPTIP would add an
  official option. Cost: about an hour of reading.
- **Why Canada is absent from INHOPE's directory.** It does not change
  Canada's entry (the RCMP names Cybertip.ca), but it is worth one line of
  explanation if someone asks. INHOPE's own news or Cybertip.ca's "About" page
  would answer it.
- **Nigeria's 112 coverage by state**, and **South Africa's 112 from mobiles**:
  the NCC and the South African communications regulator would know.
- **New Zealand's route for grooming and other non-image harm**: the DIA PDF
  "How to Report Online Child Exploitation" and Netsafe's own site.
- **Every other country.** Only 18 were researched. The fallback covers the
  rest without guessing.
- **How long any of this stays true.** Report URLs change. A link check at
  every release (the file is small) would catch a dead link; a full re-read
  once a year would catch a hotline that has changed role.

## Sources that could not be reached

- https://112.gov.in/ : connection refused and timed out (wanted the national
  112 site's own words; the Ministry of Home Affairs page was used instead).
- https://www.bka.de/ : connection timed out (wanted a federal police page for
  110; the Berlin police page was used instead).
- https://www.bmi.bund.de/ (an emergency-number FAQ): connection timed out.
- https://crtc.gc.ca/eng/phone/911/ : refused with HTTP 403 (a bot check).
  Canada's 911 rests on the RCMP page instead.
- https://www.polizei-beratung.de/opferinformationen/notruf-110/ and
  https://www.npa.go.jp/bureau/soumu/110/ : HTTP 404 (guessed addresses).
- https://www.gob.mx/sesnsp/... 9-1-1 pages: HTTP 404; https://www.gob.mx/911
  was used instead.
- https://www.saps.gov.za/ : incomplete certificate chain (see South Africa).
- https://www.incibe.es/menores/hotline : refused a plain request ("Request
  Rejected"); read with browser headers.
- https://www.internet-signalement.gouv.fr/ : a JavaScript application with no
  text in its HTML; read through its language file.

## What it means for this project

**Certain** (stated by the sources above):

- In all 18 countries an official source names an emergency number, and ten
  of the 18 reporting bodies tell people, on the pages read, to call it first
  when someone is in immediate danger.
- In all 18 countries there is a named body that takes reports of online child
  sexual exploitation or abuse, and its report page is at the address in the
  data file on 9 October 2026.
- Several of these are not police, and some reports to them are explicitly not
  formal complaints (Spain, Italy).
- INHOPE's directory lists national hotlines for abuse material by country
  (and IWF portals for countries without one). It was the only international
  list looked at.

**Judgement calls** (the researcher's choices; a reader may reasonably choose
differently):

- **One body per country in the data file.** Where there were two (the United
  Kingdom's CEOP and IWF, Australia's ACCCE and eSafety, France's PHAROS and
  Point de Contact, Germany's three, Brazil's SaferNet and Disque 100), the
  file names the one that takes reports about a **child at risk** online
  rather than only about pictures, and prefers police or government where such
  a body takes online reports. Brazil is the exception: Disque 100 is the
  government's line, but it is a phone line for rights violations in general,
  so the file names SaferNet's online form and lists 100 under `also`. The
  others are in this document.
- **The emergency number in the file is the one to show first**, usually the
  general number (112 in the EU) with the police number under `also` where a
  source gave one. For France, 17 or 119 could reasonably come first instead.
- **The app should say what it is showing**: "These are outside services. This
  server's admins are volunteers, not police." That matches the help text
  already specified in section 10e, and the project's rule of saying plainly
  what the app does not do.
- **The country should be the person's choice, not guessed.** The app has no
  reliable way to know where someone is (and should not try to find out).
  Showing the whole list, perhaps starting from the country that matches the
  device's own region setting, plus the INHOPE fallback, avoids a wrong guess
  sending someone to the wrong country's line.

This document is a reading of public sources on 9 October 2026, not legal
advice. Hotlines, laws and phone numbers change; check the dates above before
relying on any entry.
