# Communication and Association

## Purpose
This document defines the non-negotiable human rules for communication systems in Humanity OS.
These rules exist to protect voluntary association, minimize abuse, and preserve long-term social health.
All technical designs must comply with this document.

## Scope
Applies to:
- Forums, chat, direct messages, notifications, and community spaces.
- Web client, desktop client, game client, and offline-first storage.
- Centralized, hybrid, and peer-to-peer transport modes.

Does not define implementation details. Those belong in design documentation.

## Core Principles

### Voluntary participation
- Participation is voluntary at every layer: platform, space, channel, and conversation.
- Leaving a space must always be possible for a member.
- Blocking and muting must be available at the personal level.
- No system may require continued participation to preserve personal dignity or safety.

### Consent in contact
- Direct messages must not be forced.
- A user must be able to restrict who can contact them.
- A user must be able to close contact pathways without escalating to moderators.

### Clear boundaries of authority
- Authority must be explicit and attributable.
- Every space must declare who can moderate and what powers they hold.
- Users must be able to see the rules that govern a space before participating.

### Due process inside spaces
- A space must define how moderation decisions are made and appealed.
- Moderation actions must be attributable to an accountable authority within that space.
- The default posture should be proportional response:
  - warn, limit, or quarantine before removal where possible
  - remove immediately only for severe harm or repeated violations

### Forking is legitimate
- If a space becomes untrustworthy or hostile, members must be able to leave and form alternatives.
- A space may set its own membership rules, but cannot claim absolute authority over people.
- The platform must not punish users for creating alternatives.

### Transparency without surveillance
- Rules and authority must be transparent.
- Personal behavior monitoring must be minimized.
- The platform must not require invasive identity proofs for basic participation.
- Collection of personal data must be limited to what is necessary for safety, integrity, and operation.

### Privacy as dignity
- Private communication must remain private by design when feasible.
- The platform must minimize metadata exposure where feasible:
  - avoid public exposure of private relationship graphs
  - avoid unnecessary disclosure of who talked to whom, when, and how often
- Private spaces must support access control and confidentiality.

### Safety against abuse
- The platform must include protections against:
  - spam and automated harassment
  - impersonation and identity spoofing
  - coordinated abuse and brigading
  - malicious content distribution
- These protections must not require collective punishment of good actors.

### Non-coercive engagement
- The platform must not use manipulative engagement mechanics as governance.
- No dark patterns.
- No forced feeds that amplify outrage as a default.
- Ranking and recommendation systems must be optional, inspectable, and avoid harm incentives.

## Platform Requirements Derived From These Principles
These are binding outcomes that implementation must satisfy:

1. Users can leave spaces and control contact.
2. Spaces publish readable rules and declare authority.
3. Moderation actions are attributable within a space.
4. Proportional response is the default; severe harm allows immediate removal.
5. Privacy protection is a first-class requirement, not an afterthought.
6. Anti-abuse protections exist and are adjustable per space without punishing everyone.

## Conflicts and precedence
If a space rule conflicts with this document, this document takes precedence for platform-wide behavior.
Spaces may be stricter, but not coercive or invasive beyond necessity.

## Change control
Changes to this document require:
- clear justification
- explicit impacts on user rights and safety
- corresponding updates in technical design documents
