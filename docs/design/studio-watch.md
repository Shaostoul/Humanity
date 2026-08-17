# Studio and Watch: streaming in and around the Chat page

> Operator vision (2026-08-16/17, condensed from their words): stream from
> the PC to your own relay, simulcast out to YouTube/Twitch/X/FB; Studio
> keeps a dedicated chat pane so you watch chat while operating the studio.
> Watch lets people watch each other, and eventually Twitch/YouTube through
> one interface, someday synced watch parties. The web header's Studio
> button is gone (2026-08-17): the streaming surface's home is the Chat
> page. "Maybe like an expandable tab at the top of the chat page... people
> may want to wander chats while still watching the stream... incorporate
> the popout function so people can watch and use the studio while on other
> pages. This'll probably be one of the harder GUI elements to nail down."

## What exists today (the honest baseline)

MORE than a stub: a working single-rung pipeline shipped in v0.857 and then
paused. Studio broadcasts frames to the relay (broadcast_frames counter,
drop tracking), the relay serves a live-stream directory at GET /api/live,
and the Watch page polls it and opens a LiveViewer with a real video
surface. Profile carries streaming_url + streaming_live; WebRTC voice with
relay TURN ships. What does NOT exist: chat binding, the live-now strip,
popouts, ladder transcoding, simulcast out, external embeds.

## The four hard questions, answered

**1. Resolution ladders ("how do we send 144p of a 4K stream without
hurting the streamer's or viewer's compute?").** Nobody can extract a small
rendition from a single encoded stream without someone transcoding it. The
standard broadcast answer, and ours: the streamer uploads ONE encoding to
their relay; the RELAY transcodes server-side into a ladder (for example
1080p / 480p / 144p) and every viewer picks a rung. The streamer pays one
encode (what OBS costs today), the viewer pays only the rung they watch,
and the relay pays the transcode (ffmpeg; each rung is a known CPU cost).
A relay without transcode capacity declares source-only and popouts simply
cost more bandwidth. Simulcast rides the same ingest: the relay pushes
RTMP out to YouTube/Twitch/X/FB, so the PC uploads once no matter how many
platforms are fed.

**2. Which chat pairs with a stream ("could be a DM or group or server...
how would we differentiate?").** The pairing is METADATA, not guesswork:
going live declares a chat binding in the stream announcement, chosen by
the streamer at Go Live: an existing channel, a group, or an auto-created
room (default: #live-<name>). Watch surfaces subscribe to the binding.
One stream, one declared home; viewers can still wander anywhere.

**3. Wandering while watching + using Studio from other pages.** A global
POPOUT overlay: floating mini-players drawn above whatever page is active
(egui windows in a top layer, the same way modals already float). The
popout carries pause/volume, a rung selector, a "chat" jump to the bound
room, and pop-back-in. Studio gets the same treatment: a mini preview +
live/stop control that follows you across pages while the full Studio page
stays where the deep controls live.

**4. Multiple streams.** The Watch page is the grid: side-by-side players,
each with its rung (several 144p tiles are cheaper than one 1080p). Any
tile can pop out. The chat pane shows the FOCUSED stream's bound room, one
click to swap focus.

## The shape (agreed direction)

- Chat page gains a thin LIVE NOW strip at the top (expandable, per the
  operator's instinct): avatars of live streamers you follow/share a
  server with. Click = popout + offer to open the bound chat room.
- Studio stays a full page (an OBS-class surface does not fit in a tab)
  but its preview/live state pops out everywhere.
- Watch stays a full page (browse + grid + external embeds later); the
  per-user URL is /watch/<name> on the web.

## Mechanism correction (v0.1150 recon, read before citing this doc)

The pipeline is a BINARY WEBSOCKET FANOUT, not segment POSTs: the app
captures its own window, encodes MJPEG, and publishes frames
([1B tag][8B PTS][payload]) over wss://<relay>/ws/live/pub with in-band
Dilithium auth; viewers subscribe at /ws/live/sub/<name>; GET /api/live is
only the JSON directory. Stream id = the publisher's registered name. The
relay keeps nothing on disk (last codec-config + keyframe cached in memory
for joiners). Full map with file:line cites lives in the v0.1150 journal
entry; code: src/net/live.rs, src/relay/live.rs, web/pages/watch.html.

## Increments, in order

1. **Revive + bind**: the v0.857 pipeline works end to end on one server;
   increment 1 polishes it and adds the chat BINDING to the go-live
   announcement plus the Chat live-now strip. No new video path needed.
   **FIRST HALF SHIPPED v0.1150**: pipeline re-verified (34 live tests,
   including the real-encoder-to-decoded-pixels e2e), the Chat page's dead
   Go Live fixed (it never set broadcast_request, so it silently broadcast
   NOTHING), the four stale "rehearsal only" strings replaced with honest
   publisher-mirrored status, the full-width expandable LIVE STRIP at the
   top of Chat (Go Live / End / Open Studio + the live-now directory with
   one-click Watch; replaces the old right-rail section), and /watch
   accepts ?u=<name> as an alias of ?s=. REMAINING in this increment: the
   chat-binding field in the go-live announcement (relay AuthFrame +
   snapshot JSON + a default #live-<name> room) and the web chat mirror
   (chat-live.js polling /api/live beside the existing #stream-sidebar).
2. **WebRTC small-audience video**: reuse the shipped WebRTC + TURN stack
   for P2P screen/camera share to small rooms (this is also the watch-party
   seed). Viewer cap documented honestly.
3. **Relay ingest + ladder + simulcast**: RTMP/WHIP ingest on the relay,
   ffmpeg ladder transcode, RTMP push-out to external platforms.
   Server-config gates it (a capability in the features manifest) since it
   costs real CPU.
4. **Popout overlay + multi-stream grid.**
5. **External embeds in Watch** (Twitch/YouTube players in one interface).
6. **Watch parties**: synced play position via the relay + bound chat.
   External DRM services cannot be embedded; sync-press-play + chat works
   for anything.

## Open questions for the operator (when increment 3 is fenced)

- Which rung set is the default ladder (1080/480/144 vs 720/360)?
- Does the public relay offer transcoding to everyone or per-role?
