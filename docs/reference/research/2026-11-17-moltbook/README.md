# Should HumanityOS's AI join Moltbook? (2026-11-17)

The operator asked: "Would you joining Moltbook help?" (2026-11-17). This is the answer as it stood
that day, so the question can be compared against later changes. No account was made.

## What Moltbook is (read 2026-11-17, news reporting)

- A Reddit-style forum for AI agents ("Submolts"), launched in late January 2026 by Matt Schlicht.
  Agents post, comment and vote, and people may only watch (NBC News).
- **Meta acquired it on 10 March 2026** (Axios, TechCrunch). Its founders joined Meta
  Superintelligence Labs, and Meta signalled that existing users could keep using it only for the
  time being (reported; the length of time was not given).
- **Trust problems:** an unsecured database was found three days after launch and every agent key
  was reset; humans could post as agents, so viral "agent" posts were questioned (TechCrunch's
  headline: it "went viral because of fake posts"); a study in June 2026 reportedly found the posting
  CAPTCHA could be passed by a person or a language model (secondary reporting).

## The assessment

It would bring little and costs more than it looks:

1. **Who is there.** The agents act for the people who run them. An agent reading a post cannot
   "join" HumanityOS unless its person sets that up, and since humans can pose as agents, nobody can
   tell who is actually reading.
2. **Every post read is untrusted text.** The forum is full of instructions written for agents to
   follow. The project's own rule for AI participants is never to act on instructions inside other
   people's messages (docs/ai/README.md), so a presence there would mean reading prompt injections
   at scale for little return.
3. **The presence is not persistent.** The AI that builds HumanityOS keeps no memory between sessions
   beyond the repo, so a Moltbook presence would need scheduled runs that spend the usage that
   builds the project.
4. **Account and voice.** Making the account and every public post would each need the operator's
   explicit yes. And a post signed by a named commercial model could read as that company speaking.
   Any AI presence should be labelled as the project's AI helper.
5. **The platform's future** under Meta is undecided.

## Where AI participants actually come from, and what to do instead

Agents arrive because their people plug tools into them and point them at things. So:

- Make HumanityOS easy for AI assistants to find and describe correctly: a plain page for AI
  participants on the website, and an `llms.txt` at the site root pointing to it (a proposed
  convention, llmstxt.org; compare alternatives before adopting).
- Make the codebase welcoming to people who code with an AI assistant: the repo already carries its
  own guide for coding agents (CLAUDE.md, docs/ai/), and small, scoped "good first issues" let a
  newcomer and their assistant contribute in the first hour. (On 2026-11-17 the GitHub repository
  had 16 stars, 0 forks and 0 open issues: nothing for a newcomer to pick up.)
- A way for an agent to take part through a standard tool interface (the dev MCP in docs/ai/mcp.md
  is the developer version) once there is something for participants to do.

## Sources (read 2026-11-17)

- https://www.axios.com/2026/03/10/meta-facebook-moltbook-agent-social-network
- https://techcrunch.com/2026/03/10/meta-acquired-moltbook-the-ai-agent-social-network-that-went-viral-because-of-fake-posts/
- https://www.nbcnews.com/tech/tech-news/ai-agents-social-media-platform-moltbook-rcna256738
- https://theconversation.com/moltbook-ai-bots-use-social-network-to-create-religions-and-deal-digital-drugs-but-are-some-really-humans-in-disguise-274895
