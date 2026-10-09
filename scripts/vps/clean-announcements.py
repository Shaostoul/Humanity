#!/usr/bin/env python3
"""Make #announcements one post per release (operator, 2026-10-09).

The channel held 5,325 bot posts: a "Deployed <sha>" post for every push to main (docs pushes and
re-runs included, 455 of them exact repeats), a "Native Desktop vX built" post for every release
(posted even when the build failed), and 73 empty "[Watchdog] Relay recovered" posts. This keeps
ONE post per release, with the release's own title and a link to its notes:

    keep:   per release tag (pre-releases and stray tags excluded), the earliest "built" post,
            else the earliest "Deployed" post whose commit is that version's;
    delete: everything else (other deploy posts, repeats, the Watchdog posts), and the reactions
            and pins of what was deleted;
    rewrite the kept post's text (and the copy in raw_json) to
            "🚀 **vX.Y.Z**: <title> · [Release notes](<link>)".

Every row of the channel is saved to a JSON file first. Usage:
    scripts/vps/clean-announcements.py <relay.db> <tag-titles.tsv> <prerelease.txt> <backup.json> [--apply]
Without --apply it only reports what it would do. Running it again changes nothing (posts it
already rewrote are kept as they are), so it is safe to rerun if an old-style notice appears.
Run 2026-10-09: 5,325 posts became 2,354; the saved rows are in /root/announcements-cleanup/.

The two lists come from a clone with every tag fetched:
    git for-each-ref 'refs/tags/v*' --format='%(refname:short)%09%(subject)' > tag-titles.tsv
    gh api --paginate "repos/Shaostoul/Humanity/releases?per_page=100" \
        --jq '.[] | select(.prerelease) | .tag_name' > prerelease.txt
Since 2026-10-09 the Build Desktop App workflow posts one post per release in this same format,
and the deploy workflow posts nothing, so there should be nothing left for this to do.
"""
import json
import re
import sqlite3
import sys

db_path, titles_path, pre_path, backup_path = sys.argv[1:5]
apply = "--apply" in sys.argv

titles = {}
for line in open(titles_path, encoding="utf-8"):
    line = line.rstrip("\r\n")
    if "\t" not in line:
        continue
    tag, subject = line.split("\t", 1)
    # "v0.1464.0: build on your own plot" -> "build on your own plot"
    m = re.match(r"^" + re.escape(tag) + r"\s*:\s*(.+)$", subject)
    titles[tag] = (m.group(1) if m else subject).strip()
pre = {l.strip() for l in open(pre_path, encoding="utf-8") if l.strip()}

con = sqlite3.connect(db_path, timeout=30)
con.row_factory = sqlite3.Row
rows = [dict(r) for r in con.execute(
    "SELECT * FROM messages WHERE channel_id = 'announcements' ORDER BY timestamp, id")]
json.dump(rows, open(backup_path, "w", encoding="utf-8"), ensure_ascii=False)

# A post this script already rewrote: kept first, so running it again changes nothing.
NEW = re.compile("^\U0001F680 " + r"\*\*(v[0-9][0-9.]*)\*\*: ")
BUILT = re.compile(r"Native Desktop (v[0-9][0-9.]*) built")
DEPLOYED = re.compile(r"\*\*Deployed\*\* `[0-9a-f]+` by [^:]+: (v[0-9]+\.[0-9]+\.[0-9]+)\b")

new, built, deployed = {}, {}, {}
for r in rows:
    if r["from_name"] != "Deploy Bot":
        continue
    m = NEW.search(r["content"] or "")
    if m:
        new.setdefault(m.group(1), r)
        continue
    m = BUILT.search(r["content"] or "")
    if m:
        built.setdefault(m.group(1), r)
        continue
    m = DEPLOYED.search(r["content"] or "")
    if m:
        deployed.setdefault(m.group(1), r)

keep = {}
for tag in set(new) | set(built) | set(deployed):
    if tag in pre:
        continue
    keep[tag] = new.get(tag) or built.get(tag) or deployed[tag]

def text_for(tag, row):
    title = titles.get(tag)
    if title is None:
        # No git tag of that name: take the commit message the deploy post carried.
        m = re.search(r"by [^:]+: " + re.escape(tag) + r"\s*:?\s*(.*)$", row["content"] or "")
        title = (m.group(1).strip() if m else "") or "released"
        return f"🚀 **{tag}**: {title}"
    return f"🚀 **{tag}**: {title} · [Release notes](<https://github.com/Shaostoul/Humanity/releases/tag/{tag}>)"

keep_ids = {r["id"] for r in keep.values()}
delete = [r for r in rows if r["id"] not in keep_ids]
by_name = {}
for r in delete:
    by_name[r["from_name"]] = by_name.get(r["from_name"], 0) + 1
print(f"rows in channel: {len(rows)}")
print(f"keep (one per release): {len(keep)}  ({sum(1 for t in keep if t in built)} from build posts, "
      f"{sum(1 for t in keep if t not in built)} from deploy posts)")
print(f"delete: {len(delete)}  by sender: {by_name}")
print(f"excluded pre-release tags seen: {sorted(t for t in (set(new) | set(built) | set(deployed)) if t in pre)}")
for tag in sorted(keep, key=lambda t: keep[t]["timestamp"])[-3:]:
    print("  newest:", text_for(tag, keep[tag]))
for tag in sorted(keep, key=lambda t: keep[t]["timestamp"])[:2]:
    print("  oldest:", text_for(tag, keep[tag]))

if not apply:
    print("dry run: nothing changed")
    sys.exit(0)

con.execute("PRAGMA secure_delete = ON")
with con:
    for r in delete:
        con.execute("DELETE FROM messages WHERE id = ?", (r["id"],))
        con.execute("DELETE FROM reactions WHERE target_from = ? AND target_timestamp = ?",
                    (r["from_key"], r["timestamp"]))
        con.execute("DELETE FROM pinned_messages WHERE channel = 'announcements' AND from_key = ? AND original_timestamp = ?",
                    (r["from_key"], r["timestamp"]))
    for tag, r in keep.items():
        text = text_for(tag, r)
        raw = r["raw_json"]
        try:
            j = json.loads(raw) if raw else None
            if isinstance(j, dict):
                j["content"] = text
                raw = json.dumps(j, ensure_ascii=False, separators=(",", ":"))
        except ValueError:
            pass
        con.execute("UPDATE messages SET content = ?, raw_json = ? WHERE id = ?", (text, raw, r["id"]))
left = con.execute("SELECT count(*) FROM messages WHERE channel_id = 'announcements'").fetchone()[0]
print(f"applied: {left} posts left in #announcements")
