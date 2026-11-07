// The browser half of `just verify-browser-webrtc`: takes orders from the test
// (src/gui/crossing_tests/webrtc_browser.rs) over the page's chat socket and carries them out with the
// WEBSITE'S OWN functions (chat-voice-calls.js startCall / acceptIncomingCall / hangupCall /
// toggleMute, chat-file-transfer.js offerFileDirect / ingestFileNote / fileTransferActions), then
// reports what the browser itself measured (RTCPeerConnection.getStats, the transfers' states).
//
// Orders arrive as {"type":"harness_do","what":...}; everything else on the socket is the relay's
// traffic (the test plays the relay) and goes down the chat page's handleMessage chain, as on the
// real page. Reports go back as {"type":"harness_<what>",...}.

(function () {
  'use strict';

  const params = new URLSearchParams(location.search);
  const wsUrl = params.get('ws');

  function send(obj) {
    try {
      if (ws && ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify(obj));
    } catch (e) {
      console.error('harness: could not send ' + (obj && obj.type) + ': ' + (e && e.message));
    }
  }

  // Every error in the page reaches the test (errors.js keeps them from the first script on): a
  // stand-in that is missing, or a website script that threw, is never silent.
  const reportError = (text) => send({ type: 'harness_error', message: text });

  /** The same made-up bytes the test makes (crossing_tests/webrtc_browser.rs `file_bytes`). */
  function fileBytes(len, seed) {
    const out = new Uint8Array(len);
    for (let i = 0; i < len; i++) out[i] = (((Math.imul(i, 0x9E3779B1) >>> 0) >>> 13) ^ seed) & 0xff;
    return out;
  }

  /** What one RTCPeerConnection's own statistics say: its transport, its chosen path, its audio. */
  async function statsOf(pc) {
    if (!pc) return null;
    const report = await pc.getStats();
    const byId = new Map();
    report.forEach((s) => byId.set(s.id, s));
    const out = { connectionState: pc.connectionState, iceConnectionState: pc.iceConnectionState, iceGatheringState: pc.iceGatheringState, candidates: [], pairs: [] };
    report.forEach((s) => {
      if (s.type === 'local-candidate' || s.type === 'remote-candidate') {
        out.candidates.push(`${s.type === 'local-candidate' ? 'local' : 'remote'} ${s.candidateType} ${s.address}:${s.port}${s.relayProtocol ? ' via ' + s.relayProtocol : ''}${s.url ? ' ' + s.url : ''}`);
      } else if (s.type === 'candidate-pair') {
        const l = byId.get(s.localCandidateId);
        const r = byId.get(s.remoteCandidateId);
        out.pairs.push(`${l ? l.candidateType + ' ' + l.address + ':' + l.port : '?'} -> ${r ? r.candidateType + ' ' + r.address + ':' + r.port : '?'}: ${s.state}, ${s.requestsSent || 0} checks sent, ${s.responsesReceived || 0} answered, ${s.requestsReceived || 0} received`);
      }
      if (s.type === 'inbound-rtp' && s.kind === 'audio') {
        const codec = byId.get(s.codecId);
        out.inbound = {
          packetsReceived: s.packetsReceived, bytesReceived: s.bytesReceived, packetsLost: s.packetsLost,
          totalAudioEnergy: s.totalAudioEnergy, audioLevel: s.audioLevel, totalSamplesReceived: s.totalSamplesReceived,
          concealedSamples: s.concealedSamples, codec: codec ? codec.mimeType : null,
        };
      } else if (s.type === 'outbound-rtp' && s.kind === 'audio') {
        out.outbound = { packetsSent: s.packetsSent, bytesSent: s.bytesSent };
      } else if (s.type === 'data-channel') {
        (out.channels = out.channels || []).push({ label: s.label, state: s.state, bytesSent: s.bytesSent, bytesReceived: s.bytesReceived });
      } else if (s.type === 'transport') {
        const pair = byId.get(s.selectedCandidatePairId);
        const local = pair ? byId.get(pair.localCandidateId) : null;
        const remote = pair ? byId.get(pair.remoteCandidateId) : null;
        out.transport = {
          dtlsState: s.dtlsState, dtlsRole: s.dtlsRole, tlsVersion: s.tlsVersion, dtlsCipher: s.dtlsCipher,
          srtpCipher: s.srtpCipher, iceState: s.iceState,
          local: local ? { type: local.candidateType, protocol: local.protocol, relayProtocol: local.relayProtocol, address: local.address, port: local.port } : null,
          remote: remote ? { type: remote.candidateType, address: remote.address, port: remote.port } : null,
        };
      }
    });
    return out;
  }

  function transfersNow() {
    return (typeof fileTransfers === 'function' ? fileTransfers() : []).map((t) => ({
      id: t.id, role: t.role, state: t.state, reason: t.reason || null, done: t.done || 0, pieces: t.offer ? t.offer.pieces : 0,
      blake3: t.offer ? t.offer.blake3 : null, name: t.name, size: t.size,
    }));
  }

  /** The file links' connections (chat-file-transfer.js ftLinks), by peer. */
  async function linkStats() {
    const out = [];
    if (typeof ftLinks === 'undefined') return out;
    for (const [peer, link] of ftLinks) {
      out.push({ peer: peer.slice(0, 12), role: link.role, open: link.open, closed: link.closed, path: link.path, stats: link.pc ? await statsOf(link.pc) : null });
    }
    return out;
  }

  async function act(msg) {
    switch (msg.what) {
      case 'peer':
        window.harnessPeerKey = msg.key;
        window.hosServerDid = msg.server;
        peerData[msg.key] = { display_name: msg.name || 'Desktop app' };
        send({ type: 'harness_ready' });
        return;
      case 'call':
        startCall(msg.key, msg.name || 'Desktop app');
        return;
      case 'accept_call':
        acceptIncomingCall();
        return;
      case 'hangup':
        hangupCall();
        return;
      case 'mute':
        toggleMute();
        send({ type: 'harness_muted', muted: isMuted });
        return;
      case 'stats':
        send({ type: 'harness_stats', tag: msg.tag, callState, call: await statsOf(peerConnection), links: await linkStats() });
        return;
      case 'offer_file': {
        const bytes = fileBytes(msg.size, msg.seed);
        const file = new File([bytes], msg.name, { type: msg.mime || 'application/octet-stream' });
        const t = await offerFileDirect(file, msg.key);
        send({ type: 'harness_offered', id: t ? t.id : null, state: t ? t.state : null, reason: t ? t.reason : null });
        return;
      }
      case 'note':
        await ingestFileNote(msg.inner);
        send({ type: 'harness_noted', transfers: transfersNow() });
        return;
      case 'accept_file': {
        const t = fileTransfers().find((x) => x.id === msg.id);
        if (t) await fileTransferActions.accept(t);
        send({ type: 'harness_accepted', found: !!t, transfers: transfersNow() });
        return;
      }
      case 'transfers':
        send({ type: 'harness_transfers', tag: msg.tag, transfers: transfersNow(), links: await linkStats(), lines: harnessSystemLines.slice(-12) });
        return;
      default:
        send({ type: 'harness_error', message: 'harness: no such order: ' + msg.what });
    }
  }

  async function start() {
    // A real Dilithium3 identity, made the way the chat page makes one from its recovery phrase's seed.
    const seed = crypto.getRandomValues(new Uint8Array(32));
    const id = await pqDeriveIdentity(seed);
    if (!id) {
      console.error('harness: no identity: the PQ bundle did not load');
      return;
    }
    myKey = id.dilithiumPublicHex;
    myDilithiumPublicHex = myKey;
    myDilithiumSecret = id.dilithiumSecret;
    ws = new WebSocket(wsUrl);
    ws.onopen = () => {
      send({ type: 'harness_hello', key: myKey, ua: navigator.userAgent });
      window.harnessErrorSink(reportError);
    };
    ws.onmessage = (ev) => {
      let msg = null;
      try { msg = JSON.parse(ev.data); } catch (e) { return; }
      if (msg && msg.type === 'harness_do') {
        act(msg).catch((e) => send({ type: 'harness_error', message: `harness: ${msg.what} threw: ${(e && (e.stack || e.message)) || e}` }));
        return;
      }
      Promise.resolve()
        .then(() => handleMessage(msg))
        .catch((e) => send({ type: 'harness_error', message: `handleMessage(${msg && msg.type}) threw: ${(e && (e.stack || e.message)) || e}` }));
    };
    ws.onclose = () => console.log('harness: the test closed the socket');
  }

  start();
})();
