# Since the last public update

A running list of what has shipped since the last posts went out, kept so the
next update can be written from it without digging through the logs. The last
posts were the update of 25 October 2026
(`docs/outreach/posts/2026-10-25-update.md`, v0.1464.1 to v0.1547.0).
When the next update is written, move this list into it and start a fresh one.

Each line is written the way a player would hear it; the release notes and
`docs/history/` hold the detail.

## Shipped

### v0.1554.1 (10 November 2026)

- v0.1554.0 for macOS and Linux, whose builds had stopped on one line meant only for Windows.

### v0.1554.0 (10 November 2026)

- Shape your body on the character page with build, proportions and ancestry sliders; other players see the shape you made.
- Feet stand on what is under them: ship floors, stair treads, ramps and the slopes of the map, and a planted foot stays put.
- Hold C to crouch (others see it), and jumping plays a take-off, a moment in the air and a landing.
- Settings > Gameplay > Animation: Realistic, or Simplified for older computers. The character page shows your height in metres.
- AI only if you want it: Settings > AI can download a small model, IBM's Granite 4.0 1B (about 1 GB), showing its size, maker and licence first; downloads resume, every file is checked, and Remove deletes it all. Everything works without it.
- Map roads sit in beds cut into the ground and buildings stand on level pads, with full detail near you.
- Your own body on a planet now stands where you are, not 400 km up with the station.

### v0.1553.0 (9 November 2026)

- Real people: you, other players and the crew are now real human bodies, a woman and a man made from MakeHuman's free body data, instead of the plain test figure.
- They idle, walk, jog and sprint with animation clips from Quaternius's free animation library, fitted to each body so the feet stay planted.
- Choose your body on the character page; other players see the one you picked, and your height reshapes the body itself.
- Settings > Gameplay > Walking pace: Simplified (the quick pace) or Realistic (a 1.4 m/s walk and a 4 m/s run, the pace of a real person).
- Silverdale and Seattle come from the new map packs: the waterfront slopes gently into the water like the real shore, with no sheer cliffs and no dry harbour.
- The website's download page now gets each release by the server fetching it from GitHub and checking every file, instead of an upload that could hang.

### v0.1552.0 (9 November 2026)

- People have bodies: you in third person, other players and the crew stand, turn, walk and run, and their legs keep pace with how fast they move.
- Third person walks your body through the ship with the camera over your shoulder.
- 40 detailed models from NVIDIA's free SimReady library in the homestead and the Commons: a kitchen counter with its basin and tap, a workbench, a vise, a tool chest, hand tools, a fire extinguisher, a first aid kit and more.
- A Medical Bay set to install from Settings > Graphics > Furniture models.
- Settings > Graphics > Furniture detail, which shows the video memory each choice uses and says plainly when your graphics card is short.
- Furniture that was drawn from its corner now stands where it belongs, and entering the world is faster.
- The OpenStreetMap credit shows wherever map data is drawn, and heights are credited to their real sources.

### v0.1551.0 (8 November 2026)

- Hidden means hidden: turning off "Show others when I am online" now keeps you out of every list and announcement.
- Erasing your account takes everything that is yours, and both erase windows say exactly what stays.
- Bug reports no longer show who sent them, except to admins and moderators.
- The website no longer asks GitHub on every page.
- Files in members-only rooms open only for that room's members.
- Settings sync is sealed or nothing; group details are for members only.
- The server's own files are readable only by its own account.

### v0.1550.1 (7 November 2026)

- The ship has music: a live score at A = 432 Hz with deep drums, steel pan, strings and bells, rising with danger and settling back. Settings > Music.
- The ship hums, tuned to the music, louder near the drive.
- Free recorded instruments to install from Settings > Audio > Instruments; a small set ships inside the app.
- Every sound has its own level, and there is a Notifications volume, so turning clicks down never silences chimes.
- A chat video turns the music down; opening a page from the world keeps the music quieter instead of stopping it.
- Videos play inside their message, with a preview picture, and can follow you around the app in a small dock.
- In-world screens can be any size and shape.
- A direct message and your own copy can no longer be matched up on the server; pictures the server cannot clean are refused unless you choose "Send anyway".
- A privacy policy that says what the server keeps, for how long and who can see it, in the Library and at united-humanity.us/privacy.
- Our own page for an address that does not exist, and a description and share preview on every website page.

### v0.1549.0 (5 November 2026)

- Back and Forward like a web browser: the arrows at the top left, the mouse's side buttons, Alt+Left and Alt+Right.
- Escape goes back to the page a button opened from, and closes the nearest popup first.
- Invite links name the server you are on.
- Linked servers no longer keep the old words of a message edited or deleted right after posting.

### v0.1548.0 (3 November 2026)

- A double top menu: eight categories, each with its row of pages; rest the pointer on one to preview it.
- Font Size works: the whole app from 100% to 200%.
- Chat channels each say who can read and who can post; anyone can read the public ones without signing up.
- #general is now #global; #feedback is on every server, for telling us what works and what does not.
- Links straight to a channel, like united-humanity.us/chat#global, and Copy link on every room.
- Your own settings for each person on their profile card: their notifications, quiet hours, files.
- Privacy: switches that did nothing are gone, the Privacy page says what others can see, and you stay hidden until you choose.
- AV1 videos in MP4 files play in the app.
- The Files page in three colours: private, what you share, what everyone shares.
- Desktop notifications, quiet hours with people you always let through, and a tray icon.
- A server whose upgrade fails now stops and says why, and never swaps in an older backup.
