// The Model Context Protocol subset the dev bridge speaks (scripts/mcp-server.js): JSON-RPC 2.0
// with tools/list and tools/call, in both eras the specification has had. The same rules as the
// game's own endpoint (src/engine/mcp/protocol.rs), for the stdio side Claude Code talks to.
//
// Read 2026-10-22 from modelcontextprotocol.io and each revision's schema.ts:
//   LEGACY (2025-11-25 and earlier): initialize, notifications/initialized, ping, tools/*.
//   MODERN (2026-07-28): no handshake; every request names its version in
//     params._meta["io.modelcontextprotocol/protocolVersion"]; server/discover lists the
//     versions; an unsupported version is -32022 with {supported, requested}; every result
//     carries resultType, and tools/list ttlMs and cacheScope.
// A dual-era server answers both, per request, by how the client speaks.

"use strict";

const MODERN_VERSIONS = ["2026-07-28"];
const LEGACY_VERSIONS = ["2025-11-25", "2025-06-18", "2025-03-26", "2024-11-05"];
const META_VERSION = "io.modelcontextprotocol/protocolVersion";
const META_SERVER_INFO = "io.modelcontextprotocol/serverInfo";

const ERR = Object.freeze({
  PARSE_ERROR: -32700,
  INVALID_REQUEST: -32600,
  METHOD_NOT_FOUND: -32601,
  INVALID_PARAMS: -32602,
  HEADER_MISMATCH: -32020,
  UNSUPPORTED_PROTOCOL_VERSION: -32022,
});

const supportedVersions = () => [...MODERN_VERSIONS, ...LEGACY_VERSIONS];

/** The legacy version to answer initialize with: the client's own when spoken here, else the newest. */
function negotiateLegacy(asked) {
  return LEGACY_VERSIONS.includes(asked) ? asked : LEGACY_VERSIONS[0];
}

function error(id, code, message, data) {
  const e = { code, message };
  if (data !== undefined) e.data = data;
  return { jsonrpc: "2.0", id: id === undefined ? null : id, error: e };
}

/**
 * Handle one message. `host` is { tools(): [def], call(name, args, ctx): Promise<CallToolResult|null>
 * (null = no such tool), serverInfo: {name, version}, instructions: string }.
 * `ctx` reaches the tool: { progressToken, signal, notify(msg) }.
 * Resolves to the response object, or null for a notification (nothing to send).
 */
async function handle(msg, host, ctx = {}) {
  if (!msg || typeof msg !== "object" || Array.isArray(msg)) {
    return error(null, ERR.INVALID_REQUEST, "a JSON-RPC message must be one JSON object (batches are not part of MCP)");
  }
  if (msg.jsonrpc !== "2.0") return error(msg.id, ERR.INVALID_REQUEST, 'jsonrpc must be "2.0"');
  if (typeof msg.method !== "string") {
    return error(msg.id, ERR.INVALID_REQUEST, "expected a request or a notification (a message with a method)");
  }
  if (!("id" in msg)) return null; // a notification: initialized, cancelled, ...
  const id = msg.id;
  if (!(typeof id === "string" || Number.isInteger(id))) {
    return error(null, ERR.INVALID_REQUEST, "a request id must be a string or an integer");
  }
  const params = msg.params && typeof msg.params === "object" ? msg.params : {};
  const meta = params._meta && typeof params._meta === "object" ? params._meta : {};
  const modernVersion = typeof meta[META_VERSION] === "string" ? meta[META_VERSION] : null;
  if (modernVersion !== null && !MODERN_VERSIONS.includes(modernVersion)) {
    return error(id, ERR.UNSUPPORTED_PROTOCOL_VERSION, "Unsupported protocol version", {
      supported: supportedVersions(),
      requested: modernVersion,
    });
  }
  const modern = modernVersion !== null;
  const finish = (r) => (modern ? { ...r, resultType: "complete", _meta: { [META_SERVER_INFO]: host.serverInfo } } : r);
  const ok = (r) => ({ jsonrpc: "2.0", id, result: r });

  switch (msg.method) {
    case "initialize":
      return ok({
        protocolVersion: negotiateLegacy(params.protocolVersion),
        capabilities: { tools: { listChanged: false } },
        serverInfo: host.serverInfo,
        instructions: host.instructions,
      });
    case "ping":
      return ok(finish({}));
    case "server/discover":
      return ok({
        resultType: "complete",
        supportedVersions: supportedVersions(),
        capabilities: { tools: {} },
        instructions: host.instructions,
        ttlMs: 0,
        cacheScope: "private",
        _meta: { [META_SERVER_INFO]: host.serverInfo },
      });
    case "tools/list": {
      const r = { tools: host.tools() };
      if (modern) Object.assign(r, { ttlMs: 0, cacheScope: "private" });
      return ok(finish(r));
    }
    case "tools/call": {
      if (typeof params.name !== "string") return error(id, ERR.INVALID_PARAMS, "tools/call needs params.name, the tool's name");
      const args = params.arguments === undefined ? {} : params.arguments;
      if (!args || typeof args !== "object" || Array.isArray(args)) {
        return error(id, ERR.INVALID_PARAMS, "tools/call params.arguments must be an object");
      }
      let r;
      try {
        r = await host.call(params.name, args, { ...ctx, progressToken: meta.progressToken });
      } catch (e) {
        r = toolError(`${params.name} failed: ${e && e.message ? e.message : e}`);
      }
      if (r === null || r === undefined) return error(id, ERR.INVALID_PARAMS, `Unknown tool: ${params.name}`);
      return ok(finish(r));
    }
    default:
      return error(id, ERR.METHOD_NOT_FOUND, `Method not found: ${msg.method} (this server offers tools only: tools/list, tools/call)`);
  }
}

/** A tool's answer as text, optionally with structured content. */
function textResult(text, structured) {
  const r = { content: [{ type: "text", text: String(text) }], isError: false };
  if (structured !== undefined) r.structuredContent = structured;
  return r;
}

/** A tool's own failure, said plainly. */
function toolError(text) {
  return { content: [{ type: "text", text: String(text) }], isError: true };
}

/** A header value in the =?base64?...?= form when it is not plain header-safe ASCII (2026-07-28). */
function headerValue(v) {
  const s = String(v);
  const plain = /^[\x21-\x7e]([\x20-\x7e]*[\x21-\x7e])?$/.test(s) && !(s.startsWith("=?base64?") && s.endsWith("?="));
  return plain ? s : `=?base64?${Buffer.from(s, "utf8").toString("base64")}?=`;
}

module.exports = {
  MODERN_VERSIONS,
  LEGACY_VERSIONS,
  META_VERSION,
  META_SERVER_INFO,
  ERR,
  supportedVersions,
  negotiateLegacy,
  handle,
  textResult,
  toolError,
  headerValue,
};
