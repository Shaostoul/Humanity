# Since the last public update

A running list of what has shipped since the last posts went out, kept so the
next update can be written from it without digging through the logs. The last
posts were the update of 11 November 2026
(`docs/outreach/posts/2026-11-11-update.md`, v0.1548.0 to v0.1556.0).
When the next update is written, move this list into it and start a fresh one.

Each line is written the way a player would hear it; the release notes and
`docs/history/` hold the detail.

## Shipped

### Next release

### v0.1564.0 (16 November 2026)

- Organisations deliver themselves: changes reach every member as sealed messages, owners can hand an organisation on, and Guilds are folded into Organisations.
- Rooms look more finished: casings round every door and window, sills, and chair and picture rails you can add per room or per wall.
- Skills feel like practice: repeating one task gives less until you rest, a first try counts extra, and a guide or a more skilled teacher helps; the Skills page says why.
- No furniture blocks a doorway, and every room can be walked into round its furniture.
- The simulation runs without a screen: a week of the homestead in about half a second, a person kept alive by a daily routine, and a walk through the homestead's floors with no window.
- Fixed: a new player's privacy question buried under garden notices that blocked its button, overlays that blocked clicks under them, and a dead body that could still eat.

### v0.1563.0 (15 November 2026)

- The chat follows you around the app: a footer on every page but Chat, one line folded, or the latest messages and a box to type in unfolded, with the same conversation and draft as the Chat page (Alt+C, or turn it off in Settings).
- Furniture is the size it looks: footprints match the models, the bed lies the right way at a real queen length, and furniture against a wall stands on the skirting.
- The phone build compiles on GitHub's free build machines.

### v0.1562.0 (15 November 2026)

- A new licence: the code is now under the GNU AGPL 3.0, so anyone who runs a changed HumanityOS server must share its source; the Library and docs are under CC BY-SA 4.0, so Wikipedia's text can come in, credited; game data and the Humanity Accord stay public domain; and a short policy keeps the name HumanityOS for the real thing.
- Skills: the base set, with no cap; each stage says in plain words what you can do, everything you do trains its own skill, and a new Skills page shows the next step. 103 of the statements were corrected against primary sources before shipping.
- Organisations: members, roles, invitations, a directory and an audit log, with nothing kept on the server; Sponsor-a-Can first.
- Rooms look finished: skirting and crown moulding round every room with mitred corners, chosen per room or per wall in the build editor.
- A model inspector: see a 3D model's attachment points, axes, size, catalog box and collision shapes before you use it, in the app and on the website.
- The homestead: furniture stands flush against its walls, and glass roofs carry glazing bars so they read as roofs.
- Music: one place chooses what plays, and house, drum and bass, synthwave and ambient join the genres.
- Far trees cost less: each card is a tight outline around its tree, and the GPU draws only the ones in view.
- Android, first rung: the app builds as a phone app on GitHub's free build machines and says plainly what a phone cannot do yet.
- Fixed: the chat's text box hiding below the window in some channels, and films through ffmpeg looping without a picture on a busy computer.

### v0.1561.0 (14 November 2026)

- Far forests: distant tree cards draw out to the Tree silhouette distance for the first time in any release.
- Row Street comes alive: neighbours' houses are lit inside by day and night and their gardens are planted along the yards.
- New synths and drum machines: a wavetable synth, a supersaw, a pulse-width synth and 808 and 909 style drum kits; lo-fi, electro swing and deep dubstep use them.
- Cheaper room-light shadows, and the blur behind frosted plastic no longer borrows light from beside the tent.
- Fixed: an in-app update bringing a terminal-started game back invisible, Row Green listing no ways out, the music's streaming thread outliving the music, a crash when a camera screen or a high-resolution screenshot showed frosted plastic.

### v0.1560.0 (14 November 2026)

- The Music page: explore endless pieces by seed and genre, tune one, keep favourites, export them as WAV files, and watch the music as a piano roll while it plays.
- Eleven free instrument libraries, with optional packs and seven full-length packs (417 instruments) to download from Settings; long recordings play from disk, so the full Steinway takes about 15 MB of memory instead of 452 MB, and sounds exactly the same.
- Deep dubstep: 140 bpm half-time with a tempo-locked wobble, a sub bass that ducks under the kick and vowel-shaped growls; electro swing and lo-fi now pick their kits and pianos piece by piece, and calm music sometimes sings its pads with a choir.
- Row Street: four row house layouts (two bedrooms for four, three for five, three couples, five friends) on their own street with one open green behind the yards; build a fence on your plot line only if you want one.
- Softer hair and steadier distant trees with temporal anti-aliasing; your own fringe shows on a planet.
- Frosted tent plastic and glass cast faint tinted shadows, and blur what is behind them by distance.
- Shadows from room lights cost the computer 20 to 43 percent less.
- Fixed: getting up from a chair tucked under a desk, a piece changing when a genre was added, screenshots with the reuse switch off.
- Plans: every activity in the game catalogued, a proposed base set of skills (alchemy is now chemistry), what everyday services we still lack next to Microsoft's, and what it takes to run on phones and every other device.

### v0.1559.0 (13 November 2026)

- Smoother edges: temporal anti-aliasing is on by default (Settings > Graphics > Image), hair, chairs and the horizon lose their jagged steps.
- Endless music: a new piece every few minutes, each with its own key, song shape, chords, melodies and lead; switching genre is heard in seconds; a steel-string guitar, violin and tenor sax.
- You stop at furniture, and talking characters use the ship's real place names.
- The mushroom racks wear fitted frosted plastic covers with a zip front that rolls up while you tend them.
- Videos the player cannot decode play at once through your ffmpeg, with nothing written to disk.
- Pulleys on any world: choose the planet or the ship's rim.
- Fixed: the sea's backstop in the wave troughs, two tree sliders that did nothing until you walked, stock piles left behind a moved zone.
- Plans: a better radiation shield (water in valved compartments, magnets not worth it at drum scale), every use for slag and the armour vote per ship, every free instrument library worth adding.

### v0.1558.0 (13 November 2026)

- Talking characters: walk up to a crew member and press E to talk about the ship, their work, the cause or the way, take on work, trade, or have them mark the way; all of it works with written lines, and with an AI model installed a "Say anything" box lets you ask in your own words, every AI line tagged.
- Music genres in Settings > Music, with electro swing at 128 bpm played from recordings that come with the app.
- A Pulleys mode on the Calculator page designs a block and tackle for any load and any puller, from one person to a ship's winch, and says what it costs in rope and time.
- Crowds walk through doorways, round tables and round each other.
- The garden's pests, pollination, soil pH and picking each have Off, Gentle and Realistic.
- Room light shadows set to Off now cost nothing while the scene is drawn.
- Plans: the mothership sizes research, the radiation shield set for 5 mSv a year, a room-by-room program for homes, the renderer roadmap.

### v0.1557.0 (12 November 2026)

- The music plays from the moment HumanityOS opens and on the pages all session (Settings > Music > Music on the app's pages).
- Settings > AI also offers Ministral 3 3B by Mistral AI, the stronger "Rich" model beside Granite; nothing AI is installed unless you choose it.
- Faces shaped with the detail sliders no longer fold into ridges at the limits.
