# Projects System

Projects group tasks by category. Scopes (cosmos/civilization/community/self) handle **scale**; projects handle **what**.

## Data Model

### Schema

```sql
CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT DEFAULT '',
    owner_key TEXT NOT NULL,
    visibility TEXT DEFAULT 'public',  -- public, private, members-only
    color TEXT DEFAULT '#4488ff',
    icon TEXT DEFAULT '',
    created_at TEXT NOT NULL
);

ALTER TABLE tasks ADD COLUMN project TEXT DEFAULT 'default';
CREATE INDEX idx_tasks_project ON tasks(project);
```

Every server has an implicit `default` project (never stored in the table, always exists). Tasks without an explicit project land here.

### Visibility rules

| Visibility | Who sees it |
|------------|-------------|
| `public` | Anyone on the server |
| `members-only` | Owner + explicitly added members (future: `project_members` table) |
| `private` | Owner only |

Private projects and their tasks are excluded from all queries for non-owners. This is enforced server-side — the client never receives data it shouldn't.

## API

### Project endpoints

```
GET    /api/projects          → list visible projects (filtered by caller's key)
POST   /api/projects          → create project (authenticated)
PATCH  /api/projects/{id}     → update project (owner only)
DELETE /api/projects/{id}     → delete project + reassign tasks to 'default' (owner only)
```

### Task endpoint changes

Existing task routes gain a `project` query parameter:

```
GET  /api/tasks?project=abc123        → tasks in project
POST /api/tasks  { ..., project: "abc123" }
```

If `project` is omitted on GET, return tasks across all visible projects. If omitted on POST, default to `'default'`.

### Request/response shapes

**POST /api/projects**
```json
{
  "name": "Car Restoration",
  "description": "1967 Mustang rebuild",
  "visibility": "private",
  "color": "#e85d04",
  "icon": ""
}
```

**GET /api/projects response**
```json
[
  {
    "id": "proj_a1b2c3",
    "name": "Car Restoration",
    "description": "1967 Mustang rebuild",
    "owner_key": "abc123...",
    "visibility": "private",
    "color": "#e85d04",
    "icon": "",
    "created_at": "2026-03-20T00:00:00Z",
    "task_count": 14
  }
]
```

`task_count` is computed via subquery — no denormalized counter.

## WebSocket Messages

| Message | Direction | Fields |
|---------|-----------|--------|
| `project_list` | server → client | `projects: [...]` |
| `project_create` | client → server → broadcast | `name, description, visibility, color, icon` |
| `project_update` | client → server → broadcast | `id, <changed fields>` |
| `project_delete` | client → server → broadcast | `id` |

`task_create` and `task_update` gain a `project` field. Clients subscribed to a project they can't see (race condition on visibility change) get filtered out server-side before broadcast.

## Storage (Rust)

Add `server/src/storage/projects.rs`:

```rust
impl Storage {
    pub fn create_project(&self, id: &str, name: &str, description: &str,
        owner_key: &str, visibility: &str, color: &str, icon: &str) -> Result<()>;
    pub fn get_projects_visible_to(&self, viewer_key: &str) -> Result<Vec<Project>>;
    pub fn update_project(&self, id: &str, owner_key: &str, updates: ProjectUpdate) -> Result<()>;
    pub fn delete_project(&self, id: &str, owner_key: &str) -> Result<()>;
}
```

Visibility query:
```sql
SELECT p.*, COUNT(t.id) as task_count
FROM projects p
LEFT JOIN tasks t ON t.project = p.id
WHERE p.visibility = 'public'
   OR p.owner_key = ?1
GROUP BY p.id
ORDER BY p.name
```

## UI

### Project selector (task board)

```
┌──────────────────────────────────────────┐
│ [Project ▾: Car Restoration]  [+ New]    │  ← dropdown + create button
│                                          │
│  Scope: [cosmos] [civ] [community] [self]│  ← existing scope filter
│                                          │
│  ┌─────────┐ ┌──────────┐ ┌──────────┐  │
│  │ Backlog │ │ In Prog  │ │  Done    │  │  ← existing columns
│  │         │ │          │ │          │  │
│  │ task 1  │ │ task 4   │ │ task 7   │  │
│  │ task 2  │ │ task 5   │ │          │  │
│  │ task 3  │ │ task 6   │ │          │  │
│  └─────────┘ └──────────┘ └──────────┘  │
└──────────────────────────────────────────┘
```

The dropdown shows:
- "All Projects" (no filter)
- "Default" (uncategorized tasks)
- Each visible project, prefixed with its icon and colored dot

### Create/edit project dialog

```
┌─ New Project ─────────────────┐
│ Name:  [___________________]  │
│ Desc:  [___________________]  │
│ Color: [■ #4488ff] [pick]     │
│ Icon:  [📋] [pick]            │
│ Visibility: (•) Public        │
│              ( ) Private       │
│              ( ) Members Only  │
│                                │
│       [Cancel]  [Create]       │
└────────────────────────────────┘
```

### State management

```js
// New global
let activeProject = null;  // null = all projects, 'default' = uncategorized

// Project list cached locally
let projects = [];

// Filter tasks by project
function getVisibleTasks(tasks) {
    let filtered = tasks;
    if (activeProject !== null) {
        filtered = filtered.filter(t => (t.project || 'default') === activeProject);
    }
    // existing scope filter applies on top
    return filterByScope(filtered);
}
```

### Roadmap page

The roadmap page (`web/pages/tasks.html`) gets the same project dropdown. Selecting a project filters the roadmap view to that project's tasks only.

## Migration path

1. **Server**: Add `projects` table + `project` column to `tasks` (SQLite `ALTER TABLE` — no downtime needed)
2. **Server**: Add storage functions, API routes, WS message handlers
3. **UI**: Add project selector dropdown + create dialog to task board
4. **UI**: Wire up project filtering in task list and roadmap

Existing tasks get `project = 'default'` automatically via the column default. No data migration needed.

## Future extensions

- `project_members` table for `members-only` access control
- Project-level permissions (admin, contributor, viewer)
- Cross-server project federation
- Project templates (preset task lists for common workflows)
- Project archival (hide without deleting)
