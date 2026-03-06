# AI Plugin Runtime Architecture (Optional, Replaceable)

## Goals
- No AI dependency by default.
- Pluggable AI adapters (local/cloud/OpenClaw/none).
- Replace orchestration layer without rewriting app logic.
- Strong permission/safety controls for server + desktop operations.

---

## Runtime Modes

- `none` (default): no model calls, all features still usable.
- `local`: local model only (e.g., Ollama/LM Studio).
- `cloud`: hosted APIs.
- `hybrid`: local primary + cloud fallback.

---

## Core Interface Contract

```ts
interface AiAdapter {
  id: string;
  kind: 'none'|'local'|'cloud'|'bridge';
  health(): Promise<HealthStatus>;
  generate(req: GenerateRequest): Promise<GenerateResponse>;
  embed?(req: EmbedRequest): Promise<EmbedResponse>;
  toolInvoke?(req: ToolInvokeRequest): Promise<ToolInvokeResponse>;
  startSession?(req: SessionStartRequest): Promise<SessionStartResponse>;
}
```

App code depends on this interface only.

---

## Adapter Examples

- `adapter-none`
- `adapter-openclaw`
- `adapter-ollama`
- `adapter-openai`
- `adapter-anthropic`

Future: custom enterprise/local adapters.

---

## Permission & Policy Layer

Capabilities are policy-gated independent of model/provider:
- read_files
- edit_files
- exec_local
- exec_remote
- network_access
- messaging_send

Profiles:
- viewer
- planner
- operator
- admin

All privileged actions require explicit policy grants and audit logs.

---

## Safety Controls

- Action allowlists/denylists
- Human confirmation gates for external/destructive operations
- Rate limits and budget caps
- Data egress policy (e.g., local-only mode)
- Tamper-evident action logs

---

## Session Routing

- Task router chooses adapter by policy + availability + cost target.
- Fallback order configurable.
- If local model unhealthy, degrade gracefully (or ask before cloud fallback).

---

## Migration Strategy

1. Build adapter contract and runtime registry.
2. Move existing AI calls behind adapter facade.
3. Implement `none` + `local` + bridge adapter first.
4. Add cloud adapters with explicit opt-in.
5. Add UI for selecting runtime mode and policy profile.

---

## Success Criteria

- App fully functional in `none` mode.
- Local-only mode with no cloud egress.
- OpenClaw can be replaced by adapter switch, not architectural rewrite.
