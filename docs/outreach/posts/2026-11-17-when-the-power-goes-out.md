# When the power or the internet goes out: a preparedness invitation

Asked for by the operator on 2026-11-17: "a lot of people are scared currently about the Great Reset
happening... This could result in the loss of electricity and internet. However some people can
provide their own electricity and internet... Even just a household. I want to appeal to helping
people endure what could potentially be the Great Reset happening."

## How this is framed, and why

The posts below speak to the fear (losing power and the internet) without the label. "The Great
Reset" names a 2020 World Economic Forum initiative and is also the name of a conspiracy theory
about it, so using it puts HumanityOS in one political camp. That breaks the outreach rule (no
partisan framing, docs/outreach/README.md) and turns away everyone outside that camp. A post about
"if the power or the internet went out for a week" reaches the people who fear it AND the people who
don't, because preparedness is useful whatever the cause. It also needs no claim a reply can refute,
and it is the project's own purpose (docs/design/platform-architecture.md: "HumanityOS is built for a
planet that experiences floods, earthquakes, hurricanes, pandemics, wars, and infrastructure
collapse on a regular basis.").

**The news, checked 2026-11-17 (web search; mostly secondary reporting):**

- The US bank outages of 16 November 2026 (thousands of Downdetector reports for Chase, Bank of
  America, Wells Fargo, U.S. Bank, Zelle): Chase attributed them to a software bug and said there
  was no evidence of a cyberattack; no coordinated attack had been confirmed.
- Microsoft's outage of 12 to 13 November 2026 was attributed by Microsoft to a configuration
  change after a routine update, not an attack.
- The viral "17 November blackout" and "90-day blackout" claims had no official backing. The same
  day carried a scheduled nationwide Emergency Alert System test (FCC notice DA 26-989, 16 September
  2026: about 2:20 pm Eastern on TV and radio, saying it is a test; backup date 3 December 2026).
- Real attacks do happen (the French bank Credit Mutuel Arkea in November 2026; Poland's grid in
  2025), but the posts do not need them, and a post leaning on unconfirmed ones would be refuted in
  the replies.

So no post below names an incident. If one ever does, quote the primary source (the company's or
agency's own statement) with its date.

**What HumanityOS honestly offers when the network fails (checked in the repo 2026-11-17):**

- The Library ships inside the desktop app (`data/library/`), so its guides open with no connection
  (categories Grow Food, Wild Food, Water, Shelter and Power, `data/library/catalog.json`). It only
  helps if it is downloaded before the outage: that is the honest call to action, not manufactured
  urgency.
- Saved work lives on the device (docs/outreach/how_humanityos_helps.md).
- Any computer running the app can host a server for the devices on its own network from a button
  (Servers > Host a node on this PC > "Who can connect": "Devices on my network",
  docs/design/in-app-ops.md). Anyone can run a server, and servers connect to each other when they
  can, so no single company's server has to stay up.
- **Not yet proven:** that a home network with the internet unplugged at the router keeps two of its
  computers chatting through a node hosted on one of them. Until that test passes, the posts say
  what is verified (a server for the devices on your own network) and stop short of "keeps working
  when cut off".
- The game world (food, water, power, repair) is in active development and not fully playable yet.

## The core version (Facebook)

```text
If the power or the internet went out for a week, what would you need?

A lot of people are worried about that, and the worry makes sense. Outages happen: storms, fires, accidents, a bad software update, sometimes an attack. Whatever the cause, the households that come through best are the ones that can cover a few basics themselves: safe water, food, light and heat, and a way to reach the people nearby.

That's what HumanityOS is for. It's a free, open-source app built to keep being useful when the wider world isn't:
- Its library of practical guides (growing food, finding wild food, safe water, shelter and power) lives on your own computer and opens with no connection at all.
- Your notes, plans and saved work stay on your device, not on someone else's server.
- Any computer running it can host a server for the devices on its own network, from a button in the app. Anyone can run one, so no single company has to stay up for it to work.
- A game world, still being built, where you practice growing food, cleaning water and making power. A mistake there costs nothing, and the skills carry over to real life.

You don't have to believe the worst is coming to want this. A family that can feed itself for a while, drink safe water and talk to its neighbors is ready for a winter storm, a long outage or a hard month, whatever brings it.

It's free, with no sign-up, no ads, and no company that owns it. It's early and rough in places, and I'd love your help making it better: try it, tell us what confused you, share it with someone who's been worried, or bring what you know.

One honest note: the guides only help in an outage if they're already on your computer. Get it while the internet works.

united-humanity.us
```

## Threads, Mastodon (500 characters)

```text
If the power or the internet went out for a week, what would you need?

HumanityOS is a free, open-source app whose practical guides (growing food, wild food, safe water, shelter and power) live on your own computer and open with no connection. Any computer running it can host a server for the devices on its own network.

You don't have to expect the worst to want this. It helps in a storm, an outage or a hard month.

The guides only help if they're already downloaded: united-humanity.us
```

## X and Bluesky (280 and 300)

```text
If the power or the internet went out for a week, what would you need?

HumanityOS is a free, open-source app whose survival guides (food, water, shelter, power) live on your own computer and open with no connection.

Get it while the internet works: united-humanity.us
```

## Instagram

Use the Facebook text up to "whatever brings it.", then:

```text
Free, no sign-up, no ads, and no company owns it. It's early, and I'd love your help: try it, tell us what confused you, or share it with someone who's been worried.

The guides only help in an outage if they're already on your computer. Type united-humanity.us into your browser while the internet works.

#Preparedness #EmergencyPreparedness #SelfSufficiency #Homesteading #OffGrid #OpenSource
```

## Discord (no closing links block)

```text
**If the power or the internet went out for a week, what would you need?**

A lot of people are worried about outages, whatever they think the cause might be. Whatever happens, HumanityOS can already help a household through one:
- The Library's guides (growing food, wild food, safe water, shelter and power) ship inside the desktop app and open with no connection.
- Your saved work stays on your own device.
- Servers > Host a node on this PC lets any computer serve the devices on its own network.

The one catch: it only helps if it's downloaded before the outage. If someone you know has been worried, this is something calm and practical to hand them.

And if you know real preparedness skills (water, food storage, first aid, power), checking and adding to the Library's guides is one of the most useful things anyone can do for the project.
```

## Left out, on purpose

- "The Great Reset", and any named villain: the posts are about what a household can do, not who to
  blame.
- Unconfirmed attacks and rumors (see above), and dates that would tie the post to one news cycle.
- Fear words ("collapse", "before it's too late", "when it hits"). The only urgency is the true one:
  the guides must be on your computer before the outage.
- "Keeps working when cut off" until the unplugged-router test passes.

## Sources read 2026-11-17

- https://cybersecuritynews.com/chase-outage-disrupts-bank/
- https://ransomnews.com/chase-bank-of-america-outage-cyberattack-november-2026/
- https://www.malaymail.com/news/money/2026/11/17/after-binance-hack-and-credit-mutuel-arkea-attack-us-bank-outage-sparks-cyber-fears/243552
- https://www.yahoo.com/news/us/articles/90-day-blackout-rumors-spreading-133110846.html
- https://docs.fcc.gov/public/attachments/DA-26-989A1.txt (the FCC's notice of the test)
- https://www.tvtechnology.com/regulatory-legal/fcc/fcc-releases-more-details-on-nationwide-eas-test
- https://en.wikipedia.org/wiki/2025_cyberattack_on_the_Polish_power_grid
