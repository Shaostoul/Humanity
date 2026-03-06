# Market Integration Architecture (Legal + Practical)

## Goal
Enable in-game shopping and media/service access through mall kiosks and home VR terminals **without violating laws, platform policies, or partner terms**.

---

## Core Use Cases

1. **In-game Mall Kiosks**
   - Players walk to branded kiosks to browse approved partners.
   - Kiosk opens partner experience using permitted integration path.

2. **Home VR Access (Private)**
   - Players can access personal services (e.g., streaming account) from home modules.
   - Access method must respect DRM, auth, and provider embedding restrictions.

3. **Website Directory / Fenced Access**
   - Curated allowlisted websites by category.
   - Optional sandboxing/fencing policies between web sessions.

---

## Compliance-First Integration Modes

Each partner/site is assigned one mode:

- `embedded_ok`
  - In-game webview/embed allowed by provider policies.
- `external_launch_required`
  - Must open system browser/app/deep link for compliance.
- `api_only`
  - Data shown via licensed API; checkout/playback still provider-side.
- `manual_listing_only`
  - Non-integrated listing with outbound link only.
- `blocked`
  - Not allowed due to legal/policy/technical constraints.

No feature should attempt to bypass provider restrictions (DRM, X-Frame, auth flow, etc.).

---

## Architecture Layers

## 1) Partner Catalog Layer
- Canonical partner entries:
  - name, category, region support, integration mode
  - affiliate/tracking metadata
  - logo/branding policy refs

## 2) Experience Router
- Chooses execution path at runtime:
  - embed view
  - in-game browser shell
  - external app/browser launch

## 3) Attribution & Revenue Layer
- Affiliate/campaign tracking IDs
- Click/session conversion event logging
- Per-partner disclosure requirements

## 4) Policy Engine
- Region gating
- Age/content restrictions
- Partner-specific terms gates
- Feature flags for staged rollout

## 5) Session Fencing / Sandbox Controls
- Optional isolated web sessions per site/partner
- Cookie/storage partitioning policy
- Permission guards (camera/mic/location/download)

---

## Kiosk Model (Mall)

Kiosk = themed launcher + policy envelope.

### Kiosk behavior
1. Player interacts with kiosk.
2. Kiosk displays approved services/products.
3. Router determines allowed integration mode.
4. User is routed via compliant path.
5. Attribution event is logged.

### Kiosk types
- Retail
- Learning
- Media/Streaming
- Tools/Services
- Travel/Logistics

---

## Home VR Service Access

Home module (Private) can provide personal-service launch cards, but:
- Must use provider-approved rendering and auth flows.
- DRM/media playback limitations must be honored.
- If embed blocked, route to external app/browser seamlessly.

---

## Website List + Fencing Model

## Allowlist schema
- domain
- category
- trust level
- integration mode
- region constraints
- parent/age rating
- permission profile

## Fencing policy examples
- `strict_isolated`: separate storage/session for each site
- `category_shared`: shared state within category only
- `trusted_shared`: shared state for vetted partners

Recommended default: `strict_isolated` for safety/privacy.

---

## Security & Legal Requirements

1. Do not circumvent DRM, geofencing, licensing, or auth constraints.
2. Preserve provider branding and attribution requirements.
3. Provide user-visible disclosures for affiliate relationships.
4. Enforce regional/legal restrictions.
5. Maintain audit log of integration mode decisions and policy gates.

---

## MVP Build Order

1. Partner catalog + mode router
2. Mall kiosk framework (UI + routing)
3. Basic allowlist + fenced browser profiles
4. Attribution pipeline
5. Home VR launch cards with external fallback
6. Region/policy enforcement expansion

---

## Immediate Next Actions

- Define first 10 pilot partners/sites by category.
- Assign each pilot an integration mode.
- Implement kiosk prototype in mall tab with mode-aware routing.
- Add admin UI to manage allowlist and fencing profiles.
