# Your privacy in calls and voice rooms

*Written 2026-10-22. This page explains one setting in plain words: how your calls and voice
rooms connect, and who can see your network address in each choice.*

## What a network address shows

Every device on the internet has a network address (an IP address). Anyone who learns it can
usually tell roughly where you are (often the town or the region) and who your internet provider
is. It does not show your name, and it cannot be used to read your messages or hear your calls.
The server you use already sees your address, the same way every website you visit sees it.

## The three choices

You choose in the desktop app under **Settings > Privacy > Calls and voice rooms**, and in the web
chat under **Settings > Safety > Calls and voice rooms**. The choice is kept on each device, so
set it on each one you call from. It applies to calls and voice rooms you start or join after you
change it.

**Private (the default).** Your sound goes through your server's call relay. Nobody in the call
sees your network address: they see only the server's. The server passes your call on without
being able to hear it, because the sound is encrypted between the people in the call. This uses
some of the server's bandwidth, and can add a little delay if the server is far from you.

**Direct.** Your device and theirs connect to each other when your networks allow it. That is a
little faster and takes load off the server. To find the way, your app asks your own server's
address lookup (no other company is asked). **The people in the call can see your network
address.** When no direct route works, the call still goes through the server.

**Direct, also asking a public service.** As Direct, and your app also asks a public address-lookup
service you pick from a list, such as Google's or Cloudflare's. That finds a direct route on more
networks. **That service sees your network address each time you start a call or join a voice
room**, as well as the people in the call. It never hears your calls. Choose this only if you are
fine with that company seeing your address.

## What the encryption rests on

In every choice the sound is encrypted between the people in the call. The keys for that are
agreed through the server's chat connection when the call starts, so the protection relies on the
server passing them on unchanged. A server that tampered with them could listen in; signing the
call setup with each person's own key, so a change would be noticed, is planned and not built yet.

## Your choice covers only you

Each person's choice decides only what happens to their own address. If you are set to Private
and you call someone set to Direct, the call still connects, through the server, and your address
stays hidden from them; theirs is visible to you because they chose that. Two people who both
chose a Direct option can connect straight to each other.

## When the server cannot carry calls

Some servers have not opened the port their call relay needs yet. Then:

- set to **Private**, a call or voice room cannot connect, and the app says so: "Calls go through
  the server to keep your address private; this server is not set up for that yet."
- set to a **Direct** option, the call connects device to device where your networks allow it,
  and the app tells you once that the people in it can see your network address.

## The protected setup

With the protected setup on (Settings > Safety, the setup for children and anyone who wants the
strictest settings), calls and voice rooms are always Private, whatever is chosen, and the other
choices cannot be picked.

## Where this is built

For anyone who wants to check: the desktop app's side is `src/net/call_connection.rs` and
`src/net/webrtc.rs`, the web's is `web/shared/call-connection.js` and
`web/chat/chat-voice-rooms.js`, and the words both apps show come from
`data/net/call_connection.json`. The design, with the operator's question that started it, is
section 10p of `docs/design/blocking-and-safe-mode.md`; how a server opens the port is
`docs/admin/call-forwarder.md`.
