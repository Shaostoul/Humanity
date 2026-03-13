  // ══════════════════════════════════════
  // PROJECT BOARD
  // ══════════════════════════════════════

  let boardTasks = [];
  let boardWs = null;
  let boardMyKey = null;
  let boardMyRole = '';
  const BOARD_EDIT_ROLES = ['admin', 'mod'];
  const BOARD_COMMENT_ROLES = ['verified', 'donor'];

  function boardCanEdit() {
   return BOARD_EDIT_ROLES.includes(boardMyRole);
  }

  function boardCanComment() {
   return boardCanEdit() || BOARD_COMMENT_ROLES.includes(boardMyRole);
  }

  function boardIsConnected() {
   return !!boardWs && boardWs.readyState === 1;
  }

  function boardNeedsConnect() {
   return !boardWs || boardWs.readyState > 1;
  }

  function boardSend(payload) {
   if (!payload || !boardIsConnected()) return false;
   boardWs.send(JSON.stringify(payload));
   return true;
  }

  function boardNoticeConnectionNotReady() {
   featureWebNotice('Board connection not ready');
  }

  function boardSendOrNotice(payload) {
   if (boardSend(payload)) return true;
   boardNoticeConnectionNotReady();
   return false;
  }

  function boardNormalizeLabelsArray(labels) {
   const sourceLabels = Array.isArray(labels) ? labels : [];
   const normalized = [];
   const seen = new Set();
   sourceLabels.forEach(label => {
    const clean = featureWebClampText(String(label == null ? '' : label), 40).trim();
    if (!clean) return;
    const key = clean.toLowerCase();
    if (seen.has(key)) return;
    seen.add(key);
    normalized.push(clean);
   });
   return normalized;
  }

  function boardParseTaskLabels(task) {
   try { return boardNormalizeLabelsArray(JSON.parse((task && task.labels) || '[]')); } catch { return []; }
  }

  function boardLabelsToJson(labelsText) {
   const raw = typeof labelsText === 'string' ? labelsText.trim() : '';
   return JSON.stringify(boardNormalizeLabelsArray(raw ? raw.split(',') : []));
  }

  function boardNormalizeLabelsJson(labels) {
   return JSON.stringify(boardParseTaskLabels({ labels }));
  }

  function boardNormalizeStatus(status) {
   return typeof status === 'string' ? status.trim().toLowerCase() : '';
  }

  function boardNormalizeTaskStatus(status, fallback) {
   const normalizedStatus = boardNormalizeStatus(status);
   return VALID_STATUSES.includes(normalizedStatus) ? normalizedStatus : (fallback === undefined ? 'backlog' : fallback);
  }

  function boardTaskStatusLabel(status) {
   const normalizedStatus = boardNormalizeTaskStatus(status);
   return STATUS_LABELS[normalizedStatus] || normalizedStatus;
  }

  function boardNormalizePriority(priority) {
   const normalizedPriority = featureWebNormalizePriority(priority);
   return FW_ALLOWED_PRIORITIES.includes(normalizedPriority) ? normalizedPriority : 'medium';
  }

  function boardNormalizeAssignee(assignee) {
   const normalizedAssignee = featureWebClampText(assignee || '', 50).trim();
   return normalizedAssignee || null;
  }

  function boardNormalizeTitle(title) {
   return featureWebClampText(title || '', 200).trim();
  }

  function boardNormalizeDescription(description) {
   return featureWebClampText(description || '', FW_TEXT_LIMITS.details);
  }

  function boardNormalizeCommentContent(content) {
   return featureWebClampText(content || '', FW_TEXT_LIMITS.summary).trim();
  }

  function boardNormalizeCommentCount(commentCount) {
   const normalizedCommentCount = Number.isFinite(+commentCount) ? Math.max(0, Math.trunc(+commentCount)) : 0;
   return normalizedCommentCount;
  }

  function boardNormalizePosition(position) {
   return Number.isFinite(+position) ? Math.trunc(+position) : 0;
  }

  function boardTaskSortCompare(a, b) {
   const positionDelta = boardNormalizePosition(a && a.position) - boardNormalizePosition(b && b.position);
   if (positionDelta !== 0) return positionDelta;
   const aId = featureWebNormalizeTaskId(a && a.id);
   const bId = featureWebNormalizeTaskId(b && b.id);
   if (aId != null && bId != null && aId !== bId) return aId - bId;
   const aTitle = boardNormalizeTitle(a && a.title);
   const bTitle = boardNormalizeTitle(b && b.title);
   return aTitle.localeCompare(bTitle);
  }

  function boardNormalizeCreatedAt(createdAt) {
   const t = Date.parse(createdAt);
   return Number.isFinite(t) ? new Date(t).toISOString() : '';
  }

  function boardFormatDateTime(value, fallback) {
   const t = Date.parse(value);
   return Number.isFinite(t) ? new Date(t).toLocaleString() : (fallback === undefined ? 'Unknown' : fallback);
  }

  function boardNormalizeComment(comment) {
   const sourceComment = (comment && typeof comment === 'object') ? comment : {};
   return {
    author_name: featureWebClampText(sourceComment.author_name || '', FW_TEXT_LIMITS.owner) || 'Unknown',
    content: boardNormalizeCommentContent(sourceComment.content),
    created_at: boardNormalizeCreatedAt(sourceComment.created_at),
   };
  }

  function boardNormalizeComments(comments) {
   return (Array.isArray(comments) ? comments : []).map(boardNormalizeComment).filter(c => c.content);
  }

  function boardNormalizeTasks(tasks) {
   const rawTasks = Array.isArray(tasks) ? tasks : [];
   const dedupedTasks = [];
   const seenTaskIds = new Set();
   rawTasks.forEach(task => {
    const taskObj = (task && typeof task === 'object') ? { ...task } : null;
    if (!taskObj) return;
    const taskId = featureWebNormalizeTaskId(taskObj.id);
    if (taskId != null) {
     if (seenTaskIds.has(taskId)) return;
     seenTaskIds.add(taskId);
     taskObj.id = taskId;
    }
    taskObj.status = boardNormalizeTaskStatus(taskObj.status);
    taskObj.priority = boardNormalizePriority(taskObj.priority);
    taskObj.assignee = boardNormalizeAssignee(taskObj.assignee);
    taskObj.title = boardNormalizeTitle(taskObj.title) || 'Untitled Task';
    taskObj.description = boardNormalizeDescription(taskObj.description);
    taskObj.labels = boardNormalizeLabelsJson(taskObj.labels);
    taskObj.comment_count = boardNormalizeCommentCount(taskObj.comment_count);
    taskObj.position = boardNormalizePosition(taskObj.position);
    taskObj.created_at = boardNormalizeCreatedAt(taskObj.created_at);
    dedupedTasks.push(taskObj);
   });
   return dedupedTasks;
  }

  const PRIORITY_COLORS = { critical: '#e55', high: '#f80', medium: '#eb4', low: '#666' };
  const STATUS_LABELS = { backlog: '📥 Backlog', in_progress: '🔧 In Progress', testing: '🧪 Testing', done: '✅ Done' };
  const VALID_STATUSES = ['backlog', 'in_progress', 'testing', 'done'];

  const FEATURE_WEB_KEY = 'humanity_feature_web_v1';
  const FEATURE_WEB_FILTERS_KEY = 'humanity_feature_web_filters_v1';
  const FEATURE_WEB_UI_KEY = 'humanity_feature_web_ui_v1';
  const FW_ALLOWED_TYPES = ['feature', 'subfeature', 'tech', 'ux', 'org', 'lore'];
  const FW_ALLOWED_DOMAINS = ['vision', 'business', 'school', 'game', 'economy', 'tech', 'lore', 'roadmap', 'community', 'operations'];
  const FW_ALLOWED_STATUSES = ['planned', 'active', 'blocked', 'done'];
  const FW_ALLOWED_PRIORITIES = ['low', 'medium', 'high', 'critical'];
  const FW_ALLOWED_EDGE_TYPES = ['depends_on', 'blocks', 'relates_to', 'teaches', 'enables'];
  const FW_ALLOWED_VIEW_MODES = ['orbit', 'cards', 'constellation'];
  const FW_STATUS_HOTKEY_MAP = { '1': 'planned', '2': 'active', '3': 'blocked', '4': 'done' };
  const FW_TYPE_HOTKEY_MAP = { '5': 'feature', '6': 'subfeature', '7': 'tech', '8': 'ux', '9': 'org', '0': 'lore' };
  const FW_TASK_TO_NODE_STATUS_MAP = { backlog: 'planned', in_progress: 'active', testing: 'active', done: 'done' };
  const FW_NODE_TO_TASK_STATUS_MAP = { planned: 'backlog', active: 'in_progress', blocked: 'testing', done: 'done' };
  const FW_TEXT_LIMITS = { title: 140, owner: 120, summary: 400, teach: 1200, details: 6000, search: 200, id: 72 };
  const FW_MAX_NODES = 800;
  const FW_MAX_EDGES = 3200;
  const FW_STATUS_COLORS = { planned: '#6f7f95', active: '#2f8f4e', blocked: '#ba3f4a', done: '#3d7dd8' };
  const FW_TYPE_COLORS = { feature: '#ff8811', subfeature: '#cf6a13', tech: '#7b5cff', ux: '#18a8a8', org: '#7e6a4d', lore: '#a55ad9' };
  const FW_DOMAIN_COLORS = { vision:'#ffb347', business:'#f08c4a', school:'#4eb8e6', game:'#7dd56f', economy:'#f5c451', tech:'#8d7bff', lore:'#cf7cff', roadmap:'#f58e8e', community:'#6fd4c3', operations:'#9aa8b8' };
  let featureWeb = { nodes: [], edges: [] };
  let fwDrag = null;
  let fwSelectedNodeId = '';
  let fwHoveredNodeId = '';
  let fwLastVisibleNodeIds = [];
  let fwAdvancedVisible = true;
  let fwShortcutsVisible = true;
  let fwLastSavedAt = 0;

  function featureWebNewId(prefix) {
   return (prefix || 'id') + '_' + Date.now().toString(36) + Math.random().toString(36).slice(2, 7);
  }

  function featureWebNormalizeTaskId(value) {
   const n = Number(value);
   if (!Number.isFinite(n)) return null;
   const id = Math.trunc(n);
   if (!Number.isSafeInteger(id)) return null;
   return id > 0 ? id : null;
  }

  function featureWebFindTaskById(taskId) {
   const normalizedId = featureWebNormalizeTaskId(taskId);
   if (normalizedId == null) return null;
   return boardTasks.find(t => featureWebNormalizeTaskId(t && t.id) === normalizedId) || null;
  }

  function featureWebFindTaskIndexById(taskId) {
   const normalizedId = featureWebNormalizeTaskId(taskId);
   if (normalizedId == null) return -1;
   return boardTasks.findIndex(t => featureWebNormalizeTaskId(t && t.id) === normalizedId);
  }

  function featureWebFindTaskIndexByRawId(taskId) {
   return boardTasks.findIndex(t => t && t.id === taskId);
  }

  function featureWebFindTaskByRawId(taskId) {
   return boardTasks.find(t => t && t.id === taskId) || null;
  }

  function featureWebRemoveTaskByRawId(taskId) {
   const idx = featureWebFindTaskIndexByRawId(taskId);
   if (idx < 0) return false;
   boardTasks.splice(idx, 1);
   return true;
  }

  function featureWebUpsertBoardTask(task) {
   if (!task) return false;
   const normalizedTask = boardNormalizeTasks([task])[0];
   if (!normalizedTask) return false;
   const normalizedId = featureWebNormalizeTaskId(normalizedTask.id);
   const idx = normalizedId != null ? featureWebFindTaskIndexById(normalizedId) : featureWebFindTaskIndexByRawId(normalizedTask.id);
   if (idx >= 0) {
    boardTasks[idx] = normalizedTask;
    return true;
   }
   boardTasks.push(normalizedTask);
   return false;
  }

  function featureWebRemoveTaskById(taskId) {
   const idx = featureWebFindTaskIndexById(taskId);
   if (idx < 0) return false;
   boardTasks.splice(idx, 1);
   return true;
  }

  function featureWebBoardStatusLabel(status) {
   const s = boardNormalizeTaskStatus(status);
   return s === 'in_progress' ? 'IN PROGRESS' : (s === 'testing' ? 'TESTING' : (s === 'done' ? 'DONE' : 'BACKLOG'));
  }

  function featureWebMapTaskStatusToNode(status, fallback) {
   const s = boardNormalizeStatus(status);
   return Object.prototype.hasOwnProperty.call(FW_TASK_TO_NODE_STATUS_MAP, s)
    ? FW_TASK_TO_NODE_STATUS_MAP[s]
    : (fallback === undefined ? null : fallback);
  }

  function featureWebMapNodeStatusToTask(status, fallback) {
   const s = boardNormalizeStatus(status);
   return Object.prototype.hasOwnProperty.call(FW_NODE_TO_TASK_STATUS_MAP, s)
    ? FW_NODE_TO_TASK_STATUS_MAP[s]
    : (fallback === undefined ? 'backlog' : fallback);
  }

  function featureWebNormalizePriority(priority) {
   return typeof priority === 'string' ? priority.trim().toLowerCase() : '';
  }

  function featureWebNormalizeEdgeType(edgeType) {
   const normalizedEdgeType = typeof edgeType === 'string' ? edgeType.trim().toLowerCase() : '';
   return FW_ALLOWED_EDGE_TYPES.includes(normalizedEdgeType) ? normalizedEdgeType : 'depends_on';
  }

  function featureWebApplyTaskStatusToLinkedNodes(taskId, taskStatus) {
   if (taskId == null) return false;
   const mappedNodeStatus = featureWebMapTaskStatusToNode(taskStatus);
   if (!mappedNodeStatus) return false;
   let changed = false;
   featureWeb.nodes.forEach(node => {
    if (node && node.taskId === taskId && node.status !== mappedNodeStatus) {
     node.status = mappedNodeStatus;
     changed = true;
    }
   });
   return changed;
  }

  function featureWebSyncLinkedNodesFromBoardTasks(tasks) {
   if (!featureWeb.nodes.length) return false;
   const statusByTaskId = new Map();
   (Array.isArray(tasks) ? tasks : []).forEach(task => {
    const taskId = featureWebNormalizeTaskId(task && task.id);
    if (taskId == null || statusByTaskId.has(taskId)) return;
    const boardStatus = boardNormalizeTaskStatus(task.status);
    statusByTaskId.set(taskId, boardStatus);
   });

   let changed = false;
   featureWeb.nodes.forEach(node => {
    if (!node) return;
    const taskId = featureWebNormalizeTaskId(node.taskId);
    if (taskId == null) return;
    if (!statusByTaskId.has(taskId)) {
     node.taskId = null;
     changed = true;
     return;
    }
    const mappedNodeStatus = featureWebMapTaskStatusToNode(statusByTaskId.get(taskId));
    if (mappedNodeStatus && node.status !== mappedNodeStatus) {
     node.status = mappedNodeStatus;
     changed = true;
    }
   });
   return changed;
  }

  function featureWebClampText(value, maxLen) {
   if (typeof value !== 'string') return '';
   const text = value.trim();
   return text.length > maxLen ? text.slice(0, maxLen) : text;
  }

  function featureWebMakeUniqueTitle(baseTitle, fallbackBase, excludeNodeId) {
   const safeFallback = featureWebClampText(fallbackBase || 'Untitled', FW_TEXT_LIMITS.title) || 'Untitled';
   const base = featureWebClampText(baseTitle || '', FW_TEXT_LIMITS.title) || safeFallback;
   let candidate = base;
   let counter = 2;
   while (featureWeb.nodes.some(n => n && n.id !== excludeNodeId && featureWebClampText(n.title || '', FW_TEXT_LIMITS.title).toLowerCase() === candidate.toLowerCase())) {
    candidate = featureWebClampText(base + ' (' + counter + ')', FW_TEXT_LIMITS.title) || (safeFallback + ' (' + counter + ')');
    counter++;
   }
   return candidate;
  }

  function featureWebSanitizeId(value, maxLen) {
   if (typeof value !== 'string') return '';
   const cleaned = value.trim().replace(/[^a-zA-Z0-9_-]/g, '_');
   if (!cleaned) return '';
   const len = Number.isFinite(+maxLen) ? Math.max(1, Math.trunc(+maxLen)) : 64;
   return cleaned.length > len ? cleaned.slice(0, len) : cleaned;
  }

  function featureWebNormalize(raw, includeReport) {
   const src = (raw && typeof raw === 'object') ? raw : {};
   const sourceNodesAll = Array.isArray(src.nodes) ? src.nodes : [];
   const sourceEdgesAll = Array.isArray(src.edges) ? src.edges : [];
   const sourceNodes = sourceNodesAll.slice(0, FW_MAX_NODES);
   const sourceEdges = sourceEdgesAll.slice(0, FW_MAX_EDGES);
   const cappedNodes = Math.max(0, sourceNodesAll.length - sourceNodes.length);
   const cappedEdges = Math.max(0, sourceEdgesAll.length - sourceEdges.length);
   const allowedTypes = new Set(FW_ALLOWED_TYPES);
   const allowedDomains = new Set(FW_ALLOWED_DOMAINS);
   const allowedStatuses = new Set(FW_ALLOWED_STATUSES);
   const allowedPriorities = new Set(FW_ALLOWED_PRIORITIES);

   const nodes = [];
   const nodeIds = new Set();
   const nodeTitleKeys = new Set();
   const linkedTaskIds = new Set();
   let duplicateTaskLinksDropped = 0;
   sourceNodes.forEach((node, idx) => {
    if (!node || typeof node !== 'object') return;
    const fallbackX = 25 + (nodes.length % 4) * 150;
    const fallbackY = 20 + Math.floor(nodes.length / 4) * 90;
    const normalizedType = typeof node.type === 'string' ? node.type.trim().toLowerCase() : '';
    const normalizedDomain = typeof node.domain === 'string' ? node.domain.trim().toLowerCase() : '';
    const normalizedStatus = typeof node.status === 'string' ? node.status.trim().toLowerCase() : '';
    const normalizedPriority = featureWebNormalizePriority(node.priority);
    const normalized = {
     id: featureWebSanitizeId(node.id, FW_TEXT_LIMITS.id) || featureWebNewId('n'),
     title: featureWebClampText(node.title, FW_TEXT_LIMITS.title) || ('Untitled ' + (idx + 1)),
     type: allowedTypes.has(normalizedType) ? normalizedType : 'feature',
     domain: allowedDomains.has(normalizedDomain) ? normalizedDomain : 'game',
     status: allowedStatuses.has(normalizedStatus) ? normalizedStatus : 'planned',
     summary: featureWebClampText(node.summary, FW_TEXT_LIMITS.summary),
     teach: featureWebClampText(node.teach, FW_TEXT_LIMITS.teach),
     details: featureWebClampText(node.details, FW_TEXT_LIMITS.details),
     owner: featureWebClampText(node.owner, FW_TEXT_LIMITS.owner),
     priority: allowedPriorities.has(normalizedPriority) ? normalizedPriority : 'medium',
     x: Number.isFinite(+node.x) ? +node.x : fallbackX,
     y: Number.isFinite(+node.y) ? +node.y : fallbackY,
    };
    let titleKey = featureWebClampText(normalized.title, FW_TEXT_LIMITS.title).toLowerCase();
    if (!titleKey) {
     normalized.title = 'Untitled ' + (idx + 1);
     titleKey = normalized.title.toLowerCase();
    }
    let titleCounter = 2;
    while (nodeTitleKeys.has(titleKey)) {
     normalized.title = featureWebClampText((featureWebClampText(node.title, FW_TEXT_LIMITS.title) || ('Untitled ' + (idx + 1))) + ' (' + titleCounter + ')', FW_TEXT_LIMITS.title) || ('Untitled ' + (idx + 1) + ' (' + titleCounter + ')');
     titleKey = normalized.title.toLowerCase();
     titleCounter++;
    }
    nodeTitleKeys.add(titleKey);

    const normalizedTaskId = featureWebNormalizeTaskId(node.taskId);
    if (normalizedTaskId != null) {
     if (linkedTaskIds.has(normalizedTaskId)) {
      duplicateTaskLinksDropped++;
     } else {
      normalized.taskId = normalizedTaskId;
      linkedTaskIds.add(normalizedTaskId);
     }
    }
    while (nodeIds.has(normalized.id)) normalized.id = featureWebNewId('n');
    nodeIds.add(normalized.id);
    nodes.push(normalized);
   });

   const edges = [];
   const edgeIds = new Set();
   const edgePairs = new Set();
   sourceEdges.forEach(edge => {
    if (!edge || typeof edge !== 'object') return;
    const from = featureWebSanitizeId(edge.from, FW_TEXT_LIMITS.id);
    const to = featureWebSanitizeId(edge.to, FW_TEXT_LIMITS.id);
    if (!from || !to || from === to) return;
    if (!nodeIds.has(from) || !nodeIds.has(to)) return;
    const normalizedType = featureWebNormalizeEdgeType(edge.type);
    const pairKey = from + '->' + to + '::' + normalizedType;
    if (edgePairs.has(pairKey)) return;
    const normalized = {
     id: featureWebSanitizeId(edge.id, FW_TEXT_LIMITS.id) || featureWebNewId('e'),
     from,
     to,
     type: normalizedType
    };
    while (edgeIds.has(normalized.id)) normalized.id = featureWebNewId('e');
    edgeIds.add(normalized.id);
    edgePairs.add(pairKey);
    edges.push(normalized);
   });

   const graph = { nodes, edges };
   if (!includeReport) return graph;
   return {
    graph,
    report: {
     sourceNodes: sourceNodesAll.length,
     sourceEdges: sourceEdgesAll.length,
     nodes: nodes.length,
     edges: edges.length,
     droppedNodes: Math.max(0, sourceNodesAll.length - nodes.length),
     droppedEdges: Math.max(0, sourceEdgesAll.length - edges.length),
     cappedNodes,
     cappedEdges,
     invalidNodes: Math.max(0, (sourceNodesAll.length - nodes.length) - cappedNodes),
     invalidEdges: Math.max(0, (sourceEdgesAll.length - edges.length) - cappedEdges),
     duplicateTaskLinksDropped
    }
   };
  }

  function featureWebNoticeImportReport(report, label) {
   if (!report) return;
   const dropped = (report.droppedNodes || 0) + (report.droppedEdges || 0);
   const capped = (report.cappedNodes || 0) + (report.cappedEdges || 0);
   const invalid = (report.invalidNodes || 0) + (report.invalidEdges || 0);
   let msg = (label || 'Imported') + ': ' + report.nodes + ' nodes, ' + report.edges + ' links';
   if (dropped > 0) {
    msg += ' (' + dropped + ' dropped';
    if (invalid > 0) msg += ', ' + invalid + ' invalid';
    if (capped > 0) msg += ', ' + capped + ' over cap (max ' + FW_MAX_NODES + ' nodes/' + FW_MAX_EDGES + ' links)';
    msg += ')';
   }
   if ((report.duplicateTaskLinksDropped || 0) > 0) msg += ' · deduped ' + report.duplicateTaskLinksDropped + ' duplicate task link' + (report.duplicateTaskLinksDropped === 1 ? '' : 's');
   featureWebNotice(msg);
  }

  function featureWebConstrainNodesToCanvas() {
   const canvas = document.getElementById('feature-web-canvas');
   if (!canvas || !Array.isArray(featureWeb.nodes)) return false;
   const width = Math.max(140, canvas.clientWidth || canvas.offsetWidth || 0);
   const height = Math.max(60, canvas.clientHeight || canvas.offsetHeight || 0);
   let changed = false;
   featureWeb.nodes.forEach((node, idx) => {
    if (!node || typeof node !== 'object') return;
    const fallbackX = 25 + (idx % 4) * 150;
    const fallbackY = 20 + Math.floor(idx / 4) * 90;
    const baseX = Number.isFinite(+node.x) ? +node.x : fallbackX;
    const baseY = Number.isFinite(+node.y) ? +node.y : fallbackY;
    const clampedX = Math.max(4, Math.min(width - 120, baseX));
    const clampedY = Math.max(4, Math.min(height - 48, baseY));
    if (node.x !== clampedX) { node.x = clampedX; changed = true; }
    if (node.y !== clampedY) { node.y = clampedY; changed = true; }
   });
   return changed;
  }

  function featureWebRemainingNodeCapacity() {
   const used = Array.isArray(featureWeb.nodes) ? featureWeb.nodes.length : 0;
   return Math.max(0, FW_MAX_NODES - used);
  }

  function featureWebRemainingEdgeCapacity() {
   const used = Array.isArray(featureWeb.edges) ? featureWeb.edges.length : 0;
   return Math.max(0, FW_MAX_EDGES - used);
  }

  function featureWebLoad() {
   let parsed = { nodes: [], edges: [] };
   try { parsed = JSON.parse(localStorage.getItem(FEATURE_WEB_KEY) || '{"nodes":[],"edges":[]}'); } catch {}
   featureWeb = featureWebNormalize(parsed);
   const constrained = featureWebConstrainNodesToCanvas();
   if (constrained) featureWebSave();
   if (!featureWeb.nodes.length && !localStorage.getItem('humanity_feature_web_autoseeded_v2')) {
    localStorage.setItem('humanity_feature_web_autoseeded_v2', '1');
    setTimeout(() => featureWebSeedPU(), 0);
   }
   featureWebLoadFilters();
   featureWebLoadUiState();
   featureWebRender();
   featureWebToggleAdvanced(fwAdvancedVisible, true);
   featureWebToggleShortcutHint(fwShortcutsVisible, true);
  }

  function featureWebSave() {
   localStorage.setItem(FEATURE_WEB_KEY, JSON.stringify(featureWeb));
   fwLastSavedAt = Date.now();
  }

  function featureWebPersistAndRefresh() {
   featureWebSave();
   featureWebRender();
  }

  function featureWebPersistAndRefreshEditor() {
   featureWebSave();
   featureWebRenderEditor();
   featureWebRender();
  }

  function featureWebLoadUiState() {
   let ui = null;
   try { ui = JSON.parse(localStorage.getItem(FEATURE_WEB_UI_KEY) || '{}'); } catch {}
   if (ui && typeof ui.advancedVisible === 'boolean') fwAdvancedVisible = ui.advancedVisible;
   if (ui && typeof ui.shortcutsVisible === 'boolean') fwShortcutsVisible = ui.shortcutsVisible;
   if (ui && typeof ui.selectedNodeId === 'string') fwSelectedNodeId = ui.selectedNodeId;
   if (fwSelectedNodeId && !featureWeb.nodes.some(n => n.id === fwSelectedNodeId)) fwSelectedNodeId = '';
  }

  function featureWebSaveUiState() {
   localStorage.setItem(FEATURE_WEB_UI_KEY, JSON.stringify({ advancedVisible: !!fwAdvancedVisible, shortcutsVisible: !!fwShortcutsVisible, selectedNodeId: fwSelectedNodeId || '' }));
  }

  function featureWebSanitizeFilterValue(value, allowed, fallback) {
   if (typeof value !== 'string') return fallback;
   const v = value.trim();
   return allowed.has(v) ? v : fallback;
  }

  function featureWebLoadFilters() {
   let f = null;
   try { f = JSON.parse(localStorage.getItem(FEATURE_WEB_FILTERS_KEY) || '{}'); } catch {}
   const typeEl = document.getElementById('fw-filter-type');
   const statusEl = document.getElementById('fw-filter-status');
   const searchEl = document.getElementById('fw-filter-search');
   const linkedOnlyEl = document.getElementById('fw-linked-only');
   const ownedOnlyEl = document.getElementById('fw-owned-only');
   const focusEl = document.getElementById('fw-focus-selected');
   const simpleEl = document.getElementById('fw-simple-ui');
   const labelsEl = document.getElementById('fw-show-edge-labels');
   const viewEl = document.getElementById('fw-view-mode');
   const domainEl = document.getElementById('fw-filter-domain');

   const typeAllowed = new Set(['all'].concat(FW_ALLOWED_TYPES));
   const statusAllowed = new Set(['all'].concat(FW_ALLOWED_STATUSES));
   const domainAllowed = new Set(['all'].concat(FW_ALLOWED_DOMAINS));
   const viewAllowed = new Set(FW_ALLOWED_VIEW_MODES);

   if (typeEl) typeEl.value = featureWebSanitizeFilterValue(f && f.type, typeAllowed, 'all');
   if (statusEl) statusEl.value = featureWebSanitizeFilterValue(f && f.status, statusAllowed, 'all');
   if (domainEl) domainEl.value = featureWebSanitizeFilterValue(f && f.domain, domainAllowed, 'all');
   if (viewEl) viewEl.value = featureWebSanitizeFilterValue(f && f.viewMode, viewAllowed, 'orbit');
   if (searchEl && f && typeof f.search === 'string') searchEl.value = f.search.slice(0, FW_TEXT_LIMITS.search);
   if (linkedOnlyEl && f && typeof f.linkedOnly === 'boolean') linkedOnlyEl.checked = f.linkedOnly;
   if (ownedOnlyEl && f && typeof f.ownedOnly === 'boolean') ownedOnlyEl.checked = f.ownedOnly;
   if (focusEl && f && typeof f.focusSelected === 'boolean') focusEl.checked = f.focusSelected;
   if (simpleEl) simpleEl.checked = !(f && f.simpleUi === false);
   if (labelsEl && f && typeof f.showEdgeLabels === 'boolean') labelsEl.checked = f.showEdgeLabels;
  }

  function featureWebSaveFilters() {
   const typeEl = document.getElementById('fw-filter-type');
   const statusEl = document.getElementById('fw-filter-status');
   const searchEl = document.getElementById('fw-filter-search');
   const linkedOnlyEl = document.getElementById('fw-linked-only');
   const ownedOnlyEl = document.getElementById('fw-owned-only');
   const focusEl = document.getElementById('fw-focus-selected');
   const simpleEl = document.getElementById('fw-simple-ui');
   const labelsEl = document.getElementById('fw-show-edge-labels');
   const viewEl = document.getElementById('fw-view-mode');
   const domainEl = document.getElementById('fw-filter-domain');

   const typeAllowed = new Set(['all'].concat(FW_ALLOWED_TYPES));
   const statusAllowed = new Set(['all'].concat(FW_ALLOWED_STATUSES));
   const domainAllowed = new Set(['all'].concat(FW_ALLOWED_DOMAINS));
   const viewAllowed = new Set(FW_ALLOWED_VIEW_MODES);

   localStorage.setItem(FEATURE_WEB_FILTERS_KEY, JSON.stringify({
    type: featureWebSanitizeFilterValue(typeEl && typeEl.value, typeAllowed, 'all'),
    status: featureWebSanitizeFilterValue(statusEl && statusEl.value, statusAllowed, 'all'),
    domain: featureWebSanitizeFilterValue(domainEl && domainEl.value, domainAllowed, 'all'),
    viewMode: featureWebSanitizeFilterValue(viewEl && viewEl.value, viewAllowed, 'orbit'),
    search: ((searchEl && searchEl.value) || '').slice(0, FW_TEXT_LIMITS.search),
    linkedOnly: linkedOnlyEl ? !!linkedOnlyEl.checked : false,
    ownedOnly: ownedOnlyEl ? !!ownedOnlyEl.checked : false,
    focusSelected: focusEl ? !!focusEl.checked : true,
    simpleUi: simpleEl ? !!simpleEl.checked : true,
    showEdgeLabels: labelsEl ? !!labelsEl.checked : true,
   }));
  }

  function featureWebClearFilters(opts) {
   const options = opts || {};
   const typeEl = document.getElementById('fw-filter-type');
   const statusEl = document.getElementById('fw-filter-status');
   const searchEl = document.getElementById('fw-filter-search');
   const linkedOnlyEl = document.getElementById('fw-linked-only');
   const ownedOnlyEl = document.getElementById('fw-owned-only');
   const focusEl = document.getElementById('fw-focus-selected');
   const simpleEl = document.getElementById('fw-simple-ui');
   const labelsEl = document.getElementById('fw-show-edge-labels');
   const viewEl = document.getElementById('fw-view-mode');
   const domainEl = document.getElementById('fw-filter-domain');
   if (typeEl) typeEl.value = 'all';
   if (statusEl) statusEl.value = 'all';
   if (domainEl) domainEl.value = 'all';
   if (viewEl) viewEl.value = 'orbit';
   if (searchEl) searchEl.value = '';
   if (linkedOnlyEl) linkedOnlyEl.checked = false;
   if (ownedOnlyEl) ownedOnlyEl.checked = false;
   if (focusEl) focusEl.checked = true;
   if (simpleEl) simpleEl.checked = true;
   if (labelsEl) labelsEl.checked = false;
   featureWebSaveFilters();
   featureWebRender();
   if (!options.quiet) featureWebNotice('Filters cleared');
   if (searchEl) searchEl.focus();
  }

  function featureWebClearSearch(opts) {
   const options = opts || {};
   const searchEl = document.getElementById('fw-filter-search');
   if (!searchEl || !searchEl.value) return;
   searchEl.value = '';
   featureWebSaveFilters();
   featureWebRender();
   if (!options.quiet) featureWebNotice('Search cleared');
   searchEl.focus();
  }

  function featureWebFocusFirstVisible() {
   if (!fwLastVisibleNodeIds.length) {
    featureWebNotice('No visible nodes to focus');
    return;
   }
   const firstId = fwLastVisibleNodeIds[0];
   featureWebFocusNode(firstId);
   featureWebCenterSelected({ quiet: true });
   const node = featureWeb.nodes.find(n => n && n.id === firstId);
   featureWebNotice('Focused first visible node: ' + (node && node.title ? node.title : 'Untitled'));
  }

  function featureWebFocusLastVisible() {
   if (!fwLastVisibleNodeIds.length) {
    featureWebNotice('No visible nodes to focus');
    return;
   }
   const lastId = fwLastVisibleNodeIds[fwLastVisibleNodeIds.length - 1];
   featureWebFocusNode(lastId);
   featureWebCenterSelected({ quiet: true });
   const node = featureWeb.nodes.find(n => n && n.id === lastId);
   featureWebNotice('Focused last visible node: ' + (node && node.title ? node.title : 'Untitled'));
  }

  function featureWebNotice(text) {
   const el = document.getElementById('fw-notice');
   if (!el) return;
   if (!text) { el.style.display = 'none'; el.setAttribute('aria-hidden', 'true'); el.textContent = ''; return; }
   el.textContent = text;
   el.style.display = 'block';
   el.setAttribute('aria-hidden', 'false');
   clearTimeout(featureWebNotice._t);
   featureWebNotice._t = setTimeout(() => {
    if (el.textContent === text) { el.style.display = 'none'; el.textContent = ''; }
   }, 2200);
  }

  function featureWebToggleAdvanced(forceState, skipSave) {
   fwAdvancedVisible = typeof forceState === 'boolean' ? forceState : !fwAdvancedVisible;
   document.querySelectorAll('.fw-advanced').forEach(el => {
    if (!el.dataset.fwDisplay) el.dataset.fwDisplay = el.style.display || '';
    el.style.display = fwAdvancedVisible ? el.dataset.fwDisplay : 'none';
    el.setAttribute('aria-hidden', fwAdvancedVisible ? 'false' : 'true');
   });
   const btn = document.getElementById('fw-toggle-details-btn');
   if (btn) {
    btn.textContent = fwAdvancedVisible ? 'Hide Details' : 'Show Details';
    btn.setAttribute('aria-pressed', fwAdvancedVisible ? 'true' : 'false');
    btn.setAttribute('aria-expanded', fwAdvancedVisible ? 'true' : 'false');
   }
   if (!skipSave) featureWebSaveUiState();
  }

  function featureWebToggleShortcutHint(forceState, skipSave) {
   const hint = document.getElementById('fw-shortcuts-hint');
   const btn = document.getElementById('fw-toggle-shortcuts-btn');
   if (!hint) return;
   fwShortcutsVisible = typeof forceState === 'boolean' ? forceState : !fwShortcutsVisible;
   hint.style.display = fwShortcutsVisible ? 'block' : 'none';
   hint.setAttribute('aria-hidden', fwShortcutsVisible ? 'false' : 'true');
   if (btn) {
    btn.textContent = fwShortcutsVisible ? 'Hide Shortcuts' : 'Show Shortcuts';
    btn.setAttribute('aria-pressed', fwShortcutsVisible ? 'true' : 'false');
    btn.setAttribute('aria-expanded', fwShortcutsVisible ? 'true' : 'false');
   }
   if (!skipSave) featureWebSaveUiState();
  }

  function featureWebSeedPU() {
   if (featureWeb.nodes.length && !confirm('Seed Project Universe map pack? This will append many nodes/links.')) return;
   const mk = (title, type, domain, status, x, y, summary, teach, details, owner, priority) => ({
    id: featureWebNewId('n'),
    title, type, domain, status, x, y,
    summary: summary || '', teach: teach || '', details: details || '', owner: owner || '', priority: priority || 'medium'
   });
   const nodes = [
    mk('PU Mission', 'feature', 'vision', 'active', 40, 24, 'Prepare humanity for a united interplanetary civilization.', 'Everything should ladder up to reducing suffering and increasing capability.', 'Collaboration + education + technology + game loops.', 'leadership', 'critical'),
    mk('Intuitive Teaching Goal', 'feature', 'vision', 'active', 40, 94, 'Teach anyone anything through layered experiences.', 'Every system should explain itself in plain language and examples.', 'Short summary -> teach summary -> deep details.', 'education', 'high'),
    mk('SSPC Legal Entity', 'org', 'business', 'active', 250, 24, 'Washington SPC as governance/business shell.', 'Ensures legal continuity and benefit mandate.', 'Created Dec 2019; social purpose alignment.', 'ops', 'high'),
    mk('Contributor Benefits Program', 'feature', 'business', 'planned', 250, 94, 'Healthcare/housing/food/utilities/education support model.', 'Contributors should be sustainably supported long-term.', 'Availability varies by country until global partners scale.', 'ops', 'high'),
    mk('Funding Portfolio', 'feature', 'business', 'active', 250, 164, 'Patreon/PayPal/partners/sponsors/merch/microtransactions.', 'Diversified funding avoids single-point fragility.', 'Historical channels + future partner commissions.', 'finance', 'high'),
    mk('Community Contribution Pipeline', 'feature', 'community', 'active', 460, 24, 'Volunteers -> trained contributors -> paid roles.', 'People can start helping quickly then specialize.', 'Includes reps, creators, devs, educators.', 'community', 'high'),
    mk('Jobs & Skill Paths', 'feature', 'community', 'planned', 460, 94, 'Developer, artist, audio, representative tracks.', 'Clear role tracks reduce onboarding confusion.', 'Mapped to School and project demand.', 'community', 'high'),
    mk('School Stage 1', 'feature', 'school', 'active', 40, 194, 'Curated free/paid external lessons.', 'Fast baseline skill ramp using internet curriculum.', 'Blender/programming starter links.', 'education', 'high'),
    mk('School Stage 2', 'subfeature', 'school', 'planned', 250, 234, 'Applied project labs + mentorship.', 'Move from passive learning into guided execution.', 'Feature-focused cohort work.', 'education', 'medium'),
    mk('School Stage 3', 'subfeature', 'school', 'planned', 460, 234, 'Leadership/systems mastery and teaching others.', 'Create multipliers who can train new contributors.', 'Capstone across gameplay + operations.', 'education', 'medium'),
    mk('Core Gameplay Loops', 'feature', 'game', 'active', 40, 304, 'Farming, harvesting, crafting, missions, expeditions.', 'Players learn by contributing to shared systems.', 'Solo + group loop parity for inclusivity.', 'design', 'critical'),
    mk('Social MMO Layer', 'feature', 'game', 'active', 250, 304, 'Collaboration-first multiplayer foundations.', 'Healthy social interaction is a core gameplay objective.', 'Supports introvert/extrovert play patterns.', 'design', 'high'),
    mk('Immersive Environment Layer', 'feature', 'game', 'planned', 460, 304, 'Wide environments, planets/space ambiance, exploration rewards.', 'Immersion supports mental health and learning retention.', 'Space-first with planetary expansion.', 'world', 'high'),
    mk('Fleet Economy', 'feature', 'economy', 'planned', 40, 404, 'Money + reputation + resources as parallel progression.', 'Different value channels keep gameplay meaningful.', 'Links production, consumption, and upgrades.', 'economy', 'critical'),
    mk('Global Market (Fleet + Private)', 'subfeature', 'economy', 'planned', 250, 404, 'Public and private supply-demand markets.', 'Enables collaboration and specialization.', 'Fleet market for macro goals; private for player exchange.', 'economy', 'high'),
    mk('Access Systems (Armory/Cargo)', 'subfeature', 'economy', 'planned', 460, 404, 'Home/guild/fleet storage and loadout logistics.', 'Logistics creates strategic tradeoffs and realism.', 'Inventory movement should have time/capacity constraints.', 'economy', 'medium'),
    mk('Roadmap: Act I', 'feature', 'roadmap', 'active', 40, 504, 'Core mechanics + PvP sandbox baseline.', 'Focus first on movement/combat/control primitives.', 'Small scope for fast feedback.', 'roadmap', 'high'),
    mk('Roadmap: Act II', 'subfeature', 'roadmap', 'planned', 250, 504, 'Vehicles + AI + PvE challenge.', 'Introduce purpose loops after movement foundation.', 'At least one per vehicle style.', 'roadmap', 'high'),
    mk('Roadmap: Act III+', 'subfeature', 'roadmap', 'planned', 460, 504, 'Modularity, events, space conflict, expansion.', 'Scale carefully after core loops are stable.', 'Progressive content depth.', 'roadmap', 'medium'),
    mk('Lore Core: Murex System', 'lore', 'lore', 'planned', 40, 584, 'Murex mystery, fleet arc, chaptered progression.', 'Story gives context and stakes to systems.', 'Prelude/Chapter One scaffolding.', 'lore', 'medium'),
    mk('Lore Core: Hungering Dark', 'lore', 'lore', 'planned', 250, 584, 'Primary hostile species arc and tech identity.', 'Faction identity drives mission/event variety.', 'Visual + cultural + gameplay hooks.', 'lore', 'medium'),
    mk('Dev Toolchain Standards', 'tech', 'tech', 'active', 460, 584, 'Unity/C#/Blender/asset format constraints.', 'Consistent pipeline prevents integration chaos.', 'Binary FBX, BC7 DDS, plugin support.', 'engineering', 'high'),
    mk('Feature Web (this system)', 'feature', 'operations', 'active', 670, 304, 'Interactive dependency map for whole project.', 'Understand how everything connects before building.', 'Companion to Kanban.', 'ops', 'high'),
    mk('Kanban Execution Board', 'feature', 'operations', 'active', 670, 404, 'Task flow: backlog/in-progress/testing/done.', 'Execution throughput and ownership clarity.', 'Operational counterpart to feature graph.', 'ops', 'high'),
    mk('Main Power Grid', 'tech', 'operations', 'planned', 670, 504, 'Primary electrical generation and distribution.', 'If power fails, dependent gameplay loops shut down until restored.', 'Drives civilian/public/industrial availability gates.', 'utilities', 'critical'),
    mk('Water & Waste Loop', 'tech', 'operations', 'planned', 860, 504, 'Closed-loop potable water and sanitation systems.', 'Water stability supports habitats, food, health, and morale.', 'Includes treatment, storage, and pressure network.', 'utilities', 'critical'),
    mk('Network Backbone', 'tech', 'operations', 'planned', 860, 404, 'Fleet communications + data routing fabric.', 'Network outages degrade multiplayer and command systems.', 'Supports messaging, market, dispatch, and coordination.', 'utilities', 'high'),
    mk('Industrial Refinery Zone', 'feature', 'operations', 'planned', 860, 304, 'Raw materials refinement + production chain.', 'Damage here bottlenecks crafting and infrastructure growth.', 'Feeds fabrication, armories, and fleet expansion.', 'industry', 'high'),
    mk('Utility Failure Events', 'feature', 'operations', 'planned', 670, 584, 'Cooperative disruptions affecting major systems.', 'Players collaborate to diagnose and restore core utilities.', 'Examples: power cascade, water contamination, network outage.', 'events', 'high'),
    mk('Public Access Gating', 'feature', 'operations', 'planned', 860, 584, 'Public systems unlock/lock based on utility health.', 'Teaches interdependence between infrastructure and civic life.', 'Mall, transport, fabrication, and training may degrade when upstream systems fail.', 'ops', 'high')
   ];
   const byTitle = Object.fromEntries(nodes.map(n => [n.title, n.id]));
   const e = (a,b,t) => ({ id:'e_'+Date.now().toString(36)+Math.random().toString(36).slice(2,5), from:byTitle[a], to:byTitle[b], type:t });
   const edges = [
    e('PU Mission','Intuitive Teaching Goal','enables'),
    e('PU Mission','Core Gameplay Loops','enables'),
    e('PU Mission','School Stage 1','enables'),
    e('PU Mission','SSPC Legal Entity','enables'),
    e('SSPC Legal Entity','Contributor Benefits Program','enables'),
    e('Funding Portfolio','Contributor Benefits Program','enables'),
    e('Funding Portfolio','Community Contribution Pipeline','enables'),
    e('Community Contribution Pipeline','Jobs & Skill Paths','enables'),
    e('School Stage 1','School Stage 2','depends_on'),
    e('School Stage 2','School Stage 3','depends_on'),
    e('Jobs & Skill Paths','School Stage 1','depends_on'),
    e('Core Gameplay Loops','Social MMO Layer','relates_to'),
    e('Core Gameplay Loops','Fleet Economy','enables'),
    e('Social MMO Layer','Community Contribution Pipeline','teaches'),
    e('Immersive Environment Layer','Core Gameplay Loops','enables'),
    e('Fleet Economy','Global Market (Fleet + Private)','enables'),
    e('Fleet Economy','Access Systems (Armory/Cargo)','enables'),
    e('Global Market (Fleet + Private)','Access Systems (Armory/Cargo)','depends_on'),
    e('Roadmap: Act I','Roadmap: Act II','enables'),
    e('Roadmap: Act II','Roadmap: Act III+','enables'),
    e('Core Gameplay Loops','Roadmap: Act I','depends_on'),
    e('Social MMO Layer','Roadmap: Act II','enables'),
    e('Lore Core: Murex System','Roadmap: Act III+','teaches'),
    e('Lore Core: Hungering Dark','Lore Core: Murex System','relates_to'),
    e('Dev Toolchain Standards','Roadmap: Act I','enables'),
    e('Feature Web (this system)','Kanban Execution Board','relates_to'),
    e('Feature Web (this system)','Intuitive Teaching Goal','enables'),
    e('Kanban Execution Board','Roadmap: Act I','enables'),
    e('Main Power Grid','Industrial Refinery Zone','enables'),
    e('Water & Waste Loop','Community Contribution Pipeline','enables'),
    e('Network Backbone','Community Contribution Pipeline','enables'),
    e('Network Backbone','Global Market (Fleet + Private)','enables'),
    e('Industrial Refinery Zone','Fleet Economy','enables'),
    e('Utility Failure Events','Main Power Grid','blocks'),
    e('Utility Failure Events','Water & Waste Loop','blocks'),
    e('Utility Failure Events','Network Backbone','blocks'),
    e('Main Power Grid','Public Access Gating','enables'),
    e('Water & Waste Loop','Public Access Gating','enables'),
    e('Network Backbone','Public Access Gating','enables'),
    e('Public Access Gating','Global Market (Fleet + Private)','relates_to')
   ].filter(x => x.from && x.to);

   const extraModules = [
    ['Bedroom','game','active','Private rest + personalization.','Crew Quarters analog.'],
    ['Battlestation','operations','active','Personal command interface.','Operations Center analog.'],
    ['Network','operations','active','Messaging/social/fleet links.','Fleet Communications Grid analog.'],
    ['Garden','game','planned','Real-world-aligned growing practice.','Public Agropark analog.'],
    ['Workshop','game','planned','Crafting/fabrication practice.','Fabrication Complex analog.'],
    ['Garage','game','planned','Vehicle setup and dispatch.','Transit/Vehicle Bays analog.'],
    ['Armory','game','planned','Loadout and combat prep.','Fleet Armory analog.'],
    ['Living Room','community','planned','Hangout/social/ttrpg/media.','Community Commons analog.'],
    ['Bathroom & Healthcare','operations','planned','Body/health systems.','Medical & Bio Support analog.'],
    ['Mall Kiosks','business','planned','Partner commerce access points.','Public market hubs.'],
    ['Trail System','community','planned','Morale and movement through nature.','Public park circulation.'],
    ['Monorail Network','operations','planned','Fast horizontal transit.','Inter-hub transport backbone.'],
    ['Elevator Network','operations','planned','Fast vertical transit.','Layer-switching core transit.'],
    ['Ramp Fallback Paths','operations','planned','Always-available backup traversal.','Resilience safety path.'],
    ['Resource Dispatch Drones','economy','planned','Automated mining logistics.','Industrial extraction support.'],
    ['Fleet Utility Dashboard','operations','planned','Power/water/network telemetry.','Civic utility monitoring.'],
    ['Outage Repair Missions','operations','planned','Co-op restoration events.','Infrastructure resilience gameplay.'],
    ['Earth Twin Home Planner','tech','planned','Real home/garden planning mode.','Digital twin practical layer.'],
    ['Earth Twin Regional Overlay','tech','planned','Roads/utilities/regional context.','Regional twin data layer.'],
    ['Earth/Fleet Mode Switch','tech','planned','Switch between twins seamlessly.','Unified systems lens.']
   ];
   extraModules.forEach((m, i) => {
    nodes.push(mk(m[0], 'subfeature', m[1], m[2], 980 + (i % 3) * 120, 40 + Math.floor(i / 3) * 42, m[3], m[4], '', '', 'medium'));
   });

   const byTitle2 = Object.fromEntries(nodes.map(n => [n.title, n.id]));
   const link = (a,b,t) => { if (byTitle2[a] && byTitle2[b]) edges.push({ id:'e_'+Date.now().toString(36)+Math.random().toString(36).slice(2,5), from:byTitle2[a], to:byTitle2[b], type:t }); };
   link('Bedroom','Intuitive Teaching Goal','teaches');
   link('Battlestation','Feature Web (this system)','enables');
   link('Network','Community Contribution Pipeline','enables');
   link('Garden','Core Gameplay Loops','enables');
   link('Workshop','Core Gameplay Loops','enables');
   link('Garage','Core Gameplay Loops','enables');
   link('Armory','Roadmap: Act II','enables');
   link('Living Room','Social MMO Layer','enables');
   link('Bathroom & Healthcare','Contributor Benefits Program','relates_to');
   link('Mall Kiosks','Funding Portfolio','enables');
   link('Trail System','Community Contribution Pipeline','enables');
   link('Monorail Network','Public Access Gating','depends_on');
   link('Elevator Network','Public Access Gating','depends_on');
   link('Ramp Fallback Paths','Elevator Network','blocks');
   link('Resource Dispatch Drones','Industrial Refinery Zone','enables');
   link('Fleet Utility Dashboard','Main Power Grid','teaches');
   link('Fleet Utility Dashboard','Water & Waste Loop','teaches');
   link('Fleet Utility Dashboard','Network Backbone','teaches');
   link('Outage Repair Missions','Utility Failure Events','relates_to');
   link('Earth Twin Home Planner','Earth/Fleet Mode Switch','depends_on');
   link('Earth Twin Regional Overlay','Earth/Fleet Mode Switch','depends_on');

   const existingTitles = new Set(featureWeb.nodes.map(n => featureWebClampText(n.title || '', FW_TEXT_LIMITS.title).toLowerCase()).filter(Boolean));
   const dedupedNodes = nodes.filter(n => {
    const key = featureWebClampText(n.title || '', FW_TEXT_LIMITS.title).toLowerCase();
    if (!key || existingTitles.has(key)) return false;
    existingTitles.add(key);
    return true;
   });
   const remaining = featureWebRemainingNodeCapacity();
   const nodesToAdd = dedupedNodes.slice(0, remaining);

   const validNodeIds = new Set(featureWeb.nodes.map(n => n.id));
   nodesToAdd.forEach(n => validNodeIds.add(n.id));
   const existingPairs = new Set(featureWeb.edges.map(e => (e && e.from && e.to) ? (e.from + '->' + e.to) : ''));
   const dedupedEdges = edges.filter(e => {
    if (!e || !e.from || !e.to || e.from === e.to) return false;
    if (!validNodeIds.has(e.from) || !validNodeIds.has(e.to)) return false;
    const key = e.from + '->' + e.to;
    if (existingPairs.has(key)) return false;
    existingPairs.add(key);
    return true;
   });
   const edgesToAdd = dedupedEdges.slice(0, featureWebRemainingEdgeCapacity());

   featureWeb.nodes = featureWeb.nodes.concat(nodesToAdd);
   featureWeb.edges = featureWeb.edges.concat(edgesToAdd);
   featureWebOrbitLayout();
   const skippedForCapacity = Math.max(0, dedupedNodes.length - nodesToAdd.length);
   const skippedEdgesForCapacity = Math.max(0, dedupedEdges.length - edgesToAdd.length);
   let seedMsg = 'Seeded PU map pack (' + nodesToAdd.length + ' nodes, ' + edgesToAdd.length + ' links)';
   if (skippedForCapacity > 0) seedMsg += ' · node limit reached (' + FW_MAX_NODES + ')';
   if (skippedEdgesForCapacity > 0) seedMsg += ' · link limit reached (' + FW_MAX_EDGES + ')';
   featureWebNotice(seedMsg);
  }

  function featureWebTeachMode() {
   if (!fwSelectedNodeId) { featureWebNotice('Select a node first'); return; }
   const node = featureWeb.nodes.find(n => n.id === fwSelectedNodeId);
   if (!node) {
    featureWebClearSelection({ quiet: true });
    featureWebNotice('Selected node is unavailable');
    return;
   }
   const incoming = featureWeb.edges.filter(e => e.to === node.id).map(e => featureWeb.nodes.find(n => n.id === e.from)).filter(Boolean);
   const outgoing = featureWeb.edges.filter(e => e.from === node.id).map(e => featureWeb.nodes.find(n => n.id === e.to)).filter(Boolean);
   const panel = document.getElementById('fw-teach-panel');
   if (!panel) return;
   const prereq = incoming.length ? incoming.map(n => '• ' + n.title).join('\n') : '• None';
   const next = outgoing.length ? outgoing.map(n => '• ' + n.title).join('\n') : '• None yet';
   panel.style.display = 'block';
   panel.setAttribute('aria-hidden', 'false');
   const teachBtn = document.getElementById('fw-teach-btn');
   if (teachBtn) teachBtn.setAttribute('aria-expanded', 'true');
   panel.textContent = (node.teach || node.summary || 'No teach summary yet.') + '\n\nPrerequisites:\n' + prereq + '\n\nSuggested Next:\n' + next;
  }

  function featureWebBuildNode(title, x, y) {
   const typeEl = document.getElementById('fw-node-type');
   const statusEl = document.getElementById('fw-node-status');
   const domainEl = document.getElementById('fw-node-domain');
   const ownerEl = document.getElementById('fw-node-owner');
   const type = (typeEl && typeof typeEl.value === 'string') ? typeEl.value.trim().toLowerCase() : '';
   const domain = (domainEl && typeof domainEl.value === 'string') ? domainEl.value.trim().toLowerCase() : '';
   const status = (statusEl && typeof statusEl.value === 'string') ? statusEl.value.trim().toLowerCase() : '';
   return {
    id: featureWebNewId('n'),
    title: featureWebClampText(title, FW_TEXT_LIMITS.title),
    type: FW_ALLOWED_TYPES.includes(type) ? type : 'feature',
    domain: FW_ALLOWED_DOMAINS.includes(domain) ? domain : 'game',
    status: FW_ALLOWED_STATUSES.includes(status) ? status : 'planned',
    summary: '',
    teach: '',
    details: '',
    owner: featureWebClampText(ownerEl ? ownerEl.value : '', FW_TEXT_LIMITS.owner),
    priority: 'medium',
    x: x,
    y: y,
   };
  }

  function featureWebAddNode() {
   const titleEl = document.getElementById('fw-node-title');
   if (!titleEl || !titleEl.value.trim()) {
    featureWebNotice('Enter a node title before adding');
    if (titleEl && typeof titleEl.focus === 'function') titleEl.focus();
    return;
   }
   if (featureWebRemainingNodeCapacity() <= 0) {
    featureWebNotice('Node limit reached (' + FW_MAX_NODES + ')');
    return;
   }
   const normalizedTitle = featureWebClampText(titleEl.value, FW_TEXT_LIMITS.title).toLowerCase();
   if (!normalizedTitle) {
    featureWebNotice('Node title is required');
    if (titleEl && typeof titleEl.focus === 'function') titleEl.focus();
    return;
   }
   const existingByTitle = featureWeb.nodes.find(n => featureWebClampText(n && n.title || '', FW_TEXT_LIMITS.title).toLowerCase() === normalizedTitle);
   if (existingByTitle) {
    featureWebFocusNode(existingByTitle.id);
    featureWebCenterSelected({ quiet: true });
    featureWebNotice('Node already exists: ' + (existingByTitle.title || 'Untitled'));
    return;
   }
   const node = featureWebBuildNode(
    titleEl.value,
    25 + (featureWeb.nodes.length % 4) * 150,
    20 + Math.floor(featureWeb.nodes.length / 4) * 90,
   );
   featureWeb.nodes.push(node);
   fwSelectedNodeId = node.id;
   featureWebSaveUiState();
   titleEl.value = '';
   const ownerEl = document.getElementById('fw-node-owner');
   if (ownerEl) ownerEl.value = '';
   featureWebNotice('Added node: ' + (node.title || 'Untitled'));
   featureWebPersistAndRefresh();
   if (titleEl && typeof titleEl.focus === 'function') titleEl.focus();
  }

  function featureWebCanvasAdd(e) {
   const titleEl = document.getElementById('fw-node-title');
   const canvas = document.getElementById('feature-web-canvas');
   if (featureWebRemainingNodeCapacity() <= 0) {
    featureWebNotice('Node limit reached (' + FW_MAX_NODES + ')');
    return;
   }
   if (!titleEl || !canvas || !titleEl.value.trim()) {
    featureWebNotice('Enter a node title, then double-click canvas to place it');
    if (titleEl && typeof titleEl.focus === 'function') titleEl.focus();
    return;
   }
   const title = featureWebClampText(titleEl.value, FW_TEXT_LIMITS.title);
   if (!title) {
    featureWebNotice('Node title is required');
    if (titleEl && typeof titleEl.focus === 'function') titleEl.focus();
    return;
   }
   const existingByTitle = featureWeb.nodes.find(n => featureWebClampText(n && n.title || '', FW_TEXT_LIMITS.title).toLowerCase() === title.toLowerCase());
   if (existingByTitle) {
    featureWebFocusNode(existingByTitle.id);
    featureWebCenterSelected({ quiet: true });
    featureWebNotice('Node already exists: ' + (existingByTitle.title || 'Untitled'));
    return;
   }
   const rect = canvas.getBoundingClientRect();
   const x = Math.max(4, Math.min(rect.width - 120, e.clientX - rect.left - 52));
   const y = Math.max(4, Math.min(rect.height - 48, e.clientY - rect.top - 20));
   const node = featureWebBuildNode(title, x, y);
   featureWeb.nodes.push(node);
   fwSelectedNodeId = node.id;
   featureWebSaveUiState();
   titleEl.value = '';
   const ownerEl = document.getElementById('fw-node-owner');
   if (ownerEl) ownerEl.value = '';
   featureWebNotice('Added node: ' + (node.title || 'Untitled'));
   featureWebPersistAndRefresh();
   if (titleEl && typeof titleEl.focus === 'function') titleEl.focus();
  }

  function featureWebCanvasKeydown(e) {
   if (e.key === 'Enter') {
    e.preventDefault();
    featureWebFocusFirstVisible();
    if (e.shiftKey) featureWebTeachMode();
    return;
   }
   if (e.key === 'Escape') {
    e.preventDefault();
    featureWebClearSelection({ quiet: true });
   }
  }

  function featureWebAddNodeFromTask(taskId) {
   const task = featureWebFindTaskById(taskId);
   if (!task) {
    featureWebNotice('Task not found for node creation');
    return;
   }
   if (featureWebRemainingNodeCapacity() <= 0) {
    featureWebNotice('Node limit reached (' + FW_MAX_NODES + ')');
    return;
   }
   const normalizedTaskId = featureWebNormalizeTaskId(taskId);
   if (normalizedTaskId == null) {
    featureWebNotice('Invalid task id');
    return;
   }
   const existing = featureWeb.nodes.find(n => featureWebNormalizeTaskId(n && n.taskId) === normalizedTaskId);
   if (existing) {
    featureWebFocusNode(existing.id);
    featureWebCenterSelected({ quiet: true });
    featureWebNotice('Task already linked to node: ' + (existing.title || ('Task #' + normalizedTaskId)));
    return;
   }
   const normalizedPriority = boardNormalizePriority(task.priority);
   const uniqueTitle = featureWebMakeUniqueTitle(boardNormalizeTitle(task.title), 'Task #' + normalizedTaskId);
   const newNode = {
    id: featureWebNewId('n'),
    taskId: normalizedTaskId,
    title: uniqueTitle,
    type: 'feature',
    domain: 'operations',
    status: featureWebMapTaskStatusToNode(task.status, 'planned'),
    summary: featureWebClampText(boardNormalizeDescription(task.description), FW_TEXT_LIMITS.summary),
    teach: '',
    details: '',
    owner: boardNormalizeAssignee(task.assignee) || '',
    priority: normalizedPriority,
    x: 25 + (featureWeb.nodes.length % 4) * 150,
    y: 20 + Math.floor(featureWeb.nodes.length / 4) * 90,
   };
   featureWeb.nodes.push(newNode);
   fwSelectedNodeId = newNode.id;
   featureWebSaveUiState();
   featureWebNotice('Task linked as node: ' + uniqueTitle);
   featureWebPersistAndRefresh();
  }

  function featureWebAddEdge() {
   const fromEl = document.getElementById('fw-link-from');
   const toEl = document.getElementById('fw-link-to');
   const typeEl = document.getElementById('fw-link-type');
   if (!fromEl || !toEl || !fromEl.value || !toEl.value) {
    featureWebNotice('Select both link source and target');
    if (fromEl && !fromEl.value && typeof fromEl.focus === 'function') fromEl.focus();
    else if (toEl && !toEl.value && typeof toEl.focus === 'function') toEl.focus();
    return;
   }
   if (featureWebRemainingEdgeCapacity() <= 0) {
    featureWebNotice('Link limit reached (' + FW_MAX_EDGES + ')');
    return;
   }
   const fromId = featureWebSanitizeId(fromEl.value, FW_TEXT_LIMITS.id);
   const toId = featureWebSanitizeId(toEl.value, FW_TEXT_LIMITS.id);
   if (!fromId || !toId) {
    featureWebNotice('Invalid link source or target');
    return;
   }
   if (fromId === toId) {
    featureWebNotice('Link source and target must be different');
    if (toEl && typeof toEl.focus === 'function') toEl.focus();
    return;
   }
   const hasFromNode = featureWeb.nodes.some(n => n && n.id === fromId);
   const hasToNode = featureWeb.nodes.some(n => n && n.id === toId);
   if (!hasFromNode || !hasToNode) {
    if (!hasFromNode && fromEl) fromEl.value = '';
    if (!hasToNode && toEl) toEl.value = '';
    featureWebRender();
    featureWebNotice('Link source or target no longer exists; refreshed selectors');
    if (!hasFromNode && fromEl && typeof fromEl.focus === 'function') fromEl.focus();
    else if (!hasToNode && toEl && typeof toEl.focus === 'function') toEl.focus();
    return;
   }
   const edgeType = featureWebNormalizeEdgeType(typeEl && typeEl.value);
   if (featureWeb.edges.some(e => e.from === fromId && e.to === toId && featureWebNormalizeEdgeType(e.type) === edgeType)) {
    featureWebNotice('That link type already exists between these nodes');
    if (typeEl && typeof typeEl.focus === 'function') typeEl.focus();
    return;
   }
   const fromNode = featureWeb.nodes.find(n => n && n.id === fromId);
   const toNode = featureWeb.nodes.find(n => n && n.id === toId);
   featureWeb.edges.push({ id: featureWebNewId('e'), from: fromId, to: toId, type: edgeType });
   if (toEl) {
    toEl.value = '';
    if (toEl.options && toEl.options.length) toEl.selectedIndex = 0;
   }
   const fromTitle = featureWebClampText(fromNode && fromNode.title, 40) || 'Source';
   const toTitle = featureWebClampText(toNode && toNode.title, 40) || 'Target';
   featureWebNotice('Linked ' + fromTitle + ' → ' + toTitle + ' (' + edgeType.replace(/_/g, ' ') + ')');
   featureWebPersistAndRefresh();
   if (toEl && typeof toEl.focus === 'function') toEl.focus();
  }

  function featureWebDeleteNode(id) {
   const node = featureWeb.nodes.find(n => n && n.id === id);
   if (!node) {
    if (fwSelectedNodeId === id) {
     fwSelectedNodeId = '';
     featureWebSaveUiState();
    }
    featureWebNotice('Node already removed');
    return;
   }
   const removedLinks = featureWeb.edges.filter(e => e.from === id || e.to === id).length;
   const linkedTaskId = featureWebNormalizeTaskId(node.taskId);
   featureWeb.nodes = featureWeb.nodes.filter(n => n.id !== id);
   featureWeb.edges = featureWeb.edges.filter(e => e.from !== id && e.to !== id);
   if (fwSelectedNodeId === id) {
    fwSelectedNodeId = '';
    featureWebSaveUiState();
   }
   const linkSuffix = removedLinks ? (' (' + removedLinks + ' link' + (removedLinks === 1 ? '' : 's') + ' removed)') : '';
   const taskSuffix = linkedTaskId != null ? (' · task #' + linkedTaskId + ' remains on board') : '';
   featureWebNotice('Deleted node: ' + (node.title || 'Untitled') + linkSuffix + taskSuffix);
   featureWebPersistAndRefresh();
  }

  function featureWebSyncLinkedTask(node) {
   if (!node) return;
   const taskId = featureWebNormalizeTaskId(node.taskId);
   if (taskId == null) {
    node.taskId = null;
    return;
   }
   if (node.taskId !== taskId) node.taskId = taskId;
   const task = featureWebFindTaskById(taskId);
   const oldTitle = task ? (boardNormalizeTitle(task.title) || '') : '';
   const mapped = featureWebMapNodeStatusToTask(node.status);
   const nextTitle = boardNormalizeTitle(node.title) || ('Task #' + taskId);
   node.title = nextTitle;

   let titleSynced = !!task;
   if (task && oldTitle !== nextTitle) {
    titleSynced = boardSendOrNotice({ type: 'task_update', id: taskId, title: nextTitle, description: boardNormalizeDescription(task.description), priority: boardNormalizePriority(task.priority), assignee: boardNormalizeAssignee(task.assignee), labels: boardNormalizeLabelsJson(task.labels) });
   }

   const currentTaskStatus = boardNormalizeTaskStatus(task && task.status, null);
   let statusSynced = !!task && currentTaskStatus === mapped;
   if (!task || currentTaskStatus !== mapped) {
    statusSynced = boardSendOrNotice({ type: 'task_move', id: taskId, status: mapped });
   }

   if (task) {
    let changed = false;
    if (titleSynced && task.title !== nextTitle) {
     task.title = nextTitle;
     changed = true;
    }
    if (statusSynced && task.status !== mapped) {
     task.status = mapped;
     changed = true;
    }
    if (changed) renderBoard();
   }
  }

  function featureWebDeleteEdge(id) {
   const edge = featureWeb.edges.find(e => e && e.id === id);
   if (!edge) {
    featureWebNotice('Link already removed');
    return;
   }
   featureWeb.edges = featureWeb.edges.filter(e => e.id !== id);
   const fromNode = featureWeb.nodes.find(n => n && n.id === edge.from);
   const toNode = featureWeb.nodes.find(n => n && n.id === edge.to);
   const fromTitle = featureWebClampText(fromNode && fromNode.title, 32) || 'Source';
   const toTitle = featureWebClampText(toNode && toNode.title, 32) || 'Target';
   featureWebNotice('Deleted link: ' + fromTitle + ' → ' + toTitle + ' (' + featureWebNormalizeEdgeType(edge.type).replace(/_/g, ' ') + ')');
   featureWebPersistAndRefresh();
  }

  function featureWebCycleEdgeType(id) {
   const edge = featureWeb.edges.find(e => e.id === id);
   if (!edge) {
    featureWebNotice('Link not found');
    return;
   }
   const order = FW_ALLOWED_EDGE_TYPES;
   const currentType = featureWebNormalizeEdgeType(edge.type);
   const idx = order.indexOf(currentType);
   edge.type = order[(idx + 1 + order.length) % order.length];
   const fromNode = featureWeb.nodes.find(n => n && n.id === edge.from);
   const toNode = featureWeb.nodes.find(n => n && n.id === edge.to);
   const fromTitle = featureWebClampText(fromNode && fromNode.title, 32) || 'Source';
   const toTitle = featureWebClampText(toNode && toNode.title, 32) || 'Target';
   featureWebNotice('Link type updated: ' + fromTitle + ' → ' + toTitle + ' (' + edge.type.replace(/_/g, ' ') + ')');
   featureWebPersistAndRefresh();
  }

  function featureWebRenderEditor() {
   const editSelect = document.getElementById('fw-edit-node');
   const titleEl = document.getElementById('fw-edit-title');
   const typeEl = document.getElementById('fw-edit-type');
   const statusEl = document.getElementById('fw-edit-status');
   const domainEl = document.getElementById('fw-edit-domain');
   const summaryEl = document.getElementById('fw-edit-summary');
   const teachEl = document.getElementById('fw-edit-teach');
   const detailsEl = document.getElementById('fw-edit-details');
   const ownerEl = document.getElementById('fw-edit-owner');
   const priorityEl = document.getElementById('fw-edit-priority');
   const unlinkBtn = document.getElementById('fw-unlink-btn');
   const linkTaskEl = document.getElementById('fw-link-task');
   const linkedInfoEl = document.getElementById('fw-linked-task-info');
   const openTaskBtn = document.getElementById('fw-open-task-btn');
   const metricsEl = document.getElementById('fw-selected-metrics');
   if (!editSelect || !titleEl || !typeEl || !statusEl) return;
   if (linkTaskEl) {
    const selectedTask = linkTaskEl.value;
    const orderedTasks = boardTasks.slice().sort((a, b) => {
     const ao = Number.isFinite(+a.position) ? +a.position : 999999;
     const bo = Number.isFinite(+b.position) ? +b.position : 999999;
     if (ao !== bo) return ao - bo;
     return (a.id || 0) - (b.id || 0);
    });
    linkTaskEl.innerHTML = '<option value="">Link to task…</option>' + orderedTasks.map(t => '<option value="' + t.id + '">#' + t.id + ' [' + featureWebBoardStatusLabel(t.status) + '] · ' + escHtml(t.title) + '</option>').join('');
    linkTaskEl.value = selectedTask;
   }
   const node = featureWeb.nodes.find(n => n.id === editSelect.value);
   if (!node) {
    fwSelectedNodeId = '';
    featureWebSaveUiState();
    titleEl.value = '';
    typeEl.value = 'feature';
    statusEl.value = 'planned';
    if (domainEl) domainEl.value = 'game';
    if (summaryEl) summaryEl.value = '';
    if (teachEl) teachEl.value = '';
    if (detailsEl) detailsEl.value = '';
    if (ownerEl) ownerEl.value = '';
    if (priorityEl) priorityEl.value = 'medium';
    if (unlinkBtn) {
     unlinkBtn.style.display = 'none';
     unlinkBtn.setAttribute('aria-hidden', 'true');
    }
    if (linkTaskEl) linkTaskEl.value = '';
    if (linkedInfoEl) { linkedInfoEl.style.display = 'none'; linkedInfoEl.setAttribute('aria-hidden', 'true'); linkedInfoEl.textContent = ''; }
    if (openTaskBtn) {
     openTaskBtn.style.display = 'none';
     openTaskBtn.setAttribute('aria-hidden', 'true');
    }
    if (metricsEl) { metricsEl.style.display = 'none'; metricsEl.setAttribute('aria-hidden', 'true'); metricsEl.textContent = ''; }
    return;
   }
   fwSelectedNodeId = node.id;
   featureWebSaveUiState();
   titleEl.value = node.title || '';
   typeEl.value = node.type || 'feature';
   statusEl.value = node.status || 'planned';
   if (domainEl) domainEl.value = node.domain || 'game';
   if (summaryEl) summaryEl.value = node.summary || '';
   if (teachEl) teachEl.value = node.teach || '';
   if (detailsEl) detailsEl.value = node.details || '';
   if (ownerEl) ownerEl.value = node.owner || '';
   if (priorityEl) priorityEl.value = node.priority || 'medium';
   const linkedTaskId = featureWebNormalizeTaskId(node.taskId);
   if (unlinkBtn) {
    const showUnlink = linkedTaskId != null;
    unlinkBtn.style.display = showUnlink ? 'inline-block' : 'none';
    unlinkBtn.setAttribute('aria-hidden', showUnlink ? 'false' : 'true');
   }
   if (linkTaskEl) linkTaskEl.value = linkedTaskId != null ? String(linkedTaskId) : '';
   if (linkedInfoEl) {
    if (linkedTaskId != null) {
     const t = featureWebFindTaskById(linkedTaskId);
     linkedInfoEl.style.display = 'block';
     linkedInfoEl.setAttribute('aria-hidden', 'false');
     linkedInfoEl.textContent = t ? ('Linked task: #' + t.id + ' [' + featureWebBoardStatusLabel(t.status) + '] ' + t.title) : ('Linked task: #' + linkedTaskId);
     if (openTaskBtn) {
      openTaskBtn.style.display = t ? 'inline-block' : 'none';
      openTaskBtn.setAttribute('aria-hidden', t ? 'false' : 'true');
     }
    } else {
      linkedInfoEl.style.display = 'none';
      linkedInfoEl.setAttribute('aria-hidden', 'true');
      linkedInfoEl.textContent = '';
      if (openTaskBtn) {
       openTaskBtn.style.display = 'none';
       openTaskBtn.setAttribute('aria-hidden', 'true');
      }
    }
   }
   if (metricsEl) {
    const out = featureWeb.edges.filter(e => e.from === node.id).length;
    const inc = featureWeb.edges.filter(e => e.to === node.id).length;
    metricsEl.style.display = 'block';
    metricsEl.setAttribute('aria-hidden', 'false');
    metricsEl.textContent = 'Selected node metrics: out-links ' + out + ' · in-links ' + inc;
   }
  }

  function featureWebSaveNodeEdits() {
   const editSelect = document.getElementById('fw-edit-node');
   const titleEl = document.getElementById('fw-edit-title');
   const typeEl = document.getElementById('fw-edit-type');
   const statusEl = document.getElementById('fw-edit-status');
   const domainEl = document.getElementById('fw-edit-domain');
   const summaryEl = document.getElementById('fw-edit-summary');
   const teachEl = document.getElementById('fw-edit-teach');
   const detailsEl = document.getElementById('fw-edit-details');
   const ownerEl = document.getElementById('fw-edit-owner');
   const priorityEl = document.getElementById('fw-edit-priority');
   if (!editSelect || !titleEl || !typeEl || !statusEl || !editSelect.value) return;
   const node = featureWeb.nodes.find(n => n.id === editSelect.value);
   if (!node) {
    featureWebClearSelection({ quiet: true });
    featureWebNotice('Selected node is unavailable');
    return;
   }
   const nextTitle = featureWebClampText(titleEl.value, FW_TEXT_LIMITS.title);
   if (!nextTitle) {
    featureWebNotice('Node title is required');
    if (titleEl && typeof titleEl.focus === 'function') titleEl.focus();
    return;
   }
   const normalizedTitle = nextTitle.toLowerCase();
   const duplicate = featureWeb.nodes.find(n => n.id !== node.id && featureWebClampText(n.title || '', FW_TEXT_LIMITS.title).toLowerCase() === normalizedTitle);
   if (duplicate) {
    featureWebFocusNode(duplicate.id);
    featureWebCenterSelected({ quiet: true });
    featureWebNotice('Node already exists: ' + (duplicate.title || 'Untitled'));
    return;
   }
   const nextType = typeEl && typeof typeEl.value === 'string' ? typeEl.value.trim().toLowerCase() : '';
   const nextStatus = statusEl && typeof statusEl.value === 'string' ? statusEl.value.trim().toLowerCase() : '';
   const nextDomain = domainEl && typeof domainEl.value === 'string' ? domainEl.value.trim().toLowerCase() : '';
   const nextPriority = featureWebNormalizePriority(priorityEl && priorityEl.value);
   node.title = nextTitle;
   node.type = FW_ALLOWED_TYPES.includes(nextType) ? nextType : (node.type || 'feature');
   node.status = FW_ALLOWED_STATUSES.includes(nextStatus) ? nextStatus : (node.status || 'planned');
   node.domain = FW_ALLOWED_DOMAINS.includes(nextDomain) ? nextDomain : (node.domain || 'game');
   node.summary = featureWebClampText(summaryEl ? summaryEl.value : '', FW_TEXT_LIMITS.summary);
   node.teach = featureWebClampText(teachEl ? teachEl.value : '', FW_TEXT_LIMITS.teach);
   node.details = featureWebClampText(detailsEl ? detailsEl.value : '', FW_TEXT_LIMITS.details);
   node.owner = featureWebClampText(ownerEl ? ownerEl.value : '', FW_TEXT_LIMITS.owner);
   node.priority = FW_ALLOWED_PRIORITIES.includes(nextPriority) ? nextPriority : (node.priority || 'medium');
   featureWebSyncLinkedTask(node);
   featureWebPersistAndRefresh();
  }

  function featureWebFocusNode(id) {
   const editEl = document.getElementById('fw-edit-node');
   if (!editEl) return;
   editEl.value = id;
   featureWebRenderEditor();
   const panel = document.getElementById('fw-teach-panel');
   if (panel) {
    panel.style.display = 'none';
    panel.setAttribute('aria-hidden', 'true');
   }
   const teachBtn = document.getElementById('fw-teach-btn');
   if (teachBtn) teachBtn.setAttribute('aria-expanded', 'false');
   const node = featureWeb.nodes.find(n => n.id === id);
   const t = document.getElementById('sys-selected-title');
   const s = document.getElementById('sys-selected-summary');
   if (!node) {
    fwSelectedNodeId = '';
    if (editEl) editEl.value = '';
    featureWebSaveUiState();
    if (t) t.textContent = 'None';
    if (s) s.textContent = 'Select a node to inspect.';
    featureWebNotice('Node not found');
    return;
   }
   if (t) t.textContent = node.title || 'Untitled';
   if (s) s.textContent = node.summary || node.teach || node.details || 'No summary yet.';
  }

  function featureWebFocusByTitle(title) {
   const target = featureWebClampText(String(title || ''), 140).toLowerCase();
   if (!target) {
    featureWebNotice('Enter a node title to focus');
    return;
   }
   const node = featureWeb.nodes.find(n => featureWebClampText(n.title || '', FW_TEXT_LIMITS.title).toLowerCase() === target);
   if (!node) { featureWebNotice('Not found: ' + title); return; }
   featureWebFocusNode(node.id);
   featureWebCenterSelected({ quiet: true });
   featureWebRender();
   featureWebNotice('Focused node: ' + (node.title || 'Untitled'));
  }

  async function featureWebCopySelectedTitle() {
   if (!fwSelectedNodeId) {
    featureWebNotice('Select a node first');
    return;
   }
   const node = featureWeb.nodes.find(n => n.id === fwSelectedNodeId);
   if (!node) {
    featureWebNotice('Selected node is unavailable');
    return;
   }
   if (!node.title) {
    featureWebNotice('Selected node has no title to copy');
    return;
   }
   const copiedMsg = 'Node title copied: ' + featureWebClampText(node.title, 60);
   try {
    if (navigator.clipboard && navigator.clipboard.writeText) {
     await navigator.clipboard.writeText(node.title);
     featureWebNotice(copiedMsg);
     return;
    }
   } catch {}
   const ta = document.createElement('textarea');
   ta.value = node.title;
   ta.style.position = 'fixed';
   ta.style.left = '-9999px';
   document.body.appendChild(ta);
   ta.select();
   try { document.execCommand('copy'); featureWebNotice(copiedMsg); }
   catch { featureWebNotice('Could not copy title'); }
   document.body.removeChild(ta);
  }

  async function featureWebCopySelectedId() {
   if (!fwSelectedNodeId) {
    featureWebNotice('Select a node first');
    return;
   }
   const node = featureWeb.nodes.find(n => n.id === fwSelectedNodeId);
   if (!node) {
    featureWebNotice('Selected node is unavailable');
    return;
   }
   const copiedMsg = 'Node ID copied: ' + node.id;
   try {
    if (navigator.clipboard && navigator.clipboard.writeText) {
     await navigator.clipboard.writeText(node.id);
     featureWebNotice(copiedMsg);
     return;
    }
   } catch {}
   const ta = document.createElement('textarea');
   ta.value = node.id;
   ta.style.position = 'fixed';
   ta.style.left = '-9999px';
   document.body.appendChild(ta);
   ta.select();
   try { document.execCommand('copy'); featureWebNotice(copiedMsg); }
   catch { featureWebNotice('Could not copy node ID'); }
   document.body.removeChild(ta);
  }

  async function featureWebCopySelectedJson() {
   if (!fwSelectedNodeId) {
    featureWebNotice('Select a node first');
    return;
   }
   const node = featureWeb.nodes.find(n => n.id === fwSelectedNodeId);
   if (!node) {
    featureWebNotice('Selected node is unavailable');
    return;
   }
   const payload = JSON.stringify(node, null, 2);
   const copiedMsg = 'Node JSON copied: ' + featureWebClampText(node.title || 'Untitled', 48) + ' (' + payload.length + ' chars)';
   try {
    if (navigator.clipboard && navigator.clipboard.writeText) {
     await navigator.clipboard.writeText(payload);
     featureWebNotice(copiedMsg);
     return;
    }
   } catch {}
   const ta = document.createElement('textarea');
   ta.value = payload;
   ta.style.position = 'fixed';
   ta.style.left = '-9999px';
   document.body.appendChild(ta);
   ta.select();
   try { document.execCommand('copy'); featureWebNotice(copiedMsg); }
   catch { featureWebNotice('Could not copy node JSON'); }
   document.body.removeChild(ta);
  }

  function featureWebUnlinkSelected() {
   if (!fwSelectedNodeId) {
    featureWebNotice('Select a node first');
    return;
   }
   const node = featureWeb.nodes.find(n => n.id === fwSelectedNodeId);
   if (!node) {
    featureWebNotice('Selected node is unavailable');
    return;
   }
   const taskId = featureWebNormalizeTaskId(node.taskId);
   if (taskId == null) {
    featureWebNotice('Selected node is not linked to a task');
    return;
   }
   node.taskId = null;
   featureWebPersistAndRefreshEditor();
   featureWebNotice('Unlinked node ' + (node.title || 'Untitled') + ' from task #' + taskId);
  }

  function featureWebOpenLinkedTask() {
   if (!fwSelectedNodeId) {
    featureWebNotice('Select a node first');
    return;
   }
   const node = featureWeb.nodes.find(n => n.id === fwSelectedNodeId);
   if (!node) {
    featureWebNotice('Selected node is unavailable');
    return;
   }
   if (node.taskId == null) {
    featureWebNotice('Selected node is not linked to a task');
    return;
   }
   const taskId = featureWebNormalizeTaskId(node.taskId);
   if (taskId == null) {
    node.taskId = null;
    featureWebPersistAndRefreshEditor();
    featureWebNotice('Linked task id was invalid and has been cleared');
    return;
   }
   if (node.taskId !== taskId) {
    node.taskId = taskId;
    featureWebPersistAndRefreshEditor();
   }
   const task = featureWebFindTaskById(taskId);
   if (!task) {
    featureWebNotice('Linked task is unavailable on the board');
    return;
   }
   openTaskModal(taskId);
   featureWebNotice('Opened task #' + taskId + ': ' + (boardNormalizeTitle(task.title) || ('Task #' + taskId)));
  }

  function featureWebLinkSelectedToTask() {
   if (!fwSelectedNodeId) {
    featureWebNotice('Select a node first');
    return;
   }
   const linkTaskEl = document.getElementById('fw-link-task');
   if (!linkTaskEl || !linkTaskEl.value) {
    featureWebNotice('Select a task to link');
    if (linkTaskEl && typeof linkTaskEl.focus === 'function') linkTaskEl.focus();
    return;
   }
   const node = featureWeb.nodes.find(n => n.id === fwSelectedNodeId);
   if (!node) {
    featureWebNotice('Selected node is unavailable');
    return;
   }
   const taskId = featureWebNormalizeTaskId(linkTaskEl.value);
   if (taskId == null) {
    featureWebNotice('Invalid task selection');
    if (linkTaskEl && typeof linkTaskEl.focus === 'function') linkTaskEl.focus();
    return;
   }
   const task = featureWebFindTaskById(taskId);
   if (!task) {
    featureWebNotice('Selected task is unavailable');
    if (linkTaskEl && typeof linkTaskEl.focus === 'function') linkTaskEl.focus();
    return;
   }
   const currentTaskId = featureWebNormalizeTaskId(node.taskId);
   if (currentTaskId != null && currentTaskId === taskId) {
    featureWebNotice('Node already linked to task #' + taskId);
    return;
   }
   const conflict = featureWeb.nodes.find(n => n.id !== node.id && featureWebNormalizeTaskId(n && n.taskId) === taskId);
   if (conflict) {
    featureWebFocusNode(conflict.id);
    featureWebCenterSelected({ quiet: true });
    featureWebNotice('Task #' + taskId + ' already linked to node: ' + (conflict.title || 'Untitled'));
    return;
   }
   node.taskId = taskId;
   featureWebPersistAndRefreshEditor();
   featureWebNotice('Linked node to task #' + taskId + ': ' + (boardNormalizeTitle(task.title) || ('Task #' + taskId)));
  }

  function featureWebSyncNodesToBoard() {
   if (!boardIsConnected()) {
    boardNoticeConnectionNotReady();
    return;
   }
   const canEdit = boardCanEdit();
   if (!canEdit) {
    featureWebNotice('Need admin/mod role to create tasks');
    return;
   }
   let candidates = 0;
   let created = 0;
   featureWeb.nodes.forEach((node) => {
    if (featureWebNormalizeTaskId(node && node.taskId) != null) return;
    candidates++;
    const mappedStatus = featureWebMapNodeStatusToTask(node.status);
    const rawDomain = typeof node.domain === 'string' ? node.domain.trim().toLowerCase() : '';
    const rawType = typeof node.type === 'string' ? node.type.trim().toLowerCase() : '';
    const rawPriority = featureWebNormalizePriority(node.priority);
    const nodeDomain = FW_ALLOWED_DOMAINS.includes(rawDomain) ? rawDomain : 'game';
    const nodeType = FW_ALLOWED_TYPES.includes(rawType) ? rawType : 'feature';
    const nodePriority = FW_ALLOWED_PRIORITIES.includes(rawPriority) ? rawPriority : 'medium';
    const marker = '[fw-node:' + featureWebSanitizeId(node.id, FW_TEXT_LIMITS.id) + ']';
    const descParts = [marker, featureWebClampText(node.summary || '', FW_TEXT_LIMITS.summary), featureWebClampText(node.details || '', FW_TEXT_LIMITS.details)].filter(Boolean);
    const payload = {
      type: 'task_create',
      title: featureWebClampText(node.title || '', FW_TEXT_LIMITS.title) || 'Untitled Node',
      description: descParts.join('\n\n'),
      status: mappedStatus,
      priority: nodePriority,
      assignee: featureWebClampText(node.owner || '', FW_TEXT_LIMITS.owner) || null,
      labels: JSON.stringify([nodeDomain, nodeType]),
    };
    if (boardSend(payload)) created++;
   });
   if (!candidates) {
    featureWebNotice('All nodes already linked');
    return;
   }
   const createdLabel = created === 1 ? 'task' : 'tasks';
   if (created === candidates) {
    featureWebNotice('Queued ' + created + ' ' + createdLabel + ' from ' + candidates + ' unlinked node' + (candidates === 1 ? '' : 's'));
    return;
   }
   const failed = Math.max(0, candidates - created);
   featureWebNotice('Queued ' + created + '/' + candidates + ' ' + createdLabel + ' from unlinked nodes (' + failed + ' failed to queue)');
  }

  function featureWebClearSelection(opts) {
   const options = opts || {};
   const hadSelection = !!fwSelectedNodeId;
   fwSelectedNodeId = '';
   featureWebSaveUiState();
   const editEl = document.getElementById('fw-edit-node');
   if (editEl) editEl.value = '';
   featureWebRenderEditor();
   featureWebRender();
   if (!options.quiet) featureWebNotice(hadSelection ? 'Selection cleared' : 'No node selected');
  }

  function featureWebCenterSelected(opts) {
   const options = opts || {};
   if (!fwSelectedNodeId) {
    if (!options.quiet) featureWebNotice('Select a node first');
    return;
   }
   const node = featureWeb.nodes.find(n => n.id === fwSelectedNodeId);
   const canvas = document.getElementById('feature-web-canvas');
   if (!node || !canvas) {
    if (!options.quiet) featureWebNotice('Selected node is unavailable');
    return;
   }
   const width = Math.max(220, canvas.clientWidth);
   const height = Math.max(180, canvas.clientHeight);
   node.x = Math.max(4, Math.min(width - 120, (width - 104) / 2));
   node.y = Math.max(4, Math.min(height - 48, (height - 42) / 2));
   featureWebPersistAndRefresh();
   if (!options.quiet) featureWebNotice('Centered node: ' + (node.title || 'Untitled'));
  }

  function featureWebDuplicateSelected() {
   if (!fwSelectedNodeId) {
    featureWebNotice('Select a node first');
    return;
   }
   if (featureWebRemainingNodeCapacity() <= 0) {
    featureWebNotice('Node limit reached (' + FW_MAX_NODES + ')');
    return;
   }
   const node = featureWeb.nodes.find(n => n.id === fwSelectedNodeId);
   const canvas = document.getElementById('feature-web-canvas');
   if (!node || !canvas) {
    featureWebNotice('Selected node is unavailable');
    return;
   }
   const width = Math.max(220, canvas.clientWidth);
   const height = Math.max(180, canvas.clientHeight);
   const dup = JSON.parse(JSON.stringify(node));
   dup.id = featureWebNewId('n');
   dup.taskId = null;
   dup.title = featureWebMakeUniqueTitle((node.title || 'Node') + ' Copy', 'Node Copy');
   const dupType = typeof dup.type === 'string' ? dup.type.trim().toLowerCase() : '';
   const dupStatus = typeof dup.status === 'string' ? dup.status.trim().toLowerCase() : '';
   const dupDomain = typeof dup.domain === 'string' ? dup.domain.trim().toLowerCase() : '';
   const dupPriority = featureWebNormalizePriority(dup.priority);
   dup.type = FW_ALLOWED_TYPES.includes(dupType) ? dupType : 'feature';
   dup.status = FW_ALLOWED_STATUSES.includes(dupStatus) ? dupStatus : 'planned';
   dup.domain = FW_ALLOWED_DOMAINS.includes(dupDomain) ? dupDomain : 'game';
   dup.priority = FW_ALLOWED_PRIORITIES.includes(dupPriority) ? dupPriority : 'medium';
   dup.summary = featureWebClampText(dup.summary || '', FW_TEXT_LIMITS.summary);
   dup.teach = featureWebClampText(dup.teach || '', FW_TEXT_LIMITS.teach);
   dup.details = featureWebClampText(dup.details || '', FW_TEXT_LIMITS.details);
   dup.owner = featureWebClampText(dup.owner || '', FW_TEXT_LIMITS.owner);
   dup.x = Math.max(4, Math.min(width - 120, (node.x || 0) + 26));
   dup.y = Math.max(4, Math.min(height - 48, (node.y || 0) + 20));
   featureWeb.nodes.push(dup);
   fwSelectedNodeId = dup.id;
   featureWebSaveUiState();
   featureWebNotice('Duplicated node: ' + (node.title || 'Untitled') + ' → ' + (dup.title || 'Untitled'));
   featureWebPersistAndRefresh();
  }

  function featureWebNudgeSelected(dx, dy) {
   if (!fwSelectedNodeId) {
    featureWebNotice('Select a node first');
    return;
   }
   const node = featureWeb.nodes.find(n => n.id === fwSelectedNodeId);
   const canvas = document.getElementById('feature-web-canvas');
   if (!node || !canvas) {
    featureWebNotice('Selected node is unavailable');
    return;
   }
   const width = Math.max(220, canvas.clientWidth);
   const height = Math.max(180, canvas.clientHeight);
   node.x = Math.max(4, Math.min(width - 120, (node.x || 0) + dx));
   node.y = Math.max(4, Math.min(height - 48, (node.y || 0) + dy));
   featureWebPersistAndRefresh();
  }

  function featureWebNudgeToGrid(grid) {
   if (!fwSelectedNodeId) {
    featureWebNotice('Select a node first');
    return;
   }
   const node = featureWeb.nodes.find(n => n.id === fwSelectedNodeId);
   const canvas = document.getElementById('feature-web-canvas');
   if (!node || !canvas) {
    featureWebNotice('Selected node is unavailable');
    return;
   }
   const width = Math.max(220, canvas.clientWidth);
   const height = Math.max(180, canvas.clientHeight);
   const g = Math.max(4, Number(grid) || 12);
   node.x = Math.max(4, Math.min(width - 120, Math.round((node.x || 0) / g) * g));
   node.y = Math.max(4, Math.min(height - 48, Math.round((node.y || 0) / g) * g));
   featureWebPersistAndRefresh();
   featureWebNotice('Snapped ' + (node.title || 'selected node') + ' to ' + g + 'px grid');
  }

  function featureWebCycleSelectedStatus(step) {
   if (!fwSelectedNodeId) {
    featureWebNotice('Select a node first');
    return;
   }
   const node = featureWeb.nodes.find(n => n.id === fwSelectedNodeId);
   if (!node) {
    featureWebNotice('Selected node is unavailable');
    return;
   }
   const order = FW_ALLOWED_STATUSES;
   const idx = Math.max(0, order.indexOf(node.status || 'planned'));
   const next = (idx + (step || 1) + order.length) % order.length;
   node.status = order[next];
   featureWebSyncLinkedTask(node);
   featureWebNotice('Set status for ' + (node.title || 'selected node') + ': ' + node.status);
   featureWebPersistAndRefreshEditor();
  }

  function featureWebCycleSelectedType(step) {
   if (!fwSelectedNodeId) {
    featureWebNotice('Select a node first');
    return;
   }
   const node = featureWeb.nodes.find(n => n.id === fwSelectedNodeId);
   if (!node) {
    featureWebNotice('Selected node is unavailable');
    return;
   }
   const order = FW_ALLOWED_TYPES;
   const idx = Math.max(0, order.indexOf(node.type || 'feature'));
   const next = (idx + (step || 1) + order.length) % order.length;
   node.type = order[next];
   featureWebNotice('Set type for ' + (node.title || 'selected node') + ': ' + node.type);
   featureWebPersistAndRefreshEditor();
  }

  function featureWebCycleEdgeTypeForSelection(step) {
   if (!fwSelectedNodeId) {
    featureWebNotice('Select a node first');
    return;
   }
   const node = featureWeb.nodes.find(n => n && n.id === fwSelectedNodeId);
   if (!node) {
    featureWebNotice('Selected node is unavailable');
    return;
   }
   const nodeTitle = featureWebClampText(node.title, 40) || 'Selected node';
   const order = FW_ALLOWED_EDGE_TYPES;
   const edges = featureWeb.edges.filter(e => e.from === fwSelectedNodeId || e.to === fwSelectedNodeId);
   if (!edges.length) {
    featureWebNotice(nodeTitle + ' has no links');
    return;
   }
   edges.forEach(edge => {
    const currentType = featureWebNormalizeEdgeType(edge.type);
    const idx = Math.max(0, order.indexOf(currentType));
    edge.type = order[(idx + (step || 1) + order.length) % order.length];
   });
   featureWebPersistAndRefresh();
   featureWebNotice('Updated ' + edges.length + ' link type' + (edges.length === 1 ? '' : 's') + ' for ' + nodeTitle);
  }

  function featureWebDeleteSelectedLinks() {
   if (!fwSelectedNodeId) {
    featureWebNotice('Select a node first');
    return;
   }
   const node = featureWeb.nodes.find(n => n && n.id === fwSelectedNodeId);
   if (!node) {
    featureWebNotice('Selected node is unavailable');
    return;
   }
   const nodeTitle = featureWebClampText(node.title, 40) || 'Selected node';
   const before = featureWeb.edges.length;
   featureWeb.edges = featureWeb.edges.filter(e => e.from !== fwSelectedNodeId && e.to !== fwSelectedNodeId);
   const removed = before - featureWeb.edges.length;
   if (!removed) {
    featureWebNotice(nodeTitle + ' has no links');
    return;
   }
   featureWebPersistAndRefresh();
   featureWebNotice('Removed ' + removed + ' link' + (removed === 1 ? '' : 's') + ' from ' + nodeTitle);
  }

  function featureWebDeleteOutgoingLinks() {
   if (!fwSelectedNodeId) {
    featureWebNotice('Select a node first');
    return;
   }
   const node = featureWeb.nodes.find(n => n && n.id === fwSelectedNodeId);
   if (!node) {
    featureWebNotice('Selected node is unavailable');
    return;
   }
   const nodeTitle = featureWebClampText(node.title, 40) || 'Selected node';
   const before = featureWeb.edges.length;
   featureWeb.edges = featureWeb.edges.filter(e => e.from !== fwSelectedNodeId);
   const removed = before - featureWeb.edges.length;
   if (!removed) {
    featureWebNotice(nodeTitle + ' has no outgoing links');
    return;
   }
   featureWebPersistAndRefresh();
   featureWebNotice('Removed ' + removed + ' outgoing link' + (removed === 1 ? '' : 's') + ' from ' + nodeTitle);
  }

  function featureWebDeleteIncomingLinks() {
   if (!fwSelectedNodeId) {
    featureWebNotice('Select a node first');
    return;
   }
   const node = featureWeb.nodes.find(n => n && n.id === fwSelectedNodeId);
   if (!node) {
    featureWebNotice('Selected node is unavailable');
    return;
   }
   const nodeTitle = featureWebClampText(node.title, 40) || 'Selected node';
   const before = featureWeb.edges.length;
   featureWeb.edges = featureWeb.edges.filter(e => e.to !== fwSelectedNodeId);
   const removed = before - featureWeb.edges.length;
   if (!removed) {
    featureWebNotice(nodeTitle + ' has no incoming links');
    return;
   }
   featureWebPersistAndRefresh();
   featureWebNotice('Removed ' + removed + ' incoming link' + (removed === 1 ? '' : 's') + ' from ' + nodeTitle);
  }

  function featureWebDeleteSelectedNode() {
   if (!fwSelectedNodeId) {
    featureWebNotice('Select a node first');
    return;
   }
   featureWebDeleteNode(fwSelectedNodeId);
  }

  function featureWebSetHover(id) {
   fwHoveredNodeId = id || '';
   featureWebRender();
  }

  function featureWebAutoLayout() {
   const canvas = document.getElementById('feature-web-canvas');
   if (!canvas || !featureWeb.nodes.length) {
    if (!featureWeb.nodes.length) featureWebNotice('Add nodes before applying auto layout');
    return;
   }
   const width = Math.max(220, canvas.clientWidth);
   const height = Math.max(180, canvas.clientHeight);
   const cols = Math.max(1, Math.floor((width - 24) / 150));
   featureWeb.nodes.forEach((n, i) => {
    const col = i % cols;
    const row = Math.floor(i / cols);
    n.x = 16 + col * 150;
    n.y = 14 + row * 86;
    n.x = Math.max(4, Math.min(width - 120, n.x));
    n.y = Math.max(4, Math.min(height - 48, n.y));
   });
   featureWebPersistAndRefresh();
   featureWebNotice('Applied auto layout to ' + featureWeb.nodes.length + ' nodes');
  }

  function featureWebConstellationLayout() {
   const canvas = document.getElementById('feature-web-canvas');
   const viewEl = document.getElementById('fw-view-mode');
   if (!canvas || !featureWeb.nodes.length) {
    if (!featureWeb.nodes.length) featureWebNotice('Add nodes before applying constellation layout');
    return;
   }
   const width = Math.max(320, canvas.clientWidth);
   const height = Math.max(220, canvas.clientHeight);
   const domains = FW_ALLOWED_DOMAINS;
   const centerX = width / 2;
   const centerY = height / 2;
   const baseR = Math.max(70, Math.min(width, height) * 0.34);
   const domainCenters = {};
   domains.forEach((d, i) => {
    const a = (i / domains.length) * Math.PI * 2;
    domainCenters[d] = { x: centerX + Math.cos(a) * baseR, y: centerY + Math.sin(a) * baseR };
   });
   const byDomain = {};
   featureWeb.nodes.forEach(n => {
    const d = n.domain || 'game';
    if (!byDomain[d]) byDomain[d] = [];
    byDomain[d].push(n);
   });
   Object.entries(byDomain).forEach(([d, nodes]) => {
    const c = domainCenters[d] || { x: centerX, y: centerY };
    const ring = Math.max(18, 16 + nodes.length * 2.4);
    nodes.forEach((n, i) => {
      const a = (i / Math.max(1, nodes.length)) * Math.PI * 2;
      const jitter = (i % 3) * 5;
      n.x = Math.max(4, Math.min(width - 120, c.x + Math.cos(a) * (ring + jitter)));
      n.y = Math.max(4, Math.min(height - 48, c.y + Math.sin(a) * (ring + jitter)));
    });
   });
   if (viewEl) viewEl.value = 'constellation';
   featureWebSaveFilters();
   featureWebPersistAndRefresh();
   featureWebNotice('Applied constellation layout to ' + featureWeb.nodes.length + ' nodes');
  }

  function featureWebOrbitLayout() {
   const canvas = document.getElementById('feature-web-canvas');
   const viewEl = document.getElementById('fw-view-mode');
   if (!canvas || !featureWeb.nodes.length) {
    if (!featureWeb.nodes.length) featureWebNotice('Add nodes before applying orbit layout');
    return;
   }
   const width = Math.max(360, canvas.clientWidth);
   const height = Math.max(240, canvas.clientHeight);
   const centerX = width / 2;
   const centerY = height / 2;
   const domains = FW_ALLOWED_DOMAINS;
   const radiusDomain = Math.max(120, Math.min(width, height) * 0.34);
   const domainCenters = {};
   domains.forEach((d, i) => {
    const a = (i / domains.length) * Math.PI * 2 - Math.PI / 2;
    domainCenters[d] = { x: centerX + Math.cos(a) * radiusDomain, y: centerY + Math.sin(a) * radiusDomain };
   });

   const byDomain = {};
   featureWeb.nodes.forEach(n => {
    const d = n.domain || 'game';
    if (!byDomain[d]) byDomain[d] = [];
    byDomain[d].push(n);
   });

   Object.entries(byDomain).forEach(([d, nodes]) => {
    const dc = domainCenters[d] || { x: centerX, y: centerY };
    const planets = nodes.filter(n => n.type !== 'subfeature');
    const moons = nodes.filter(n => n.type === 'subfeature');
    planets.forEach((p, i) => {
      const a = (i / Math.max(1, planets.length)) * Math.PI * 2;
      const r = Math.max(28, 24 + planets.length * 2);
      p.x = Math.max(6, Math.min(width - 140, dc.x + Math.cos(a) * r));
      p.y = Math.max(6, Math.min(height - 60, dc.y + Math.sin(a) * r));
    });
    moons.forEach((m, i) => {
      const parent = planets.length ? planets[i % planets.length] : null;
      if (parent) {
        const a = (i / Math.max(1, moons.length)) * Math.PI * 2;
        const r = 18 + (i % 3) * 8;
        m.x = Math.max(6, Math.min(width - 140, parent.x + Math.cos(a) * r));
        m.y = Math.max(6, Math.min(height - 60, parent.y + Math.sin(a) * r));
      } else {
        const a = (i / Math.max(1, moons.length)) * Math.PI * 2;
        m.x = Math.max(6, Math.min(width - 140, dc.x + Math.cos(a) * 20));
        m.y = Math.max(6, Math.min(height - 60, dc.y + Math.sin(a) * 20));
      }
    });
   });

   if (viewEl) viewEl.value = 'orbit';
   featureWebSaveFilters();
   featureWebPersistAndRefresh();
   featureWebNotice('Applied orbit layout to ' + featureWeb.nodes.length + ' nodes');
  }

  function featureWebFitToView() {
   const canvas = document.getElementById('feature-web-canvas');
   if (!canvas || featureWeb.nodes.length < 2) {
    if (!featureWeb.nodes.length) featureWebNotice('Add nodes before using Fit to View');
    else featureWebNotice('Need at least 2 nodes to fit view');
    return;
   }
   let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
   featureWeb.nodes.forEach(n => {
    minX = Math.min(minX, n.x);
    minY = Math.min(minY, n.y);
    maxX = Math.max(maxX, n.x);
    maxY = Math.max(maxY, n.y);
   });
   const width = Math.max(220, canvas.clientWidth);
   const height = Math.max(180, canvas.clientHeight);
   const spanX = Math.max(1, maxX - minX);
   const spanY = Math.max(1, maxY - minY);
   const targetW = Math.max(1, width - 140);
   const targetH = Math.max(1, height - 80);
   const scale = Math.min(targetW / spanX, targetH / spanY, 1.8);
   featureWeb.nodes.forEach(n => {
    n.x = 20 + (n.x - minX) * scale;
    n.y = 20 + (n.y - minY) * scale;
    n.x = Math.max(4, Math.min(width - 120, n.x));
    n.y = Math.max(4, Math.min(height - 48, n.y));
   });
   featureWebPersistAndRefresh();
   featureWebNotice('Fit ' + featureWeb.nodes.length + ' nodes to view');
  }

  function featureWebExport() {
   const blob = new Blob([JSON.stringify(featureWeb, null, 2)], { type: 'application/json' });
   const a = document.createElement('a');
   const stamp = new Date().toISOString().replace(/[:]/g, '-').replace(/\.\d{3}Z$/, 'Z');
   a.href = URL.createObjectURL(blob);
   a.download = 'feature-web-' + stamp + '.json';
   document.body.appendChild(a);
   a.click();
   document.body.removeChild(a);
   setTimeout(() => URL.revokeObjectURL(a.href), 500);
   featureWebNotice('Exported Feature Web JSON (' + featureWeb.nodes.length + ' nodes, ' + featureWeb.edges.length + ' links)');
  }

  function featureWebImport() {
   const input = document.createElement('input');
   input.type = 'file';
   input.accept = 'application/json,.json';
   input.onchange = () => {
    const file = input.files && input.files[0];
    if (!file) return;
    const maxBytes = 5 * 1024 * 1024;
    const fileLabel = file.name ? ('"' + file.name + '"') : 'Selected file';
    if (file.size <= 0) {
     featureWebNotice(fileLabel + ' is empty');
     return;
    }
    if (file.size > maxBytes) {
     featureWebNotice(fileLabel + ' is too large (' + (file.size / (1024 * 1024)).toFixed(1) + 'MB). Max 5MB.');
     return;
    }
    const hasJsonExt = !!(file.name && /\.json$/i.test(file.name));
    const hasJsonMime = typeof file.type === 'string' && /json/i.test(file.type);
    if (!hasJsonExt && !hasJsonMime) {
     featureWebNotice(fileLabel + ' is not recognized as JSON (.json file)');
     return;
    }
    const reader = new FileReader();
    reader.onload = () => {
     try {
      const data = JSON.parse(String(reader.result || '{}'));
      if (!Array.isArray(data.nodes) || !Array.isArray(data.edges)) throw new Error('Invalid schema');
      const normalized = featureWebNormalize(data, true);
      featureWeb = normalized.graph;
      fwSelectedNodeId = '';
      featureWebSaveUiState();
      featureWebConstrainNodesToCanvas();
      featureWebPersistAndRefresh();
      const importLabel = file && file.name ? ('Imported ' + file.name) : 'Imported JSON';
      featureWebNoticeImportReport(normalized.report, importLabel);
     } catch {
      const badLabel = file && file.name ? ('Invalid Feature Web JSON in ' + file.name) : 'Invalid Feature Web JSON';
      featureWebNotice(badLabel + ' (expected {nodes:[], edges:[]})');
     }
    };
    reader.onerror = () => {
     const readLabel = file && file.name ? ('Could not read ' + file.name) : 'Could not read Feature Web JSON file';
     featureWebNotice(readLabel);
    };
    reader.readAsText(file);
   };
   input.click();
  }

  function featureWebReset() {
   const nodeCount = featureWeb.nodes.length;
   const edgeCount = featureWeb.edges.length;
   if (!nodeCount && !edgeCount) {
    featureWebNotice('Feature Web is already empty');
    return;
   }
   if (!confirm('Reset Feature Web? This clears ' + nodeCount + ' node' + (nodeCount === 1 ? '' : 's') + ' and ' + edgeCount + ' link' + (edgeCount === 1 ? '' : 's') + '.')) return;
   featureWeb = { nodes: [], edges: [] };
   fwSelectedNodeId = '';
   featureWebSaveUiState();
   featureWebPersistAndRefresh();
   featureWebNotice('Feature Web reset');
  }

  async function featureWebCopyShare() {
   const payload = JSON.stringify(featureWeb);
   const copiedMsg = 'Copied Feature Web JSON (' + featureWeb.nodes.length + ' nodes, ' + featureWeb.edges.length + ' links)';
   try {
    if (navigator.clipboard && navigator.clipboard.writeText) {
     await navigator.clipboard.writeText(payload);
     featureWebNotice(copiedMsg);
     return;
    }
   } catch {}
   const ta = document.createElement('textarea');
   ta.value = payload;
   ta.style.position = 'fixed';
   ta.style.left = '-9999px';
   document.body.appendChild(ta);
   ta.select();
   try { document.execCommand('copy'); featureWebNotice(copiedMsg); }
   catch { featureWebNotice('Could not copy JSON'); }
   document.body.removeChild(ta);
  }

  async function featureWebPasteJson() {
   let raw = '';
   try {
    if (navigator.clipboard && navigator.clipboard.readText) raw = await navigator.clipboard.readText();
   } catch {}
   if (!raw) raw = prompt('Paste Feature Web JSON');
   if (!raw) return;
   raw = String(raw).trim();
   if (!raw) {
    featureWebNotice('Paste JSON payload is empty');
    return;
   }
   const maxBytes = 5 * 1024 * 1024;
   const rawBytes = (typeof TextEncoder !== 'undefined') ? new TextEncoder().encode(raw).length : raw.length;
   if (rawBytes > maxBytes) {
    featureWebNotice('Pasted JSON is too large (' + (rawBytes / (1024 * 1024)).toFixed(1) + 'MB). Max 5MB.');
    return;
   }
   try {
    const data = JSON.parse(raw);
    if (!Array.isArray(data.nodes) || !Array.isArray(data.edges)) throw new Error('Invalid schema');
    const normalized = featureWebNormalize(data, true);
    featureWeb = normalized.graph;
    fwSelectedNodeId = '';
    featureWebSaveUiState();
    featureWebConstrainNodesToCanvas();
    featureWebPersistAndRefresh();
    featureWebNoticeImportReport(normalized.report, 'Pasted JSON (' + (rawBytes / 1024).toFixed(1) + 'KB)');
   } catch {
    featureWebNotice('Invalid Feature Web JSON (expected {nodes:[], edges:[]})');
   }
  }

  function featureWebRender() {
   const wrap = document.getElementById('feature-web');
   if (!wrap) return;
   const nodesEl = document.getElementById('feature-web-nodes');
   const edgesEl = document.getElementById('feature-web-edges');
   const metaEl = document.getElementById('feature-web-meta');
   const fromEl = document.getElementById('fw-link-from');
   const toEl = document.getElementById('fw-link-to');
   const editEl = document.getElementById('fw-edit-node');
   const edgeListEl = document.getElementById('fw-edge-list');
   const filterTypeEl = document.getElementById('fw-filter-type');
   const filterStatusEl = document.getElementById('fw-filter-status');
   const filterDomainEl = document.getElementById('fw-filter-domain');
   const legendEl = document.getElementById('fw-legend');
   const statusSummaryEl = document.getElementById('fw-status-summary');
   const linkSummaryEl = document.getElementById('fw-link-summary');
   const ownerSummaryEl = document.getElementById('fw-owner-summary');
   if (!nodesEl || !edgesEl) return;

   const filterType = (filterTypeEl && filterTypeEl.value) || 'all';
   const filterStatus = (filterStatusEl && filterStatusEl.value) || 'all';
   const filterDomain = (filterDomainEl && filterDomainEl.value) || 'all';
   const filterSearchEl = document.getElementById('fw-filter-search');
   const linkedOnlyEl = document.getElementById('fw-linked-only');
   const ownedOnlyEl = document.getElementById('fw-owned-only');
   const showEdgeLabelsEl = document.getElementById('fw-show-edge-labels');
   const viewModeEl = document.getElementById('fw-view-mode');
   const focusSelectedEl = document.getElementById('fw-focus-selected');
   const simpleUiEl = document.getElementById('fw-simple-ui');
   const clearSearchBtnEl = document.getElementById('fw-clear-search-btn');
   const clearFiltersBtnEl = document.getElementById('fw-clear-filters-btn');
   const searchMetaEl = document.getElementById('fw-search-meta');
   const viewMode = (viewModeEl && viewModeEl.value) || 'cards';
   const simpleUi = !(simpleUiEl && !simpleUiEl.checked);
   const showEdgeLabels = !showEdgeLabelsEl || (!!showEdgeLabelsEl.checked && !simpleUi);
   const linkedOnly = !!(linkedOnlyEl && linkedOnlyEl.checked);
   const ownedOnly = !!(ownedOnlyEl && ownedOnlyEl.checked);
   const focusSelected = !!(focusSelectedEl && focusSelectedEl.checked && fwSelectedNodeId);
   const filterSearch = ((filterSearchEl && filterSearchEl.value) || '').trim().slice(0, FW_TEXT_LIMITS.search).toLowerCase();
   if (clearSearchBtnEl) {
    const hasSearch = !!filterSearch;
    clearSearchBtnEl.disabled = !hasSearch;
    clearSearchBtnEl.setAttribute('aria-hidden', hasSearch ? 'false' : 'true');
    clearSearchBtnEl.style.opacity = hasSearch ? '1' : '0.45';
    clearSearchBtnEl.style.cursor = hasSearch ? 'pointer' : 'default';
    clearSearchBtnEl.style.visibility = hasSearch ? 'visible' : 'hidden';
    clearSearchBtnEl.title = hasSearch ? ('Clear current search text (' + filterSearch.length + ' chars)') : 'Search is already empty';
    clearSearchBtnEl.textContent = hasSearch ? ('Clear Search (' + filterSearch.length + ')') : 'Clear Search';
   }
   if (clearFiltersBtnEl) {
    const activeFilterCount = (filterType !== 'all' ? 1 : 0)
     + (filterStatus !== 'all' ? 1 : 0)
     + (filterDomain !== 'all' ? 1 : 0)
     + (viewMode !== 'orbit' ? 1 : 0)
     + (filterSearch ? 1 : 0)
     + (linkedOnly ? 1 : 0)
     + (ownedOnly ? 1 : 0)
     + (showEdgeLabels ? 1 : 0)
     + ((focusSelectedEl && !focusSelectedEl.checked) ? 1 : 0)
     + ((simpleUiEl && !simpleUiEl.checked) ? 1 : 0);
    const atDefaults = activeFilterCount === 0;
    clearFiltersBtnEl.disabled = atDefaults;
    clearFiltersBtnEl.setAttribute('aria-hidden', atDefaults ? 'true' : 'false');
    clearFiltersBtnEl.style.opacity = atDefaults ? '0.45' : '1';
    clearFiltersBtnEl.style.cursor = atDefaults ? 'default' : 'pointer';
    clearFiltersBtnEl.style.visibility = atDefaults ? 'hidden' : 'visible';
    clearFiltersBtnEl.title = atDefaults ? 'Filters already at defaults' : ('Reset all filters to defaults (' + activeFilterCount + ' active)');
    clearFiltersBtnEl.textContent = atDefaults ? 'Clear Filters' : ('Clear Filters (' + activeFilterCount + ')');
   }
   const canvasEl = document.getElementById('feature-web-canvas');
   if (canvasEl) {
    if (viewMode === 'constellation' || viewMode === 'orbit') {
     canvasEl.style.background = 'radial-gradient(circle at 20% 20%, rgba(127,198,255,0.12), transparent 35%), radial-gradient(circle at 80% 30%, rgba(171,120,255,0.10), transparent 38%), radial-gradient(circle at 50% 80%, rgba(120,255,173,0.08), transparent 42%), #0a1019';
     canvasEl.style.border = '1px solid rgba(150,190,255,0.22)';
    } else {
     canvasEl.style.background = 'radial-gradient(circle at 20% 20%, rgba(255,136,17,0.08), transparent 35%),var(--bg-input,#111)';
     canvasEl.style.border = '1px dashed var(--border)';
    }
   }
   document.querySelectorAll('.fw-pro').forEach(el => {
    el.style.display = simpleUi ? 'none' : '';
    el.setAttribute('aria-hidden', simpleUi ? 'true' : 'false');
   });
   let visibleNodes = featureWeb.nodes.filter(n => (filterType === 'all' || n.type === filterType) && (filterStatus === 'all' || n.status === filterStatus) && (filterDomain === 'all' || (n.domain || 'game') === filterDomain) && (!linkedOnly || featureWebNormalizeTaskId(n && n.taskId) != null) && (!ownedOnly || !!(n.owner && n.owner.trim())) && (!filterSearch || ((n.title || '').toLowerCase().includes(filterSearch) || (n.summary || '').toLowerCase().includes(filterSearch) || (n.teach || '').toLowerCase().includes(filterSearch))));
   let visibleNodeIds = new Set(visibleNodes.map(n => n.id));
   let visibleEdges = featureWeb.edges.filter(e => visibleNodeIds.has(e.from) && visibleNodeIds.has(e.to));
   const hasSelectedNode = !!fwSelectedNodeId && featureWeb.nodes.some(n => n && n.id === fwSelectedNodeId);
   if (fwSelectedNodeId && !hasSelectedNode) {
    fwSelectedNodeId = '';
    featureWebSaveUiState();
   }
   if (focusSelected && hasSelectedNode) {
    const neigh = new Set([fwSelectedNodeId]);
    visibleEdges.forEach(e => { if (e.from === fwSelectedNodeId) neigh.add(e.to); if (e.to === fwSelectedNodeId) neigh.add(e.from); });
    visibleNodes = visibleNodes.filter(n => neigh.has(n.id));
    visibleNodeIds = new Set(visibleNodes.map(n => n.id));
    visibleEdges = visibleEdges.filter(e => visibleNodeIds.has(e.from) && visibleNodeIds.has(e.to));
   }

   fwLastVisibleNodeIds = visibleNodes.map(n => n.id);

   if (searchMetaEl) {
    if (filterSearch) {
     const noMatches = visibleNodes.length === 0;
     const totalNodes = Math.max(1, featureWeb.nodes.length);
     const matchPct = Math.round((visibleNodes.length / totalNodes) * 100);
     searchMetaEl.style.display = 'inline';
     searchMetaEl.setAttribute('aria-hidden', 'false');
     searchMetaEl.tabIndex = 0;
     searchMetaEl.textContent = 'Matches: ' + visibleNodes.length + '/' + featureWeb.nodes.length + ' (' + matchPct + '%)';
     searchMetaEl.setAttribute('aria-label', noMatches ? 'No search matches. Activate to clear search.' : ('Search matches: ' + visibleNodes.length + ' of ' + featureWeb.nodes.length + '. Activate to clear search.'));
     searchMetaEl.style.color = noMatches ? '#e88' : 'var(--text-muted)';
     searchMetaEl.style.fontWeight = noMatches ? '700' : '400';
     searchMetaEl.style.cursor = 'pointer';
     searchMetaEl.title = noMatches ? 'No nodes match current search + filters. Click to clear search (or use Ctrl+Shift+Q).' : ('Visible nodes after current filters and search (' + matchPct + '%). Press Enter/Home/↓ to focus first result, End/↑ to focus last, Shift+Enter to teach, or click here to clear search (Ctrl+Shift+Q).');
    } else {
     searchMetaEl.style.display = 'none';
     searchMetaEl.setAttribute('aria-hidden', 'true');
     searchMetaEl.tabIndex = -1;
     searchMetaEl.setAttribute('aria-label', 'Search matches');
     searchMetaEl.textContent = '';
     searchMetaEl.style.color = 'var(--text-muted)';
     searchMetaEl.style.fontWeight = '400';
     searchMetaEl.style.cursor = 'default';
     searchMetaEl.title = '';
    }
   }

   if (metaEl) {
    const savedSuffix = fwLastSavedAt ? (' · saved ' + new Date(fwLastSavedAt).toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' })) : '';
    const nodeRemaining = featureWebRemainingNodeCapacity();
    const edgeRemaining = featureWebRemainingEdgeCapacity();
    const nearCapacity = nodeRemaining <= 25 || edgeRemaining <= 100;
    const capSuffix = nearCapacity ? (' · cap left: ' + nodeRemaining + ' nodes, ' + edgeRemaining + ' links') : '';
    metaEl.textContent = visibleNodes.length + '/' + featureWeb.nodes.length + ' nodes · ' + visibleEdges.length + '/' + featureWeb.edges.length + ' links' + savedSuffix + capSuffix;
    metaEl.style.color = nearCapacity ? '#e8b56a' : 'var(--text-muted)';
    metaEl.title = nearCapacity
     ? ('Near graph capacity. Remaining: ' + nodeRemaining + ' nodes, ' + edgeRemaining + ' links (max ' + FW_MAX_NODES + '/' + FW_MAX_EDGES + ').')
     : ('Visible nodes/links under current filters. Capacity remaining: ' + nodeRemaining + ' nodes, ' + edgeRemaining + ' links.');
   }
   const powerEl = document.getElementById('sys-stock-power');
   const waterEl = document.getElementById('sys-stock-water');
   const foodEl = document.getElementById('sys-stock-food');
   const netEl = document.getElementById('sys-stock-net');
   const total = Math.max(1, featureWeb.nodes.length);
   const activeCount = featureWeb.nodes.filter(n => n.status === 'active').length;
   const doneCount = featureWeb.nodes.filter(n => n.status === 'done').length;
   if (powerEl) powerEl.textContent = Math.round((activeCount / total) * 100) + '%';
   if (waterEl) waterEl.textContent = Math.round((doneCount / total) * 100) + '%';
   if (foodEl) foodEl.textContent = String(featureWeb.nodes.filter(n => (n.domain||'')==='game').length);
   if (netEl) netEl.textContent = (100 - Math.round((featureWeb.nodes.filter(n => n.status==='blocked').length / total) * 100)) + '%';

   const quickListEl = document.getElementById('fw-quick-list');
   if (quickListEl) {
    const ranked = [...visibleNodes].sort((a,b) => {
      const wa = ((a.priority==='critical')?4:(a.priority==='high'?3:(a.priority==='medium'?2:1))) + featureWeb.edges.filter(e=>e.from===a.id||e.to===a.id).length * 0.1;
      const wb = ((b.priority==='critical')?4:(b.priority==='high'?3:(b.priority==='medium'?2:1))) + featureWeb.edges.filter(e=>e.from===b.id||e.to===b.id).length * 0.1;
      return wb - wa;
    }).slice(0, simpleUi ? 16 : 10);
    quickListEl.innerHTML = ranked.map(n => '<button role="listitem" aria-label="Focus Feature Web node ' + escHtml(String(n.title || '').replace(/"/g, '&quot;')) + '" aria-pressed="' + (n.id===fwSelectedNodeId?'true':'false') + '" aria-current="' + (n.id===fwSelectedNodeId?'true':'false') + '" onclick="featureWebFocusNode(\'' + n.id + '\');featureWebCenterSelected({quiet:true});" style="background:' + (n.id===fwSelectedNodeId?'rgba(120,180,255,0.25)':'rgba(255,255,255,0.06)') + ';border:1px solid rgba(255,255,255,0.14);color:var(--text);padding:0.2rem 0.45rem;border-radius:999px;font-size:0.68rem;cursor:pointer;">' + escHtml(n.title) + '</button>').join('');
    quickListEl.style.display = simpleUi ? 'flex' : 'none';
    quickListEl.setAttribute('aria-hidden', simpleUi ? 'false' : 'true');
    quickListEl.setAttribute('aria-live', simpleUi ? 'polite' : 'off');
   }

   if (legendEl) {
    const typeEntries = Object.entries(FW_TYPE_COLORS);
    const statusEntries = Object.entries(FW_STATUS_COLORS);
    const domainEntries = Object.entries(FW_DOMAIN_COLORS);
    const typeLegendEntries = simpleUi ? typeEntries.slice(0, 4) : typeEntries;
    const domainLegendEntries = simpleUi ? domainEntries.slice(0, 5) : domainEntries;
    const typeBits = typeLegendEntries.map(([k,v]) => '<span style="display:inline-flex;align-items:center;gap:0.2rem;"><span style="width:8px;height:8px;border-radius:50%;background:'+v+';display:inline-block;"></span>'+escHtml(k)+'</span>');
    const statusBits = statusEntries.map(([k,v]) => '<span style="display:inline-flex;align-items:center;gap:0.2rem;"><span style="width:8px;height:8px;border-radius:2px;background:'+v+';display:inline-block;"></span>'+escHtml(k)+'</span>');
    const domainBits = domainLegendEntries.map(([k,v]) => '<span style="display:inline-flex;align-items:center;gap:0.2rem;"><span style="width:8px;height:8px;border-radius:50%;background:'+v+';display:inline-block;"></span>'+escHtml(k)+'</span>');
    const typeMore = simpleUi && typeEntries.length > typeLegendEntries.length ? '<span style="opacity:0.55;">+' + (typeEntries.length - typeLegendEntries.length) + ' more</span>' : '';
    const domainMore = simpleUi && domainEntries.length > domainLegendEntries.length ? '<span style="opacity:0.55;">+' + (domainEntries.length - domainLegendEntries.length) + ' more</span>' : '';
    legendEl.innerHTML = '<span style="opacity:0.75;">Type:</span> ' + typeBits.join('<span style="opacity:0.35;">·</span>') + (typeMore ? ' <span style="opacity:0.35;">·</span> ' + typeMore : '') + ' <span style="opacity:0.45;margin:0 0.2rem;">|</span> <span style="opacity:0.75;">Status:</span> ' + statusBits.join('<span style="opacity:0.35;">·</span>') + ' <span style="opacity:0.45;margin:0 0.2rem;">|</span> <span style="opacity:0.75;">Domain:</span> ' + domainBits.join('<span style="opacity:0.35;">·</span>') + (domainMore ? ' <span style="opacity:0.35;">·</span> ' + domainMore : '');
   }

   if (statusSummaryEl) {
    const counts = { planned: 0, active: 0, blocked: 0, done: 0 };
    visibleNodes.forEach(n => { if (counts[n.status] != null) counts[n.status]++; });
    const donePct = visibleNodes.length ? Math.round((counts.done / visibleNodes.length) * 100) : 0;
    const activePct = visibleNodes.length ? Math.round((counts.active / visibleNodes.length) * 100) : 0;
    const blockedPct = visibleNodes.length ? Math.round((counts.blocked / visibleNodes.length) * 100) : 0;
    const statusChips = Object.entries(counts).map(([k, c]) => {
     const color = FW_STATUS_COLORS[k] || '#666';
     return '<span style="display:inline-flex;align-items:center;gap:0.25rem;border:1px solid var(--border);background:rgba(255,255,255,0.04);padding:0.12rem 0.4rem;border-radius:999px;color:var(--text-muted);"><span style="width:7px;height:7px;border-radius:2px;background:' + color + ';display:inline-block;"></span>' + k + ': <strong style="color:var(--text);font-size:0.67rem;">' + c + '</strong></span>';
    }).join('');
    const completionChip = '<span style="display:inline-flex;align-items:center;gap:0.25rem;border:1px solid rgba(61,125,216,0.45);background:rgba(61,125,216,0.12);padding:0.12rem 0.4rem;border-radius:999px;color:#b7d0ff;">Completion: <strong style="color:#d7e6ff;font-size:0.67rem;">' + donePct + '%</strong></span>';
    const activeChip = '<span style="display:inline-flex;align-items:center;gap:0.25rem;border:1px solid rgba(47,143,78,0.45);background:rgba(47,143,78,0.12);padding:0.12rem 0.4rem;border-radius:999px;color:#b8e7c6;">Active: <strong style="color:#dbf7e3;font-size:0.67rem;">' + activePct + '%</strong></span>';
    const blockedChip = '<span style="display:inline-flex;align-items:center;gap:0.25rem;border:1px solid rgba(229,85,85,0.45);background:rgba(229,85,85,0.12);padding:0.12rem 0.4rem;border-radius:999px;color:#f1bbbb;">Blocked: <strong style="color:#ffd6d6;font-size:0.67rem;">' + blockedPct + '%</strong></span>';
    statusSummaryEl.innerHTML = statusChips + completionChip + activeChip + blockedChip;
   }

   if (linkSummaryEl) {
    const edgeCounts = { depends_on: 0, blocks: 0, relates_to: 0, teaches: 0, enables: 0 };
    const totalVisibleLinks = visibleEdges.length;
    visibleEdges.forEach(e => { edgeCounts[e.type || 'depends_on'] = (edgeCounts[e.type || 'depends_on'] || 0) + 1; });
    const edgeColors = { depends_on: 'rgba(255,255,255,0.55)', blocks: 'rgba(229,85,85,0.75)', relates_to: 'rgba(140,140,140,0.75)', teaches: 'rgba(84,194,255,0.8)', enables: 'rgba(120,210,110,0.8)' };
    const chips = Object.entries(edgeCounts).filter(([,c]) => c > 0).map(([k, c]) =>
     '<span style="display:inline-flex;align-items:center;gap:0.25rem;border:1px solid var(--border);background:rgba(255,255,255,0.04);padding:0.12rem 0.4rem;border-radius:999px;color:var(--text-muted);"><span style="width:8px;height:2px;background:' + (edgeColors[k] || 'rgba(255,255,255,0.55)') + ';display:inline-block;"></span>' + escHtml((k || '').split('_').join(' ')) + ': <strong style="color:var(--text);font-size:0.67rem;">' + c + '</strong></span>'
    ).join('');
    linkSummaryEl.innerHTML = chips
     ? ('<span style="display:inline-flex;align-items:center;gap:0.25rem;border:1px solid var(--border);background:rgba(255,255,255,0.04);padding:0.12rem 0.4rem;border-radius:999px;color:var(--text-muted);">Total links: <strong style="color:var(--text);font-size:0.67rem;">' + totalVisibleLinks + '</strong></span>' + chips)
     : '<span style="opacity:0.7;">No links in current filter</span>';
   }

   if (ownerSummaryEl) {
    const owned = visibleNodes.filter(n => n.owner && n.owner.trim());
    const linked = visibleNodes.filter(n => featureWebNormalizeTaskId(n && n.taskId) != null).length;
    const unlinked = Math.max(0, visibleNodes.length - linked);
    const linkedPct = visibleNodes.length ? Math.round((linked / visibleNodes.length) * 100) : 0;
    const totalChip = '<span style="display:inline-flex;align-items:center;gap:0.25rem;border:1px solid var(--border);background:rgba(255,255,255,0.04);padding:0.12rem 0.4rem;border-radius:999px;color:var(--text-muted);">Total nodes: <strong style="color:var(--text);font-size:0.67rem;">' + visibleNodes.length + '</strong></span>';
    const ownershipChip = owned.length
     ? '<span style="display:inline-flex;align-items:center;gap:0.25rem;border:1px solid var(--border);background:rgba(255,255,255,0.04);padding:0.12rem 0.4rem;border-radius:999px;color:var(--text-muted);">👥 Owned nodes: <strong style="color:var(--text);font-size:0.67rem;">' + owned.length + '</strong></span>'
     : '<span style="opacity:0.7;">No owned nodes in current filter</span>';
    const linkedChip = '<span style="display:inline-flex;align-items:center;gap:0.25rem;border:1px solid var(--border);background:rgba(255,255,255,0.04);padding:0.12rem 0.4rem;border-radius:999px;color:var(--text-muted);">🔗 Linked tasks: <strong style="color:var(--text);font-size:0.67rem;">' + linked + '</strong></span>';
    const unlinkedChip = '<span style="display:inline-flex;align-items:center;gap:0.25rem;border:1px solid var(--border);background:rgba(255,255,255,0.04);padding:0.12rem 0.4rem;border-radius:999px;color:var(--text-muted);">🧩 Unlinked nodes: <strong style="color:var(--text);font-size:0.67rem;">' + unlinked + '</strong></span>';
    const linkedPctChip = '<span style="display:inline-flex;align-items:center;gap:0.25rem;border:1px solid rgba(122,214,97,0.45);background:rgba(122,214,97,0.12);padding:0.12rem 0.4rem;border-radius:999px;color:#b8eab2;">Linked coverage: <strong style="color:#dbf6d8;font-size:0.67rem;">' + linkedPct + '%</strong></span>';
    ownerSummaryEl.innerHTML = totalChip + ownershipChip + linkedChip + unlinkedChip + linkedPctChip;
   }

   const options = ['<option value="">Link from…</option>'].concat(featureWeb.nodes.map(n => '<option value="' + n.id + '">' + escHtml(n.title) + '</option>'));
   const toOptions = ['<option value="">Link to…</option>'].concat(featureWeb.nodes.map(n => '<option value="' + n.id + '">' + escHtml(n.title) + '</option>'));
   const editOptions = ['<option value="">Edit node…</option>'].concat(featureWeb.nodes.map(n => '<option value="' + n.id + '">' + escHtml(n.title) + '</option>'));
   if (fromEl) {
    const selected = fromEl.value;
    fromEl.innerHTML = options.join('');
    fromEl.value = selected;
   }
   if (toEl) {
    const selected = toEl.value;
    toEl.innerHTML = toOptions.join('');
    toEl.value = selected;
   }
   if (editEl) {
    const selected = editEl.value;
    editEl.innerHTML = editOptions.join('');
    editEl.value = featureWeb.nodes.some(n => n.id === selected) ? selected : '';
   }
   featureWebRenderEditor();

   if (edgeListEl) {
    edgeListEl.setAttribute('aria-live', simpleUi ? 'off' : 'polite');
    edgeListEl.innerHTML = visibleEdges.length
     ? visibleEdges.map(e => {
       const from = featureWeb.nodes.find(n => n.id === e.from);
       const to = featureWeb.nodes.find(n => n.id === e.to);
       if (!from || !to) return '';
       const typeLabel = e.type === 'blocks' ? 'blocks' : (e.type === 'relates_to' ? 'relates to' : (e.type === 'teaches' ? 'teaches' : (e.type === 'enables' ? 'enables' : 'depends on')));
       return '<span role="listitem" style="display:inline-flex;align-items:center;gap:0.25rem;background:rgba(255,255,255,0.05);border:1px solid var(--border);border-radius:999px;padding:0.15rem 0.45rem;margin:0 0.3rem 0.3rem 0;">' + escHtml(from.title) + ' → ' + escHtml(to.title) + ' <button aria-label="Cycle link type" onclick="featureWebCycleEdgeType(\'' + e.id + '\')" style="background:none;border:none;color:var(--accent);cursor:pointer;font-size:0.66rem;padding:0;">(' + typeLabel + ')</button><button aria-label="Delete link" onclick="featureWebDeleteEdge(\'' + e.id + '\')" style="background:none;border:none;color:var(--text-muted);cursor:pointer;font-size:0.72rem;">✕</button></span>';
      }).join('')
     : '<span style="opacity:0.7;">No links in current filter</span>';
   }

   edgesEl.innerHTML = visibleEdges.map(e => {
    const from = featureWeb.nodes.find(n => n.id === e.from);
    const to = featureWeb.nodes.find(n => n.id === e.to);
    if (!from || !to) return '';
    const baseStroke = e.type === 'blocks' ? 'rgba(229,85,85,0.55)' : (e.type === 'relates_to' ? 'rgba(120,120,120,0.45)' : (e.type === 'teaches' ? 'rgba(94,194,255,0.65)' : (e.type === 'enables' ? 'rgba(122,214,97,0.65)' : 'rgba(255,255,255,0.35)')));
    const isHoverEdge = fwHoveredNodeId && (e.from === fwHoveredNodeId || e.to === fwHoveredNodeId);
    const stroke = isHoverEdge ? 'rgba(255,136,17,0.85)' : baseStroke;
    const strokeWidth = isHoverEdge ? '2.4' : '1.6';
    const dash = e.type === 'relates_to' ? ' stroke-dasharray="4 3"' : '';
    const x1 = from.x + 52, y1 = from.y + 20, x2 = to.x + 52, y2 = to.y + 20;
    const mx = (x1 + x2) / 2, my = (y1 + y2) / 2;
    const label = e.type === 'blocks' ? 'blocks' : (e.type === 'relates_to' ? 'relates' : (e.type === 'teaches' ? 'teaches' : (e.type === 'enables' ? 'enables' : 'depends')));
    const labelFill = isHoverEdge ? 'rgba(255,176,90,0.95)' : 'rgba(255,255,255,0.62)';
    const textHtml = showEdgeLabels ? ('<text x="' + mx + '" y="' + (my - 4) + '" text-anchor="middle" font-size="9" fill="' + labelFill + '">' + label + '</text>') : '';
    return '<g>' +
     '<line x1="' + x1 + '" y1="' + y1 + '" x2="' + x2 + '" y2="' + y2 + '" stroke="' + stroke + '" stroke-width="' + strokeWidth + '" marker-end="url(#fwArrow)"' + dash + ' />' +
     textHtml +
    '</g>';
   }).join('') + '<defs><marker id="fwArrow" markerWidth="10" markerHeight="7" refX="9" refY="3.5" orient="auto"><polygon points="0 0, 10 3.5, 0 7" fill="rgba(255,255,255,0.35)" /></marker></defs>';

   const domainAnchorHtml = (viewMode === 'orbit') ? (() => {
    const canvas = document.getElementById('feature-web-canvas');
    if (!canvas) return '';
    const w = Math.max(360, canvas.clientWidth), h = Math.max(240, canvas.clientHeight);
    const cx = w / 2, cy = h / 2;
    const domains = FW_ALLOWED_DOMAINS;
    const rr = Math.max(120, Math.min(w, h) * 0.34);
    return domains.map((d,i) => {
      const a = (i / domains.length) * Math.PI * 2 - Math.PI / 2;
      const x = cx + Math.cos(a) * rr;
      const y = cy + Math.sin(a) * rr;
      const c = FW_DOMAIN_COLORS[d] || '#89a';
      return '<div style="position:absolute;left:' + (x-22) + 'px;top:' + (y-22) + 'px;width:44px;height:44px;border-radius:50%;border:1px dashed rgba(180,210,255,0.22);box-shadow:inset 0 0 22px rgba(120,160,255,0.06);pointer-events:none;">' +
        '<span style="position:absolute;left:46px;top:12px;white-space:nowrap;font-size:0.58rem;color:' + c + ';text-transform:uppercase;opacity:0.75;">' + d + '</span></div>';
    }).join('');
   })() : '';

   nodesEl.innerHTML = domainAnchorHtml + visibleNodes.map(n => {
    const typeColor = FW_TYPE_COLORS[n.type] || '#888';
    const statusColor = FW_STATUS_COLORS[n.status] || '#666';
    const domainColor = FW_DOMAIN_COLORS[n.domain || 'game'] || '#888';
    const isSelected = fwSelectedNodeId === n.id;
    const borderColor = isSelected ? 'rgba(255,136,17,0.75)' : 'rgba(255,255,255,0.15)';
    const glow = isSelected ? '0 0 0 1px rgba(255,136,17,0.25),0 3px 14px rgba(0,0,0,0.45)' : '0 1px 8px rgba(0,0,0,0.35)';
    const linkedBadge = featureWebNormalizeTaskId(n && n.taskId) != null ? '<span style="font-size:0.56rem;color:#8fe0a7;background:rgba(47,143,78,0.2);padding:0.03rem 0.22rem;border-radius:999px;font-weight:700;">LINK</span>' : '';
    if (viewMode === 'constellation' || viewMode === 'orbit') {
     const isMoon = n.type === 'subfeature';
     const isStar = !isMoon && (n.priority === 'critical' || /mission|core|atlas|roadmap/i.test(String(n.title || '')));
     const incident = featureWeb.edges.reduce((acc,e)=> acc + ((e.from===n.id||e.to===n.id)?1:0), 0);
     const complexity = Math.min(10, incident + Math.ceil(((n.details||'').length + (n.summary||'').length) / 180));
     const base = viewMode === 'orbit' ? (isStar ? 18 : (isMoon ? 7 : 11)) : 12;
     const nodeSize = (isSelected ? 2 : 0) + base + (viewMode === 'orbit' ? Math.min(8, Math.floor(complexity / 2)) : 0);
     const baseColor = viewMode === 'orbit' ? (isStar ? '#ffd166' : (isMoon ? '#a6c8ff' : statusColor)) : statusColor;
     const aura = isSelected ? '0 0 18px rgba(255,136,17,0.55)' : (viewMode === 'orbit' && isStar ? '0 0 20px rgba(255,209,102,0.7)' : '0 0 12px rgba(255,255,255,0.25)');
     const labelColor = viewMode === 'orbit' ? 'rgba(210,230,255,0.92)' : 'var(--text-muted)';
     return '<div class="fw-node" role="listitem" tabindex="0" aria-keyshortcuts="Enter,Space" aria-label="Feature Web node ' + escHtml(String(n.title || '').replace(/"/g, '&quot;')) + '" title="' + escHtml((n.summary || n.teach || n.details || '').slice(0, 180)) + '" data-id="' + n.id + '" onclick="featureWebFocusNode(\'' + n.id + '\')" onkeydown="if(event.key===\'Enter\'||event.key===\' \'){event.preventDefault();featureWebFocusNode(\'' + n.id + '\');featureWebCenterSelected({quiet:true});}" onmouseenter="featureWebSetHover(\'' + n.id + '\')" onmouseleave="featureWebSetHover(\'\')" style="position:absolute;left:' + n.x + 'px;top:' + n.y + 'px;width:' + nodeSize + 'px;height:' + nodeSize + 'px;border-radius:50%;background:' + baseColor + ';box-shadow:' + aura + ';cursor:grab;border:1px solid ' + borderColor + ';">' +
      '<span style="position:absolute;left:' + (nodeSize + 6) + 'px;top:-1px;white-space:nowrap;font-size:0.64rem;color:' + labelColor + ';">' + escHtml(n.title) + '</span>' +
      '</div>';
    }
    return '<div class="fw-node" role="listitem" tabindex="0" aria-keyshortcuts="Enter,Space" aria-label="Feature Web node ' + escHtml(String(n.title || '').replace(/"/g, '&quot;')) + '" title="' + escHtml((n.summary || n.teach || n.details || '').slice(0, 180)) + '" data-id="' + n.id + '" onclick="featureWebFocusNode(\'' + n.id + '\')" onkeydown="if(event.key===\'Enter\'||event.key===\' \'){event.preventDefault();featureWebFocusNode(\'' + n.id + '\');featureWebCenterSelected({quiet:true});}" onmouseenter="featureWebSetHover(\'' + n.id + '\')" onmouseleave="featureWebSetHover(\'\')" style="position:absolute;left:' + n.x + 'px;top:' + n.y + 'px;min-width:104px;max-width:170px;padding:0.32rem 0.42rem;border-radius:8px;background:var(--bg-card,#1a1a1a);border:1px solid ' + borderColor + ';box-shadow:' + glow + ';cursor:grab;">' +
     '<div style="display:flex;justify-content:space-between;align-items:center;gap:0.2rem;"><span style="font-size:0.67rem;color:' + typeColor + ';font-weight:700;text-transform:uppercase;">' + escHtml(n.type) + '</span><span style="display:inline-flex;align-items:center;gap:0.22rem;"><span style="font-size:0.59rem;color:' + domainColor + ';text-transform:uppercase;">' + escHtml(n.domain || 'game') + '</span>' + linkedBadge + '</span></div>' +
     '<div style="font-size:0.76rem;color:var(--text);font-weight:600;line-height:1.3;word-break:break-word;">' + escHtml(n.title) + '</div>' +
     '<div style="display:flex;justify-content:space-between;align-items:center;margin-top:0.2rem;gap:0.3rem;">' +
      '<span style="font-size:0.63rem;color:' + statusColor + ';font-weight:700;text-transform:uppercase;">' + escHtml(n.status) + '</span>' +
      '<button aria-label="Delete node" onclick="event.stopPropagation();featureWebDeleteNode(\'' + n.id + '\')" style="background:none;border:none;color:var(--text-muted);font-size:0.72rem;cursor:pointer;">✕</button>' +
     '</div>' +
    '</div>';
   }).join('');

   nodesEl.querySelectorAll('.fw-node').forEach(el => {
    el.addEventListener('pointerdown', featureWebDragStart);
   });
  }

  function featureWebDragStart(e) {
   const id = e.currentTarget.dataset.id;
   const node = featureWeb.nodes.find(n => n.id === id);
   const canvas = document.getElementById('feature-web-canvas');
   if (!node || !canvas) return;
   const rect = canvas.getBoundingClientRect();
   fwDrag = { id, dx: e.clientX - rect.left - node.x, dy: e.clientY - rect.top - node.y, rect };
   e.currentTarget.setPointerCapture(e.pointerId);
  }

  document.addEventListener('pointermove', (e) => {
   if (!fwDrag) return;
   const node = featureWeb.nodes.find(n => n.id === fwDrag.id);
   if (!node) return;
   node.x = Math.max(4, Math.min(fwDrag.rect.width - 120, e.clientX - fwDrag.rect.left - fwDrag.dx));
   node.y = Math.max(4, Math.min(fwDrag.rect.height - 48, e.clientY - fwDrag.rect.top - fwDrag.dy));
   featureWebRender();
  });

  document.addEventListener('pointerup', () => {
   if (!fwDrag) return;
   fwDrag = null;
   featureWebSave();
  });

  document.addEventListener('keydown', (e) => {
   const active = document.activeElement;
   const tag = active && active.tagName ? active.tagName.toLowerCase() : '';
   const inField = tag === 'input' || tag === 'textarea' || tag === 'select' || (active && active.isContentEditable);
   if (!inField && e.key === '/') {
    const searchEl = document.getElementById('fw-filter-search');
    if (searchEl) {
     e.preventDefault();
     searchEl.focus();
     searchEl.select();
     return;
    }
   }
   if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === '?') {
    const searchEl = document.getElementById('fw-filter-search');
    if (searchEl) {
     e.preventDefault();
     searchEl.focus();
     searchEl.select();
    }
    return;
   }
   if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'q' || e.key === 'Q')) {
    e.preventDefault();
    featureWebClearSearch();
    return;
   }
   if (!inField && (e.key === 'k' || e.key === 'K')) {
    const addTitleEl = document.getElementById('fw-node-title');
    if (addTitleEl) {
     e.preventDefault();
     addTitleEl.focus();
     addTitleEl.select();
     return;
    }
   }
   if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'k' || e.key === 'K')) {
    const addOwnerEl = document.getElementById('fw-node-owner');
    if (addOwnerEl) {
     e.preventDefault();
     addOwnerEl.focus();
     addOwnerEl.select();
    }
    return;
   }
   if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'n' || e.key === 'N')) {
    const addTypeEl = document.getElementById('fw-node-type');
    if (addTypeEl) {
     e.preventDefault();
     addTypeEl.focus();
    }
    return;
   }
   if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'b' || e.key === 'B')) {
    const addDomainEl = document.getElementById('fw-node-domain');
    if (addDomainEl) {
     e.preventDefault();
     addDomainEl.focus();
    }
    return;
   }
   if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'm' || e.key === 'M')) {
    const addStatusEl = document.getElementById('fw-node-status');
    if (addStatusEl) {
     e.preventDefault();
     addStatusEl.focus();
    }
    return;
   }
   if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'a' || e.key === 'A')) {
    e.preventDefault();
    featureWebAddNode();
    return;
   }
   if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'h' || e.key === 'H')) {
    e.preventDefault();
    featureWebToggleShortcutHint();
    return;
   }
   if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'r' || e.key === 'R')) {
    e.preventDefault();
    featureWebAutoLayout();
    return;
   }
   if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'c' || e.key === 'C')) {
    e.preventDefault();
    featureWebConstellationLayout();
    return;
   }
   if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === 'y' || e.key === 'Y')) {
    e.preventDefault();
    featureWebOrbitLayout();
    return;
   }
   if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === '1' || e.key === '2' || e.key === '3')) {
    const viewEl = document.getElementById('fw-view-mode');
    if (viewEl) {
     e.preventDefault();
     viewEl.value = e.key === '1' ? 'cards' : (e.key === '2' ? 'constellation' : 'orbit');
     featureWebSaveFilters();
     featureWebRender();
    }
    return;
   }
   if (!inField && fwSelectedNodeId && (e.key === 'a' || e.key === 'A')) {
    const editTitleEl = document.getElementById('fw-edit-title');
    if (editTitleEl) {
     e.preventDefault();
     editTitleEl.focus();
     editTitleEl.select();
     return;
    }
   }
   if (!inField && fwSelectedNodeId && (e.key === 'y' || e.key === 'Y') && (e.ctrlKey || e.metaKey)) {
    const ownerEl = document.getElementById('fw-edit-owner');
    if (ownerEl) {
     e.preventDefault();
     ownerEl.focus();
     ownerEl.select();
     return;
    }
   }
   if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
    e.preventDefault();
    featureWebSaveNodeEdits();
    featureWebNotice('Node saved');
    return;
   }
   if ((e.ctrlKey || e.metaKey) && (e.key === 's' || e.key === 'S') && e.shiftKey) {
    e.preventDefault();
    featureWebSaveNodeEdits();
    featureWebSyncNodesToBoard();
    featureWebNotice('Node saved + synced');
    return;
   }
   if ((e.ctrlKey || e.metaKey) && (e.key === 's' || e.key === 'S')) {
    e.preventDefault();
    featureWebSaveNodeEdits();
    featureWebNotice('Node saved');
    return;
   }
   if ((e.ctrlKey || e.metaKey) && (e.key === 'i' || e.key === 'I')) {
    e.preventDefault();
    featureWebCopySelectedId();
    return;
   }
   if (!inField && (e.ctrlKey || e.metaKey) && e.key === 'Enter') {
    const fromEl = document.getElementById('fw-link-from');
    const toEl = document.getElementById('fw-link-to');
    if (fromEl && toEl && fromEl.value && toEl.value) {
     e.preventDefault();
     featureWebAddEdge();
     return;
    }
   }
   if ((e.ctrlKey || e.metaKey) && (e.key === 'l' || e.key === 'L') && e.shiftKey) {
    const toEl = document.getElementById('fw-link-to');
    if (toEl) {
     e.preventDefault();
     toEl.focus();
    }
    return;
   }
   if ((e.ctrlKey || e.metaKey) && (e.key === 'e' || e.key === 'E') && !e.shiftKey) {
    const editSel = document.getElementById('fw-edit-node');
    if (editSel) {
     e.preventDefault();
     editSel.focus();
    }
    return;
   }
   if ((e.ctrlKey || e.metaKey) && (e.key === 't' || e.key === 'T') && !e.shiftKey) {
    const linkTypeEl = document.getElementById('fw-link-type');
    if (linkTypeEl) {
     e.preventDefault();
     linkTypeEl.focus();
    }
    return;
   }
   if ((e.ctrlKey || e.metaKey) && (e.key === 't' || e.key === 'T') && e.shiftKey) {
    const statusEl = document.getElementById('fw-edit-status');
    if (statusEl) {
     e.preventDefault();
     statusEl.focus();
    }
    return;
   }
   if ((e.ctrlKey || e.metaKey) && (e.key === 'e' || e.key === 'E') && e.shiftKey) {
    const domainEl = document.getElementById('fw-edit-domain');
    if (domainEl) {
     e.preventDefault();
     domainEl.focus();
    }
    return;
   }
   if ((e.ctrlKey || e.metaKey) && (e.key === 'p' || e.key === 'P') && !e.shiftKey) {
    const priorityEl = document.getElementById('fw-edit-priority');
    if (priorityEl) {
     e.preventDefault();
     priorityEl.focus();
    }
    return;
   }
   if ((e.ctrlKey || e.metaKey) && (e.key === 'p' || e.key === 'P') && e.shiftKey) {
    const statusFilterEl = document.getElementById('fw-filter-status');
    if (statusFilterEl) {
     e.preventDefault();
     statusFilterEl.focus();
    }
    return;
   }
   if ((e.ctrlKey || e.metaKey) && (e.key === 'd' || e.key === 'D') && e.shiftKey) {
    const domainFilterEl = document.getElementById('fw-filter-domain');
    if (domainFilterEl) {
     e.preventDefault();
     domainFilterEl.focus();
    }
    return;
   }
   if ((e.ctrlKey || e.metaKey) && (e.key === 'f' || e.key === 'F') && e.shiftKey) {
    const typeFilterEl = document.getElementById('fw-filter-type');
    if (typeFilterEl) {
     e.preventDefault();
     typeFilterEl.focus();
    }
    return;
   }
   if ((e.ctrlKey || e.metaKey) && (e.key === 'v' || e.key === 'V') && e.shiftKey) {
    const viewModeEl = document.getElementById('fw-view-mode');
    if (viewModeEl) {
     e.preventDefault();
     viewModeEl.focus();
    }
    return;
   }
   if ((e.ctrlKey || e.metaKey) && (e.key === 'o' || e.key === 'O') && e.shiftKey) {
    const ownedOnlyEl = document.getElementById('fw-owned-only');
    if (ownedOnlyEl) {
     e.preventDefault();
     ownedOnlyEl.focus();
    }
    return;
   }
   if ((e.ctrlKey || e.metaKey) && (e.key === 'j' || e.key === 'J') && e.shiftKey) {
    const linkedOnlyEl = document.getElementById('fw-linked-only');
    if (linkedOnlyEl) {
     e.preventDefault();
     linkedOnlyEl.focus();
    }
    return;
   }
   if ((e.ctrlKey || e.metaKey) && (e.key === 'w' || e.key === 'W') && e.shiftKey) {
    const linkedOnlyEl = document.getElementById('fw-linked-only');
    const ownedOnlyEl = document.getElementById('fw-owned-only');
    if (linkedOnlyEl && ownedOnlyEl) {
     e.preventDefault();
     const next = !(linkedOnlyEl.checked && ownedOnlyEl.checked);
     linkedOnlyEl.checked = next;
     ownedOnlyEl.checked = next;
     featureWebSaveFilters();
     featureWebRender();
     featureWebNotice(next ? 'Owned+linked filters enabled' : 'Owned+linked filters disabled');
    }
    return;
   }
   if ((e.ctrlKey || e.metaKey) && (e.key === 'x' || e.key === 'X') && e.shiftKey) {
    const focusSelectedEl = document.getElementById('fw-focus-selected');
    if (focusSelectedEl) {
     e.preventDefault();
     focusSelectedEl.focus();
    }
    return;
   }
   if ((e.ctrlKey || e.metaKey) && (e.key === 'u' || e.key === 'U') && e.shiftKey) {
    const simpleUiEl = document.getElementById('fw-simple-ui');
    if (simpleUiEl) {
     e.preventDefault();
     simpleUiEl.focus();
    }
    return;
   }
   if ((e.ctrlKey || e.metaKey) && (e.key === 'g' || e.key === 'G') && e.shiftKey) {
    const edgeLabelsEl = document.getElementById('fw-show-edge-labels');
    if (edgeLabelsEl) {
     e.preventDefault();
     edgeLabelsEl.focus();
    }
    return;
   }
   if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === '0') {
    e.preventDefault();
    featureWebClearFilters({ quiet: true });
    featureWebNotice('Filters cleared');
    return;
   }
   if ((e.ctrlKey || e.metaKey) && (e.key === 'l' || e.key === 'L')) {
    const fromEl = document.getElementById('fw-link-from');
    if (fromEl) {
     e.preventDefault();
     fromEl.focus();
    }
    return;
   }
   if (inField) {
    const el = document.activeElement;
    if (e.key === 'Enter' && el && el.id === 'fw-filter-search') {
     const q = (el.value || '').trim();
     if (q) {
      e.preventDefault();
      featureWebFocusFirstVisible();
      if (e.shiftKey) featureWebTeachMode();
     }
     return;
    }
    if (e.key === 'Home' && el && el.id === 'fw-filter-search') {
     const q = (el.value || '').trim();
     if (q) {
      e.preventDefault();
      featureWebFocusFirstVisible();
     }
     return;
    }
    if (e.key === 'End' && el && el.id === 'fw-filter-search') {
     const q = (el.value || '').trim();
     if (q) {
      e.preventDefault();
      featureWebFocusLastVisible();
     }
     return;
    }
    if (e.key === 'ArrowDown' && el && el.id === 'fw-filter-search') {
     const q = (el.value || '').trim();
     if (q) {
      e.preventDefault();
      featureWebFocusFirstVisible();
     }
     return;
    }
    if (e.key === 'ArrowUp' && el && el.id === 'fw-filter-search') {
     const q = (el.value || '').trim();
     if (q) {
      e.preventDefault();
      featureWebFocusLastVisible();
     }
     return;
    }
    if (e.key === 'Escape') {
     if (el && el.id === 'fw-filter-search' && el.value) {
      e.preventDefault();
      featureWebClearSearch({ quiet: true });
      return;
     }
     if (el && typeof el.blur === 'function') el.blur();
    }
    return;
   }
   if (e.key === 'Escape') {
    e.preventDefault();
    featureWebClearSelection({ quiet: true });
    return;
   }
   if ((e.key === 'Delete' || e.key === 'Backspace') && fwSelectedNodeId) {
    e.preventDefault();
    if ((e.ctrlKey || e.metaKey) && e.shiftKey) featureWebDeleteIncomingLinks();
    else if ((e.ctrlKey || e.metaKey) && e.altKey) featureWebDeleteOutgoingLinks();
    else if (e.ctrlKey || e.metaKey) featureWebDeleteSelectedLinks();
    else featureWebDeleteSelectedNode();
    return;
   }
   if (fwSelectedNodeId && (e.key === 'ArrowUp' || e.key === 'ArrowDown' || e.key === 'ArrowLeft' || e.key === 'ArrowRight')) {
    e.preventDefault();
    const step = e.shiftKey ? 24 : 8;
    if (e.key === 'ArrowUp') featureWebNudgeSelected(0, -step);
    else if (e.key === 'ArrowDown') featureWebNudgeSelected(0, step);
    else if (e.key === 'ArrowLeft') featureWebNudgeSelected(-step, 0);
    else if (e.key === 'ArrowRight') featureWebNudgeSelected(step, 0);
    return;
   }
   if (fwSelectedNodeId && (e.key === 'c' || e.key === 'C')) {
    e.preventDefault();
    featureWebCenterSelected();
    return;
   }
   if (fwSelectedNodeId && (e.key === 'd' || e.key === 'D')) {
    e.preventDefault();
    featureWebDuplicateSelected();
    return;
   }
   if (fwSelectedNodeId && (e.key === 'o' || e.key === 'O')) {
    e.preventDefault();
    featureWebOpenLinkedTask();
    return;
   }
   if (fwSelectedNodeId && (e.key === 't' || e.key === 'T')) {
    e.preventDefault();
    featureWebCopySelectedTitle();
    return;
   }
   if (e.key === 'f' || e.key === 'F') {
    e.preventDefault();
    featureWebFitToView();
    return;
   }
   if (e.key === 'r' || e.key === 'R') {
    e.preventDefault();
    featureWebAutoLayout();
    return;
   }
   if (e.key === 'h' || e.key === 'H') {
    e.preventDefault();
    featureWebTeachMode();
    return;
   }
   if (e.key === 'y' || e.key === 'Y') {
    const ownedOnlyEl = document.getElementById('fw-owned-only');
    if (ownedOnlyEl) {
     e.preventDefault();
     ownedOnlyEl.checked = !ownedOnlyEl.checked;
     featureWebSaveFilters();
     featureWebRender();
    }
    return;
   }
   if (e.key === 'l' || e.key === 'L') {
    const linkedOnlyEl = document.getElementById('fw-linked-only');
    if (linkedOnlyEl) {
     e.preventDefault();
     linkedOnlyEl.checked = !linkedOnlyEl.checked;
     featureWebSaveFilters();
     featureWebRender();
    }
    return;
   }
   if (e.key === 'e' || e.key === 'E') {
    const edgeLabelsEl = document.getElementById('fw-show-edge-labels');
    if (edgeLabelsEl) {
     e.preventDefault();
     edgeLabelsEl.checked = !edgeLabelsEl.checked;
     featureWebSaveFilters();
     featureWebRender();
    }
    return;
   }
   if (e.key === 'x' || e.key === 'X') {
    const focusEl = document.getElementById('fw-focus-selected');
    if (focusEl) {
     e.preventDefault();
     focusEl.checked = !focusEl.checked;
     featureWebSaveFilters();
     featureWebRender();
    }
    return;
   }
   if (e.key === 'u' || e.key === 'U') {
    const simpleEl = document.getElementById('fw-simple-ui');
    if (simpleEl) {
     e.preventDefault();
     simpleEl.checked = !simpleEl.checked;
     featureWebSaveFilters();
     featureWebRender();
    }
    return;
   }
   if (e.key === 'v' || e.key === 'V') {
    const viewEl = document.getElementById('fw-view-mode');
    if (viewEl) {
     e.preventDefault();
     const order = ['cards', 'constellation', 'orbit'];
     const idx = order.indexOf(viewEl.value);
     viewEl.value = order[(idx + 1) % order.length];
     featureWebSaveFilters();
     featureWebRender();
    }
    return;
   }
   if (e.key === '1' || e.key === '2' || e.key === '3') {
    const viewEl = document.getElementById('fw-view-mode');
    if (viewEl) {
     e.preventDefault();
     viewEl.value = e.key === '1' ? 'cards' : (e.key === '2' ? 'constellation' : 'orbit');
     featureWebSaveFilters();
     featureWebRender();
    }
    return;
   }
   if (e.key === 's' || e.key === 'S') {
    const searchEl = document.getElementById('fw-filter-search');
    if (searchEl) {
     e.preventDefault();
     searchEl.value = '';
     featureWebSaveFilters();
     featureWebRender();
    }
    return;
   }
   if (e.key === '0') {
    e.preventDefault();
    featureWebClearFilters();
    return;
   }
   if (e.key === 'q' || e.key === 'Q') {
    e.preventDefault();
    featureWebClearSelection();
    return;
   }
   if (e.key === 'j' || e.key === 'J') {
    e.preventDefault();
    featureWebCopyShare();
    return;
   }
   if (e.key === 'p' || e.key === 'P') {
    e.preventDefault();
    featureWebPasteJson();
    return;
   }
   if (fwSelectedNodeId && (e.key === 'n' || e.key === 'N')) {
    e.preventDefault();
    featureWebCopySelectedJson();
    return;
   }
   if (e.key === 'b' || e.key === 'B') {
    e.preventDefault();
    featureWebToggleAdvanced();
    return;
   }
   if (e.key === 'm' || e.key === 'M') {
    e.preventDefault();
    featureWebSyncNodesToBoard();
    return;
   }
   if (e.key === 'z' || e.key === 'Z') {
    e.preventDefault();
    featureWebReset();
    return;
   }
   if (e.key === '?') {
    e.preventDefault();
    featureWebToggleShortcutHint();
    return;
   }
   if (e.key === 'w' || e.key === 'W') {
    const linkedOnlyEl = document.getElementById('fw-linked-only');
    const ownedOnlyEl = document.getElementById('fw-owned-only');
    if (linkedOnlyEl && ownedOnlyEl) {
     e.preventDefault();
     const next = !(linkedOnlyEl.checked && ownedOnlyEl.checked);
     linkedOnlyEl.checked = next;
     ownedOnlyEl.checked = next;
     featureWebSaveFilters();
     featureWebRender();
    }
    return;
   }
   if (e.key === 'i' || e.key === 'I') {
    e.preventDefault();
    featureWebImport();
    return;
   }
   if (fwSelectedNodeId && (e.key === 'g' || e.key === 'G')) {
    e.preventDefault();
    featureWebNudgeToGrid(12);
    return;
   }
   if (fwSelectedNodeId && e.key === '[') {
    e.preventDefault();
    featureWebCycleSelectedStatus(-1);
    return;
   }
   if (fwSelectedNodeId && e.key === ']') {
    e.preventDefault();
    featureWebCycleSelectedStatus(1);
    return;
   }
   if (fwSelectedNodeId && e.altKey && (e.key === '1' || e.key === '2' || e.key === '3' || e.key === '4')) {
    e.preventDefault();
    const map = FW_STATUS_HOTKEY_MAP;
    const node = featureWeb.nodes.find(n => n.id === fwSelectedNodeId);
    const nextStatus = map[e.key];
    if (node && FW_ALLOWED_STATUSES.includes(nextStatus)) {
      node.status = nextStatus;
      featureWebSyncLinkedTask(node);
      featureWebPersistAndRefreshEditor();
    }
    return;
   }
   if (fwSelectedNodeId && e.altKey && (e.key === '5' || e.key === '6' || e.key === '7' || e.key === '8' || e.key === '9' || e.key === '0')) {
    e.preventDefault();
    const map = FW_TYPE_HOTKEY_MAP;
    const node = featureWeb.nodes.find(n => n.id === fwSelectedNodeId);
    const nextType = map[e.key];
    if (node && FW_ALLOWED_TYPES.includes(nextType)) {
      node.type = nextType;
      featureWebPersistAndRefreshEditor();
    }
    return;
   }
   if (fwSelectedNodeId && e.key === ';') {
    e.preventDefault();
    featureWebCycleSelectedType(-1);
    return;
   }
   if (fwSelectedNodeId && e.key === "'") {
    e.preventDefault();
    featureWebCycleSelectedType(1);
    return;
   }
   if (fwSelectedNodeId && e.key === ',') {
    e.preventDefault();
    featureWebCycleEdgeTypeForSelection(-1);
    return;
   }
   if (fwSelectedNodeId && e.key === '.') {
    e.preventDefault();
    featureWebCycleEdgeTypeForSelection(1);
    return;
   }
  });

  const API_BASE = 'https://united-humanity.us';
  const BOARD_WS_RECONNECT_MS = 5000;

  function boardConnect() {
   // Always try REST API first (reliable, works everywhere)
   fetch(API_BASE + '/api/tasks').then(r => r.json()).then(data => {
    boardTasks = boardNormalizeTasks(data && data.tasks);
    const nodeChanged = featureWebSyncLinkedNodesFromBoardTasks(boardTasks);
    if (nodeChanged) featureWebPersistAndRefreshEditor();
    renderBoard();
   }).catch(e => console.warn('Board REST fetch failed:', e));

   // Also connect WebSocket for real-time updates
   try {
    const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
    const host = location.host || 'united-humanity.us';
    boardWs = new WebSocket(proto + '//' + host + '/ws');
    boardWs.onopen = () => {
     const storedKey = localStorage.getItem('humanity_key');
     const storedName = localStorage.getItem('humanity_name');
     if (storedKey) {
      boardMyKey = storedKey;
      boardSend({ type: 'identify', public_key: storedKey, display_name: storedName || null });
     } else {
      boardMyKey = 'viewer_' + Math.random().toString(36).slice(2, 10);
      boardSend({ type: 'identify', public_key: boardMyKey, display_name: null });
     }
    };
    boardWs.onmessage = (e) => {
     try {
      const msg = JSON.parse(e.data);
      handleBoardMessage(msg);
     } catch {}
    };
    boardWs.onclose = () => { setTimeout(boardConnect, BOARD_WS_RECONNECT_MS); };
    boardWs.onerror = () => {};
   } catch (e) { console.warn('Board WS failed:', e); }
  }

  function handleBoardMessage(msg) {
   switch (msg.type) {
    case 'peer_list':
     // Extract our role from peer list
     if (msg.peers && boardMyKey) {
      const me = msg.peers.find(p => p.public_key === boardMyKey);
      if (me) boardMyRole = me.role || '';
     }
     updateBoardPermissions();
     // Request task list
     boardSend({ type: 'task_list' });
     break;
    case 'task_list_response':
     boardTasks = boardNormalizeTasks(msg.tasks);
     const nodeChanged = featureWebSyncLinkedNodesFromBoardTasks(boardTasks);
     if (nodeChanged) {
      featureWebPersistAndRefreshEditor();
     }
     renderBoard();
     break;
    case 'task_created':
     if (msg.task) {
      const createdTaskId = featureWebNormalizeTaskId(msg.task.id);
      featureWebUpsertBoardTask(msg.task);

      const desc = String(msg.task.description || '');
      const m = desc.match(/\[fw-node:([^\]]+)\]/);
      const markerNodeId = m && m[1] ? featureWebSanitizeId(m[1], FW_TEXT_LIMITS.id) : '';
      if (createdTaskId != null && markerNodeId) {
       const node = featureWeb.nodes.find(n => n.id === markerNodeId);
       const conflict = featureWeb.nodes.find(n => n.id !== markerNodeId && n.taskId === createdTaskId);
       if (node && !conflict) {
        node.taskId = createdTaskId;
        featureWebPersistAndRefreshEditor();
        const mapped = featureWebMapNodeStatusToTask(node.status);
        const currentTaskStatus = boardNormalizeTaskStatus(msg.task.status);
        if (currentTaskStatus !== mapped) {
          boardSendOrNotice({ type: 'task_move', id: createdTaskId, status: mapped });
        }
       }
      }
      renderBoard();
     }
     break;
    case 'task_updated':
     if (msg.task) {
      const updatedTaskId = featureWebNormalizeTaskId(msg.task.id);
      featureWebUpsertBoardTask(msg.task);

      const nodeChanged = featureWebApplyTaskStatusToLinkedNodes(updatedTaskId, msg.task.status);
      if (nodeChanged) {
       featureWebPersistAndRefreshEditor();
      }
      renderBoard();
     }
     break;
    case 'task_moved':
     if (msg.id != null && msg.status) {
      const movedTaskId = featureWebNormalizeTaskId(msg.id);
      const nextStatus = boardNormalizeTaskStatus(msg.status);
      const t = movedTaskId != null
        ? featureWebFindTaskById(movedTaskId)
        : featureWebFindTaskByRawId(msg.id);
      if (t) t.status = nextStatus;

      const nodeChanged = featureWebApplyTaskStatusToLinkedNodes(movedTaskId, nextStatus);
      if (nodeChanged) {
       featureWebPersistAndRefreshEditor();
      }
      renderBoard();
     }
     break;
    case 'task_deleted':
     if (msg.id != null) {
      const deletedTaskId = featureWebNormalizeTaskId(msg.id);
      if (deletedTaskId != null) featureWebRemoveTaskById(deletedTaskId);
      else featureWebRemoveTaskByRawId(msg.id);
      if (deletedTaskId != null) {
       let unlinked = false;
       featureWeb.nodes.forEach(node => {
        if (node && node.taskId === deletedTaskId) {
         node.taskId = null;
         unlinked = true;
        }
       });
       if (unlinked) {
        featureWebPersistAndRefreshEditor();
       }
      }
      renderBoard();
     }
     break;
    case 'task_comment_added':
     if (msg.task_id != null) {
      const commentTaskId = featureWebNormalizeTaskId(msg.task_id);
      const t = commentTaskId != null
        ? featureWebFindTaskById(commentTaskId)
        : featureWebFindTaskByRawId(msg.task_id);
      if (t) t.comment_count = boardNormalizeCommentCount(t.comment_count) + 1;
      renderBoard();
      // If modal is open for this task, append comment
      const modalEl = document.getElementById('task-modal-content');
      const modalTaskId = featureWebNormalizeTaskId(modalEl && modalEl.dataset ? modalEl.dataset.taskId : null);
      if (modalEl && modalTaskId != null && modalTaskId === commentTaskId && msg.comment) {
       const commentsDiv = document.getElementById('task-comments-list');
       if (commentsDiv) {
        if ((commentsDiv.textContent || '').trim() === 'No comments yet') commentsDiv.innerHTML = '';
        commentsDiv.innerHTML += renderCommentHtml(boardNormalizeComment(msg.comment));
       }
      }
     }
     break;
    case 'task_comments_response':
     if (msg.task_id != null) {
      const commentsDiv = document.getElementById('task-comments-list');
      const commentsTaskId = featureWebNormalizeTaskId(msg.task_id);
      const normalizedComments = boardNormalizeComments(msg.comments);
      if (commentsTaskId != null) {
       const task = featureWebFindTaskById(commentsTaskId);
       if (task && task.comment_count !== normalizedComments.length) {
        task.comment_count = normalizedComments.length;
        renderBoard();
       }
      }
      const modalTaskId = featureWebNormalizeTaskId(commentsDiv && commentsDiv.dataset ? commentsDiv.dataset.taskId : null);
      if (commentsDiv && commentsTaskId != null && commentsTaskId === modalTaskId) {
       commentsDiv.innerHTML = normalizedComments.map(renderCommentHtml).join('') || '<div style="color:var(--text-muted);font-size:0.8rem;font-style:italic;">No comments yet</div>';
      }
     }
     break;
   }
  }

  function updateBoardPermissions() {
   const canEdit = boardCanEdit();
   const btn = document.getElementById('board-create-btn');
   if (btn) btn.style.display = canEdit ? 'inline-flex' : 'none';
  }

  function renderBoard() {
   const loadingEl = document.getElementById('board-loading');
   if (loadingEl) loadingEl.style.display = 'none';
   VALID_STATUSES.forEach(status => {
    const container = document.querySelector('.board-col-cards[data-status="' + status + '"]');
    const countEl = document.querySelector('.board-col-count[data-status="' + status + '"]');
    if (!container) return;
    const tasks = boardTasks.filter(t => boardNormalizeTaskStatus(t && t.status) === status).sort(boardTaskSortCompare);
    const taskCards = tasks.map(t => renderTaskCard(t)).filter(Boolean);
    if (countEl) countEl.textContent = '(' + taskCards.length + ')';
    container.innerHTML = taskCards.join('');
   });
  }

  function renderTaskCard(task) {
   const taskId = featureWebNormalizeTaskId(task && task.id);
   if (taskId == null) return '';
   const taskTitle = boardNormalizeTitle(task && task.title) || 'Untitled Task';
   const taskPriority = boardNormalizePriority(task && task.priority);
   const taskStatus = boardNormalizeTaskStatus(task && task.status);
   const taskAssignee = boardNormalizeAssignee(task && task.assignee);
   const pc = PRIORITY_COLORS[taskPriority] || '#666';
   const labels = boardParseTaskLabels(task);
   const labelHtml = labels.map(l => '<span style="display:inline-block;background:rgba(255,136,17,0.15);color:var(--accent);font-size:0.6rem;padding:0.1rem 0.4rem;border-radius:3px;margin-right:0.2rem;">' + escHtml(l) + '</span>').join('');
   const assigneeHtml = taskAssignee ? '<span style="font-size:0.7rem;color:var(--text-muted);">👤 ' + escHtml(taskAssignee) + '</span>' : '';
   const commentCount = boardNormalizeCommentCount(task.comment_count);
   const commentHtml = commentCount > 0 ? '<span style="font-size:0.7rem;color:var(--text-muted);">💬 ' + commentCount + '</span>' : '';
   const canEdit = boardCanEdit();
   let moveHtml = '';
   if (canEdit) {
    const moves = VALID_STATUSES.filter(s => s !== taskStatus);
    moveHtml = '<div style="display:flex;gap:0.2rem;margin-top:0.3rem;flex-wrap:wrap;" onclick="event.stopPropagation()">' +
     moves.map(s => {
      const statusLabel = boardTaskStatusLabel(s);
      return '<button onclick="boardMoveTask(' + taskId + ',\'' + s + '\')" style="background:none;border:1px solid var(--border);color:var(--text-muted);font-size:0.6rem;padding:0.1rem 0.3rem;border-radius:3px;cursor:pointer;" title="Move to ' + statusLabel + '">→ ' + statusLabel.split(' ').pop() + '</button>';
     }).join('') +
     '</div>';
   }
   const addToWebHtml = '<div style="margin-top:0.35rem;" onclick="event.stopPropagation()"><button onclick="featureWebAddNodeFromTask(' + taskId + ')" style="background:none;border:1px solid rgba(255,136,17,0.4);color:var(--accent);font-size:0.62rem;padding:0.1rem 0.35rem;border-radius:4px;cursor:pointer;">+ Web</button></div>';
   return '<div onclick="openTaskModal(' + taskId + ')" style="background:var(--bg-panel,#141414);border:1px solid var(--border);border-left:3px solid ' + pc + ';border-radius:6px;padding:0.5rem 0.6rem;margin-bottom:0.4rem;cursor:pointer;transition:border-color 0.15s;" onmouseover="this.style.borderColor=\'rgba(255,136,17,0.3)\'" onmouseout="this.style.borderColor=\'var(--border)\';this.style.borderLeftColor=\'' + pc + '\'">' +
    '<div style="font-size:0.82rem;font-weight:600;color:var(--text);margin-bottom:0.3rem;">' + escHtml(taskTitle) + '</div>' +
    '<div style="display:flex;gap:0.4rem;align-items:center;flex-wrap:wrap;">' +
     '<span style="display:inline-block;background:' + pc + ';color:#fff;font-size:0.6rem;padding:0.05rem 0.35rem;border-radius:3px;font-weight:700;text-transform:uppercase;">' + taskPriority + '</span>' +
     labelHtml + assigneeHtml + commentHtml +
    '</div>' +
    moveHtml +
    addToWebHtml +
    '<div style="text-align:right;margin-top:0.3rem;font-size:0.6rem;color:var(--text-muted);opacity:0.5;">#' + taskId + '</div>' +
   '</div>';
  }

  function boardMoveTask(id, status) {
   if (!boardCanEdit()) return;
   const normalizedId = featureWebNormalizeTaskId(id);
   if (normalizedId == null) return;
   const normalizedStatus = boardNormalizeTaskStatus(status, null);
   if (!normalizedStatus) return;
   boardSendOrNotice({ type: 'task_move', id: normalizedId, status: normalizedStatus });
  }

  function openTaskModal(taskId) {
   const normalizedTaskId = featureWebNormalizeTaskId(taskId);
   if (normalizedTaskId == null) return;
   const task = featureWebFindTaskById(normalizedTaskId);
   if (!task) return;
   const { modal, content } = boardGetTaskModalElements();
   if (!modal || !content) return;
   content.dataset.taskId = String(normalizedTaskId);
   const canEdit = boardCanEdit();
   const canComment = boardCanComment();
   const taskTitle = boardNormalizeTitle(task.title) || 'Untitled Task';
   const taskDescription = boardNormalizeDescription(task.description);
   const taskPriority = boardNormalizePriority(task.priority);
   const taskStatus = boardNormalizeTaskStatus(task.status);
   const taskAssignee = boardNormalizeAssignee(task.assignee);
   const pc = PRIORITY_COLORS[taskPriority] || '#666';
   const labels = boardParseTaskLabels(task);
   const created = boardFormatDateTime(task.created_at);

   let html = '<h3 style="color:var(--text);margin:0 0 0.5rem;font-size:1.1rem;">' + escHtml(taskTitle) + '</h3>' +
    '<div style="display:flex;gap:0.5rem;align-items:center;flex-wrap:wrap;margin-bottom:0.8rem;">' +
     '<span style="background:' + pc + ';color:#fff;font-size:0.7rem;padding:0.1rem 0.4rem;border-radius:3px;font-weight:700;text-transform:uppercase;">' + taskPriority + '</span>' +
     '<span style="font-size:0.75rem;color:var(--text-muted);">' + boardTaskStatusLabel(taskStatus) + '</span>' +
     (taskAssignee ? '<span style="font-size:0.75rem;color:var(--text-muted);">👤 ' + escHtml(taskAssignee) + '</span>' : '') +
     '<span style="font-size:0.7rem;color:var(--text-muted);">Created: ' + created + '</span>' +
    '</div>';

   if (labels.length) {
    html += '<div style="margin-bottom:0.5rem;">' + labels.map(l => '<span style="display:inline-block;background:rgba(255,136,17,0.15);color:var(--accent);font-size:0.7rem;padding:0.1rem 0.5rem;border-radius:3px;margin-right:0.3rem;">' + escHtml(l) + '</span>').join('') + '</div>';
   }

   if (taskDescription) {
    html += '<div style="background:var(--bg-input,#111);border-radius:6px;padding:0.6rem;margin-bottom:1rem;font-size:0.82rem;color:var(--text);line-height:1.6;white-space:pre-wrap;">' + escHtml(taskDescription) + '</div>';
   }

   if (canEdit) {
    html += '<div style="display:flex;gap:0.4rem;margin-bottom:1rem;flex-wrap:wrap;">' +
     '<button class="btn btn-clickable" style="min-width:auto;min-height:32px;padding:0.2rem 0.6rem;font-size:0.75rem;" onclick="showEditTaskForm(' + normalizedTaskId + ')">✏️ Edit</button>' +
     '<button class="btn" style="min-width:auto;min-height:32px;padding:0.2rem 0.6rem;font-size:0.75rem;box-shadow:inset 0 0 0 1px #e55;" onclick="boardDeleteTask(' + normalizedTaskId + ')">🗑️ Delete</button>' +
    '</div>';
   }

   html += '<h4 style="color:var(--text-muted);font-size:0.85rem;margin:1rem 0 0.5rem;border-top:1px solid var(--border);padding-top:0.8rem;">💬 Comments</h4>' +
    '<div id="task-comments-list" data-task-id="' + normalizedTaskId + '" style="margin-bottom:0.8rem;"><div style="color:var(--text-muted);font-size:0.8rem;">Loading…</div></div>';

   if (canComment) {
    html += '<div style="display:flex;gap:0.4rem;">' +
     '<input type="text" id="task-comment-input" placeholder="Add a comment…" style="flex:1;background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.4rem 0.6rem;border-radius:6px;font-size:0.82rem;outline:none;font-family:inherit;" onkeydown="if(event.key===\'Enter\')boardAddComment(' + normalizedTaskId + ')">' +
     '<button onclick="boardAddComment(' + normalizedTaskId + ')" style="background:var(--accent);color:#fff;border:none;padding:0.4rem 0.8rem;border-radius:6px;font-size:0.82rem;cursor:pointer;font-weight:600;">Post</button>' +
    '</div>';
   }

   content.innerHTML = html;
   modal.style.display = 'flex';

   // Request comments
   boardSend({ type: 'task_comments_request', task_id: normalizedTaskId });
  }

  function renderCommentHtml(c) {
   const time = boardFormatDateTime(c && c.created_at);
   return '<div style="background:var(--bg-input,#111);border-radius:6px;padding:0.4rem 0.6rem;margin-bottom:0.3rem;">' +
    '<div style="display:flex;justify-content:space-between;margin-bottom:0.2rem;"><span style="font-size:0.75rem;font-weight:600;color:var(--accent);">' + escHtml(c.author_name) + '</span><span style="font-size:0.65rem;color:var(--text-muted);">' + time + '</span></div>' +
    '<div style="font-size:0.8rem;color:var(--text);white-space:pre-wrap;">' + escHtml(c.content) + '</div>' +
   '</div>';
  }

  function boardGetTaskModalElements() {
   return {
    modal: document.getElementById('task-modal'),
    content: document.getElementById('task-modal-content'),
   };
  }

  function closeTaskModal() {
   const { modal } = boardGetTaskModalElements();
   if (modal) modal.style.display = 'none';
  }

  function boardAddComment(taskId) {
   if (!boardCanComment()) return;
   const normalizedTaskId = featureWebNormalizeTaskId(taskId);
   if (normalizedTaskId == null) return;
   const input = document.getElementById('task-comment-input');
   if (!input) return;
   const content = boardNormalizeCommentContent(input.value);
   if (!content) return;
   if (boardSendOrNotice({ type: 'task_comment', task_id: normalizedTaskId, content })) {
    input.value = '';
   }
  }

  function boardDeleteTask(id) {
   if (!boardCanEdit()) return;
   const normalizedId = featureWebNormalizeTaskId(id);
   if (normalizedId == null) return;
   if (!confirm('Delete this task?')) return;
   if (boardSendOrNotice({ type: 'task_delete', id: normalizedId })) closeTaskModal();
  }

  function showCreateTaskModal() {
   if (!boardCanEdit()) return;
   const { modal, content } = boardGetTaskModalElements();
   if (!modal || !content) return;
   content.dataset.taskId = '';
   content.innerHTML =
    '<h3 style="color:var(--text);margin:0 0 1rem;font-size:1.1rem;">Create New Task</h3>' +
    '<label style="font-size:0.75rem;color:var(--text-muted);display:block;margin-bottom:0.2rem;">Title</label>' +
    '<input type="text" id="new-task-title" maxlength="200" style="width:100%;background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.4rem 0.6rem;border-radius:6px;font-size:0.85rem;outline:none;font-family:inherit;margin-bottom:0.6rem;">' +
    '<label style="font-size:0.75rem;color:var(--text-muted);display:block;margin-bottom:0.2rem;">Description</label>' +
    '<textarea id="new-task-desc" rows="4" style="width:100%;background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.4rem 0.6rem;border-radius:6px;font-size:0.85rem;outline:none;font-family:inherit;resize:vertical;margin-bottom:0.6rem;"></textarea>' +
    '<div style="display:grid;grid-template-columns:1fr 1fr;gap:0.6rem;margin-bottom:0.6rem;">' +
     '<div><label style="font-size:0.75rem;color:var(--text-muted);display:block;margin-bottom:0.2rem;">Priority</label>' +
      '<select id="new-task-priority" style="width:100%;background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.4rem;border-radius:6px;font-size:0.82rem;font-family:inherit;"><option value="low">Low</option><option value="medium" selected>Medium</option><option value="high">High</option><option value="critical">Critical</option></select></div>' +
     '<div><label style="font-size:0.75rem;color:var(--text-muted);display:block;margin-bottom:0.2rem;">Status</label>' +
      '<select id="new-task-status" style="width:100%;background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.4rem;border-radius:6px;font-size:0.82rem;font-family:inherit;"><option value="backlog" selected>Backlog</option><option value="in_progress">In Progress</option><option value="testing">Testing</option><option value="done">Done</option></select></div>' +
    '</div>' +
    '<label style="font-size:0.75rem;color:var(--text-muted);display:block;margin-bottom:0.2rem;">Assignee (optional)</label>' +
    '<input type="text" id="new-task-assignee" maxlength="50" style="width:100%;background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.4rem 0.6rem;border-radius:6px;font-size:0.85rem;outline:none;font-family:inherit;margin-bottom:0.6rem;">' +
    '<label style="font-size:0.75rem;color:var(--text-muted);display:block;margin-bottom:0.2rem;">Labels (comma-separated)</label>' +
    '<input type="text" id="new-task-labels" placeholder="bug, feature, urgent" style="width:100%;background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.4rem 0.6rem;border-radius:6px;font-size:0.85rem;outline:none;font-family:inherit;margin-bottom:1rem;">' +
    '<button onclick="boardSubmitCreateTask()" style="background:var(--accent);color:#fff;border:none;padding:0.5rem 1.5rem;border-radius:6px;font-size:0.85rem;cursor:pointer;font-weight:600;width:100%;">Create Task</button>';
   modal.style.display = 'flex';
  }

  function boardSubmitCreateTask() {
   if (!boardCanEdit()) return;
   const title = boardNormalizeTitle(document.getElementById('new-task-title').value);
   if (!title) return;
   const labels = boardLabelsToJson(document.getElementById('new-task-labels').value);
   const assignee = boardNormalizeAssignee(document.getElementById('new-task-assignee').value);
   const nextPriority = boardNormalizePriority(document.getElementById('new-task-priority').value);
   const nextStatus = boardNormalizeTaskStatus(document.getElementById('new-task-status').value);
   const sent = boardSendOrNotice({
    type: 'task_create',
    title: title,
    description: boardNormalizeDescription(document.getElementById('new-task-desc').value),
    priority: nextPriority,
    status: nextStatus,
    assignee: assignee,
    labels: labels,
   });
   if (sent) closeTaskModal();
  }

  function showEditTaskForm(taskId) {
   if (!boardCanEdit()) return;
   const normalizedTaskId = featureWebNormalizeTaskId(taskId);
   if (normalizedTaskId == null) return;
   const task = featureWebFindTaskById(normalizedTaskId);
   if (!task) return;
   const { content } = boardGetTaskModalElements();
   if (!content) return;
   const labels = boardParseTaskLabels(task);
   const taskTitle = boardNormalizeTitle(task.title) || 'Untitled Task';
   const taskDescription = boardNormalizeDescription(task.description);
   const taskPriority = boardNormalizePriority(task.priority);
   content.innerHTML =
    '<h3 style="color:var(--text);margin:0 0 1rem;font-size:1.1rem;">Edit Task</h3>' +
    '<label style="font-size:0.75rem;color:var(--text-muted);display:block;margin-bottom:0.2rem;">Title</label>' +
    '<input type="text" id="edit-task-title" maxlength="200" value="' + escHtml(taskTitle).replace(/"/g,'&quot;') + '" style="width:100%;background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.4rem 0.6rem;border-radius:6px;font-size:0.85rem;outline:none;font-family:inherit;margin-bottom:0.6rem;">' +
    '<label style="font-size:0.75rem;color:var(--text-muted);display:block;margin-bottom:0.2rem;">Description</label>' +
    '<textarea id="edit-task-desc" rows="4" style="width:100%;background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.4rem 0.6rem;border-radius:6px;font-size:0.85rem;outline:none;font-family:inherit;resize:vertical;margin-bottom:0.6rem;">' + escHtml(taskDescription) + '</textarea>' +
    '<div style="display:grid;grid-template-columns:1fr 1fr;gap:0.6rem;margin-bottom:0.6rem;">' +
     '<div><label style="font-size:0.75rem;color:var(--text-muted);display:block;margin-bottom:0.2rem;">Priority</label>' +
      '<select id="edit-task-priority" style="width:100%;background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.4rem;border-radius:6px;font-size:0.82rem;font-family:inherit;"><option value="low"' + (taskPriority==='low'?' selected':'') + '>Low</option><option value="medium"' + (taskPriority==='medium'?' selected':'') + '>Medium</option><option value="high"' + (taskPriority==='high'?' selected':'') + '>High</option><option value="critical"' + (taskPriority==='critical'?' selected':'') + '>Critical</option></select></div>' +
     '<div><label style="font-size:0.75rem;color:var(--text-muted);display:block;margin-bottom:0.2rem;">Assignee</label>' +
      '<input type="text" id="edit-task-assignee" maxlength="50" value="' + escHtml(boardNormalizeAssignee(task.assignee) || '').replace(/"/g,'&quot;') + '" style="width:100%;background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.4rem 0.6rem;border-radius:6px;font-size:0.82rem;outline:none;font-family:inherit;"></div>' +
    '</div>' +
    '<label style="font-size:0.75rem;color:var(--text-muted);display:block;margin-bottom:0.2rem;">Labels (comma-separated)</label>' +
    '<input type="text" id="edit-task-labels" value="' + labels.join(', ').replace(/"/g,'&quot;') + '" style="width:100%;background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.4rem 0.6rem;border-radius:6px;font-size:0.85rem;outline:none;font-family:inherit;margin-bottom:1rem;">' +
    '<button onclick="boardSubmitEditTask(' + normalizedTaskId + ')" style="background:var(--accent);color:#fff;border:none;padding:0.5rem 1.5rem;border-radius:6px;font-size:0.85rem;cursor:pointer;font-weight:600;width:100%;">Save Changes</button>';
  }

  function boardSubmitEditTask(id) {
   if (!boardCanEdit()) return;
   const normalizedId = featureWebNormalizeTaskId(id);
   if (normalizedId == null) return;
   const title = boardNormalizeTitle(document.getElementById('edit-task-title').value);
   if (!title) return;
   const labels = boardLabelsToJson(document.getElementById('edit-task-labels').value);
   const assignee = boardNormalizeAssignee(document.getElementById('edit-task-assignee').value);
   const nextPriority = boardNormalizePriority(document.getElementById('edit-task-priority').value);
   const sent = boardSendOrNotice({
    type: 'task_update',
    id: normalizedId,
    title: title,
    description: boardNormalizeDescription(document.getElementById('edit-task-desc').value),
    priority: nextPriority,
    assignee: assignee,
    labels: labels,
   });
   if (sent) closeTaskModal();
  }

  // Close modal on backdrop click
  const { modal: taskModalEl } = boardGetTaskModalElements();
  if (taskModalEl) {
   taskModalEl.addEventListener('click', function(e) {
    if (e.target === this) closeTaskModal();
   });
  }

  // Connect board WebSocket when tab switches to board
  const origSwitchTab2 = switchTab;
  switchTab = function(tabId, pushState) {
   origSwitchTab2(tabId, pushState);
   if (tabId === 'board' && boardNeedsConnect()) {
    boardConnect();
   } else if (tabId === 'board') {
    boardSend({ type: 'task_list' });
   }
  };

  featureWebLoad();

  // Auto-connect if initial tab is board
  if (initialTab === 'board') boardConnect();

  // Mobile responsive: stack columns on small screens
  (function() {
   const style = document.createElement('style');
   style.textContent = '@media (max-width: 768px) { #board-columns { grid-template-columns: 1fr !important; } }';
   document.head.appendChild(style);
  })();

  // ── Browse Tab ──
  const TRANCO_RANKS = {
   'google.com': 1, 'youtube.com': 2, 'facebook.com': 3, 'amazon.com': 8,
   'wikipedia.org': 10, 'reddit.com': 15, 'netflix.com': 20, 'github.com': 25,
   'discord.com': 30, 'twitch.tv': 35, 'ebay.com': 40, 'aliexpress.com': 45,
   'stackoverflow.com': 50, 'fonts.google.com': 50, 'spotify.com': 55,
   'imdb.com': 60, 'duckduckgo.com': 70, 'bbc.com': 80,
   'store.steampowered.com': 100, 'etsy.com': 120, 'deepl.com': 150,
   'gitlab.com': 150, 'craigslist.org': 200, 'coursera.org': 200,
   'soundcloud.com': 200, 'reuters.com': 250, 'archive.org': 250,
   'apnews.com': 300, 'figma.com': 300, 'khanacademy.org': 350,
   'news.ycombinator.com': 400, 'proton.me': 400, 'unsplash.com': 500,
   'arstechnica.com': 500, 'libgen.is': 500, 'dev.to': 600,
   'codepen.io': 700, 'newegg.com': 800, 'wolframalpha.com': 900,
   'itch.io': 1000, 'nexusmods.com': 1500, 'blender.org': 2000,
   'regex101.com': 2000, 'coolors.co': 3000, 'bitwarden.com': 3000,
   'thingiverse.com': 4000, 'mastodon.social': 5000, 'excalidraw.com': 5000,
   'howlongtobeat.com': 5000, 'signal.org': 6000, 'lemmy.world': 8000,
   'pcgamingwiki.com': 10000, 'isthereanydeal.com': 15000, 'allsides.com': 20000,
  };

  function getDomainFromUrl(url) {
   try { return new URL(url.startsWith('http') ? url : 'https://'+url).hostname.replace(/^www\./,''); } catch { return ''; }
  }
  function getTrancoRank(url) {
   const domain = getDomainFromUrl(url);
   return TRANCO_RANKS[domain] || TRANCO_RANKS[domain.replace(/^[^.]+\./, '')] || 999999;
  }

  // Uptime ping system
  const _pingCache = {}; // { domain: { status: 'up'|'down'|'checking', ts: Date.now() } }
  const PING_TTL = 5 * 60 * 1000;
  function getPingStatus(url) {
   const domain = getDomainFromUrl(url);
   const cached = _pingCache[domain];
   if (cached && Date.now() - cached.ts < PING_TTL) return cached.status;
   return 'unchecked';
  }
  function getPingDot(url) {
   const s = getPingStatus(url);
   if (s === 'up') return '🟢';
   if (s === 'down') return '🔴';
   return '⚪';
  }
  let _pingQueue = [];
  let _pinging = false;
  async function pingBatch() {
   if (_pinging) return;
   _pinging = true;
   while (_pingQueue.length > 0) {
    const batch = _pingQueue.splice(0, 5);
    await Promise.all(batch.map(async url => {
     const domain = getDomainFromUrl(url);
     if (_pingCache[domain] && Date.now() - _pingCache[domain].ts < PING_TTL) return;
     _pingCache[domain] = { status: 'checking', ts: Date.now() };
     try {
      await fetch(url + '/favicon.ico', { mode: 'no-cors', signal: AbortSignal.timeout(5000) });
      _pingCache[domain] = { status: 'up', ts: Date.now() };
     } catch {
      _pingCache[domain] = { status: 'down', ts: Date.now() };
     }
    }));
    renderBrowseSiteCards();
    if (_pingQueue.length > 0) await new Promise(r => setTimeout(r, 300));
   }
   _pinging = false;
  }
  function enqueuePings(sites) {
   sites.forEach(s => {
    const domain = getDomainFromUrl(s.url);
    if (!_pingCache[domain] || Date.now() - _pingCache[domain].ts >= PING_TTL) {
     if (!_pingQueue.includes(s.url)) _pingQueue.push(s.url);
    }
   });
   pingBatch();
  }

  // RDAP system
  const RDAP_CACHE_KEY = 'humanity_rdap_cache';
  function loadRdapCache() { try { return JSON.parse(localStorage.getItem(RDAP_CACHE_KEY)) || {}; } catch { return {}; } }
  function saveRdapCache(cache) { localStorage.setItem(RDAP_CACHE_KEY, JSON.stringify(cache)); }
  const _rdapFetching = {};
  async function fetchRdap(url) {
   const domain = getDomainFromUrl(url);
   const cache = loadRdapCache();
   if (cache[domain] && Date.now() - cache[domain].ts < 24*60*60*1000) return cache[domain].data;
   if (_rdapFetching[domain]) return null;
   _rdapFetching[domain] = true;
   try {
    const resp = await fetch('https://rdap.org/domain/' + domain);
    if (!resp.ok) throw new Error('RDAP error');
    const json = await resp.json();
    let regYear = null, registrar = null;
    (json.events || []).forEach(e => { if (e.eventAction === 'registration' && e.eventDate) regYear = new Date(e.eventDate).getFullYear(); });
    (json.entities || []).forEach(e => { if ((e.roles || []).includes('registrar')) registrar = e.vcardArray?.[1]?.find(v=>v[0]==='fn')?.[3] || e.handle || null; });
    const data = { regYear, registrar };
    cache[domain] = { data, ts: Date.now() };
    saveRdapCache(cache);
    delete _rdapFetching[domain];
    return data;
   } catch { delete _rdapFetching[domain]; return null; }
  }
  function getRdapDisplay(url) {
   const domain = getDomainFromUrl(url);
   const cache = loadRdapCache();
   if (cache[domain] && Date.now() - cache[domain].ts < 24*60*60*1000) {
    const d = cache[domain].data;
    const parts = [];
    if (d.regYear) parts.push('📅 ' + d.regYear);
    if (d.registrar) parts.push('🏢 ' + d.registrar);
    return parts.length ? parts.join(' · ') + ' · ' : '';
   }
   return '';
  }

  let browseCurrentSort = 'popular';
  function setBrowseSort(sort) {
   browseCurrentSort = sort;
   ['popular','used','recent','az'].forEach(s => {
    const el = document.getElementById('sort-' + s);
    if (el) el.classList.toggle('active', s === sort);
   });
   renderBrowseSites();
  }

  const DEFAULT_SITES = [
   { url:'https://github.com', name:'GitHub', description:'Code hosting and collaboration', category:'Tech', icon:'🐙' },
   { url:'https://stackoverflow.com', name:'Stack Overflow', description:'Programming Q&A', category:'Tech', icon:'📚' },
   { url:'https://news.ycombinator.com', name:'Hacker News', description:'Tech news and discussion', category:'Tech', icon:'🟧' },
   { url:'https://dev.to', name:'DEV Community', description:'Developer blogs and articles', category:'Tech', icon:'👩‍💻' },
   { url:'https://gitlab.com', name:'GitLab', description:'DevOps and code hosting', category:'Tech', icon:'🦊' },
   { url:'https://codepen.io', name:'CodePen', description:'Frontend code playground', category:'Tech', icon:'✏️' },
   { url:'https://amazon.com', name:'Amazon', description:'Online marketplace', category:'Shopping', icon:'📦' },
   { url:'https://ebay.com', name:'eBay', description:'Auction and buy-it-now marketplace', category:'Shopping', icon:'🏷️' },
   { url:'https://etsy.com', name:'Etsy', description:'Handmade and vintage goods', category:'Shopping', icon:'🎨' },
   { url:'https://craigslist.org', name:'Craigslist', description:'Local classifieds', category:'Shopping', icon:'📋' },
   { url:'https://newegg.com', name:'Newegg', description:'Computer hardware and electronics', category:'Shopping', icon:'🖥️' },
   { url:'https://aliexpress.com', name:'AliExpress', description:'International marketplace', category:'Shopping', icon:'🌏' },
   { url:'https://reuters.com', name:'Reuters', description:'International news agency', category:'News', icon:'📰' },
   { url:'https://apnews.com', name:'AP News', description:'Associated Press', category:'News', icon:'🗞️' },
   { url:'https://arstechnica.com', name:'Ars Technica', description:'Technology news and analysis', category:'News', icon:'🔬' },
   { url:'https://bbc.com/news', name:'BBC News', description:'British Broadcasting Corporation', category:'News', icon:'LIVE' },
   { url:'https://allsides.com', name:'AllSides', description:'News from multiple perspectives', category:'News', icon:'⚖️' },
   { url:'https://youtube.com', name:'YouTube', description:'Video sharing platform', category:'Entertainment', icon:'▶️' },
   { url:'https://twitch.tv', name:'Twitch', description:'Live streaming platform', category:'Entertainment', icon:'💜' },
   { url:'https://netflix.com', name:'Netflix', description:'Streaming movies and shows', category:'Entertainment', icon:'PREVIEW' },
   { url:'https://spotify.com', name:'Spotify', description:'Music streaming', category:'Entertainment', icon:'🎵' },
   { url:'https://soundcloud.com', name:'SoundCloud', description:'Independent music platform', category:'Entertainment', icon:'🔊' },
   { url:'https://imdb.com', name:'IMDB', description:'Movie and TV database', category:'Entertainment', icon:'⭐' },
   { url:'https://store.steampowered.com', name:'Steam', description:'PC game store and community', category:'Gaming', icon:'🎮' },
   { url:'https://pcgamingwiki.com', name:'PCGamingWiki', description:'PC game fixes and info', category:'Gaming', icon:'🔧' },
   { url:'https://nexusmods.com', name:'Nexus Mods', description:'Game modding community', category:'Gaming', icon:'⚙️' },
   { url:'https://isthereanydeal.com', name:'IsThereAnyDeal', description:'Game price comparison', category:'Gaming', icon:'💰' },
   { url:'https://howlongtobeat.com', name:'HowLongToBeat', description:'Game completion times', category:'Gaming', icon:'Duration️' },
   { url:'https://itch.io', name:'itch.io', description:'Indie game marketplace', category:'Gaming', icon:'🕹️' },
   { url:'https://wikipedia.org', name:'Wikipedia', description:'Free encyclopedia', category:'Education', icon:'📖' },
   { url:'https://khanacademy.org', name:'Khan Academy', description:'Free courses and lessons', category:'Education', icon:'🎓' },
   { url:'https://wolframalpha.com', name:'Wolfram Alpha', description:'Computational knowledge engine', category:'Education', icon:'🧮' },
   { url:'https://coursera.org', name:'Coursera', description:'Online university courses', category:'Education', icon:'🏫' },
   { url:'https://archive.org', name:'Internet Archive', description:'Digital library and Wayback Machine', category:'Education', icon:'🏛️' },
   { url:'https://libgen.is', name:'Library Genesis', description:'Book and paper archive', category:'Education', icon:'📕' },
   { url:'https://figma.com', name:'Figma', description:'Collaborative design tool', category:'Creative', icon:'🎨' },
   { url:'https://blender.org', name:'Blender', description:'Open source 3D creation', category:'Creative', icon:'🧊' },
   { url:'https://unsplash.com', name:'Unsplash', description:'Free high-quality photos', category:'Creative', icon:'📷' },
   { url:'https://fonts.google.com', name:'Google Fonts', description:'Free web fonts', category:'Creative', icon:'🔤' },
   { url:'https://coolors.co', name:'Coolors', description:'Color palette generator', category:'Creative', icon:'🌈' },
   { url:'https://thingiverse.com', name:'Thingiverse', description:'3D printing models', category:'Creative', icon:'🖨️' },
   { url:'https://reddit.com', name:'Reddit', description:'Community forums and discussion', category:'Social', icon:'🤖' },
   { url:'https://discord.com', name:'Discord', description:'Chat and voice communities', category:'Social', icon:'💬' },
   { url:'https://mastodon.social', name:'Mastodon', description:'Decentralized social network', category:'Social', icon:'🐘' },
   { url:'https://lemmy.world', name:'Lemmy', description:'Federated link aggregator', category:'Social', icon:'🔗' },
   { url:'https://signal.org', name:'Signal', description:'Private messaging', category:'Social', icon:'🔒' },
   { url:'https://duckduckgo.com', name:'DuckDuckGo', description:'Private search engine', category:'Tools', icon:'🦆' },
   { url:'https://proton.me', name:'Proton Mail', description:'Encrypted email', category:'Tools', icon:'✉️' },
   { url:'https://bitwarden.com', name:'Bitwarden', description:'Password manager', category:'Tools', icon:'🔐' },
   { url:'https://excalidraw.com', name:'Excalidraw', description:'Virtual whiteboard', category:'Tools', icon:'✏️' },
   { url:'https://regex101.com', name:'Regex101', description:'Regular expression tester', category:'Tools', icon:'🔍' },
   { url:'https://deepl.com', name:'DeepL', description:'AI translation', category:'Tools', icon:'🌐' },
  ];

  const BROWSE_STORAGE_KEY = 'humanity_browse';
  function loadBrowseData() { try { return JSON.parse(localStorage.getItem(BROWSE_STORAGE_KEY)) || {}; } catch { return {}; } }
  function saveBrowseData(data) { localStorage.setItem(BROWSE_STORAGE_KEY, JSON.stringify(data)); if (typeof scheduleSyncSave === 'function') scheduleSyncSave(); }
  function getBrowseData() {
   const data = loadBrowseData();
   if (!data.quickBar) data.quickBar = ['https://duckduckgo.com','https://youtube.com','https://github.com','https://reddit.com','https://wikipedia.org'];
   if (!data.collections) data.collections = {};
   if (!data.hidden) data.hidden = [];
   if (!data.customSites) data.customSites = [];
   if (!data.clickCounts) data.clickCounts = {};
   if (!data.lastVisited) data.lastVisited = {};
   return data;
  }
  function getAllSites() { const data = getBrowseData(); return [...DEFAULT_SITES, ...data.customSites]; }

  let browseActiveCategory = '';
  let browseCurrentUrl = '';

  function renderQuickBar() {
   const data = getBrowseData();
   const container = document.getElementById('quickbar-sites');
   if (!container) return;
   const allSites = getAllSites();
   container.innerHTML = data.quickBar.map(url => {
    const site = allSites.find(s => s.url === url);
    const icon = site ? site.icon : '🌐';
    const name = site ? site.name : new URL(url).hostname;
    return `<button onclick="browseNavigate('${url}')" title="${name}" style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:6px;padding:0.25rem 0.5rem;cursor:pointer;font-size:0.85rem;white-space:nowrap;color:var(--text);display:flex;align-items:center;gap:0.25rem;" oncontextmenu="event.preventDefault();removeFromQuickBar('${url}')">${icon} <span style="font-size:0.72rem;">${name}</span></button>`;
   }).join('');
  }

  function renderBrowseCategories() {
   const categories = [...new Set(getAllSites().map(s => s.category))];
   const container = document.getElementById('browse-categories');
   if (!container) return;
   container.innerHTML = `<span class="filter-pill ${browseActiveCategory===''?'active':''}" onclick="browseActiveCategory='';renderBrowseSites()" style="font-size:0.72rem;padding:0.15rem 0.5rem;cursor:pointer;">All</span>` +
    categories.map(c => `<span class="filter-pill ${browseActiveCategory===c?'active':''}" onclick="browseActiveCategory='${c}';renderBrowseSites()" style="font-size:0.72rem;padding:0.15rem 0.5rem;cursor:pointer;">${c}</span>`).join('');
  }

  function renderCollections() {
   const data = getBrowseData();
   const container = document.getElementById('browse-collections');
   if (!container) return;
   const collNames = Object.keys(data.collections);
   if (collNames.length === 0) { container.innerHTML = '<div style="font-size:0.72rem;color:var(--text-muted);padding:0.2rem;">No collections yet. Right-click a site to add to a collection.</div>'; return; }
   container.innerHTML = collNames.map(name => {
    const count = data.collections[name].length;
    return `<div style="display:flex;justify-content:space-between;align-items:center;padding:0.2rem 0.3rem;cursor:pointer;border-radius:4px;font-size:0.8rem;" onclick="browseActiveCategory='collection:${name}';renderBrowseSites()" oncontextmenu="event.preventDefault();deleteCollection('${name}')"><span>📁 ${name} <span style="color:var(--text-muted);">(${count})</span></span><button onclick="event.stopPropagation();deleteCollection('${name}')" style="background:none;border:none;color:var(--text-muted);cursor:pointer;font-size:0.7rem;">Close</button></div>`;
   }).join('') + `<button onclick="createCollection()" style="font-size:0.72rem;color:var(--accent);background:none;border:none;cursor:pointer;padding:0.2rem;">+ New Collection</button>`;
  }

  function sortBrowseSites(sites, data) {
   const sorted = [...sites];
   switch (browseCurrentSort) {
    case 'popular': sorted.sort((a, b) => getTrancoRank(a.url) - getTrancoRank(b.url)); break;
    case 'used': sorted.sort((a, b) => (data.clickCounts[b.url]||0) - (data.clickCounts[a.url]||0)); break;
    case 'recent': sorted.sort((a, b) => (data.lastVisited[b.url]||0) - (data.lastVisited[a.url]||0)); break;
    case 'az': sorted.sort((a, b) => a.name.localeCompare(b.name)); break;
   }
   return sorted;
  }
  let _lastRenderedSites = [];
  function renderBrowseSites() {
   renderBrowseCategories(); renderCollections();
   const data = getBrowseData();
   const search = (document.getElementById('browse-search')?.value || '').toLowerCase();
   let sites = getAllSites().filter(s => !data.hidden.includes(s.url));
   if (browseActiveCategory.startsWith('collection:')) {
    const collName = browseActiveCategory.replace('collection:', '');
    const collUrls = data.collections[collName] || [];
    sites = sites.filter(s => collUrls.includes(s.url));
   } else if (browseActiveCategory) { sites = sites.filter(s => s.category === browseActiveCategory); }
   if (search) { sites = sites.filter(s => s.name.toLowerCase().includes(search) || s.description.toLowerCase().includes(search) || s.url.toLowerCase().includes(search)); }
   sites = sortBrowseSites(sites, data);
   _lastRenderedSites = sites;
   const container = document.getElementById('browse-sites');
   if (!container) return;
   renderBrowseSiteCards();
   enqueuePings(sites);
  }
  function renderBrowseSiteCards() {
   const sites = _lastRenderedSites;
   const data = getBrowseData();
   const container = document.getElementById('browse-sites');
   if (!container) return;
   container.innerHTML = sites.map(s => {
    const rank = getTrancoRank(s.url);
    const rankStr = rank < 999999 ? `#${rank} globally` : '';
    const dot = getPingDot(s.url);
    const rdap = getRdapDisplay(s.url);
    const domain = getDomainFromUrl(s.url);
    return `<div class="browse-site-card" style="padding:0.5rem;margin-bottom:0.3rem;background:var(--bg-secondary);border-radius:6px;cursor:pointer;border:1px solid var(--border);" onclick="browseNavigateTracked('${s.url.replace(/'/g,"\\'")}')" oncontextmenu="event.preventDefault();showSiteContextMenu(event,'${s.url.replace(/'/g,"\\'")}','${s.name.replace(/'/g,"\\'")}');" onmouseenter="lazyFetchRdap('${s.url.replace(/'/g,"\\'")}')">`+
     `<div style="display:flex;justify-content:space-between;align-items:center;"><span style="font-weight:600;font-size:0.85rem;">${s.icon} ${s.name} ${dot}</span><span style="font-size:0.65rem;color:var(--text-muted);background:var(--bg);padding:0.1rem 0.4rem;border-radius:8px;">${s.category}</span></div>`+
     (rankStr ? `<div style="font-size:0.62rem;color:var(--text-muted);margin-top:0.1rem;">${rankStr}</div>` : '')+
     `<div style="font-size:0.72rem;color:var(--text-muted);margin-top:0.2rem;">${s.description}</div>`+
     `<div style="font-size:0.62rem;color:var(--accent);margin-top:0.15rem;">${rdap}${domain}</div></div>`;
   }).join('') || '<div style="padding:1rem;color:var(--text-muted);text-align:center;">No sites found</div>';
  }

  function filterBrowseSites() { renderBrowseSites(); }
  function browseNavigateTracked(url) {
   if (!url) return;
   const status = getPingStatus(url.startsWith('http') ? url : 'https://'+url);
   if (status === 'down') {
    if (!confirm('⚠️ This site may be down. Continue anyway?')) return;
   }
   const data = getBrowseData();
   data.clickCounts[url] = (data.clickCounts[url] || 0) + 1;
   data.lastVisited[url] = Date.now();
   saveBrowseData(data);
   browseNavigate(url);
  }
  function lazyFetchRdap(url) {
   const domain = getDomainFromUrl(url);
   const cache = loadRdapCache();
   if (cache[domain] && Date.now() - cache[domain].ts < 24*60*60*1000) return;
   fetchRdap(url).then(() => renderBrowseSiteCards());
  }
  function browseNavigate(url) {
   if (!url) return;
   if (!url.startsWith('http')) url = 'https://' + url;
   browseCurrentUrl = url;
   document.getElementById('browse-url-bar').value = url;
   document.getElementById('browse-iframe').src = url;
   document.getElementById('browse-iframe').style.display = 'block';
   document.getElementById('browse-placeholder').style.display = 'none';
  }
  function browsePanelBack() {
   document.getElementById('browse-iframe').style.display = 'none';
   document.getElementById('browse-iframe').src = '';
   document.getElementById('browse-placeholder').style.display = 'flex';
   document.getElementById('browse-url-bar').value = '';
   browseCurrentUrl = '';
  }
  function browseOpenExternal() { if (browseCurrentUrl) window.open(browseCurrentUrl, '_blank'); }
  function addToQuickBar(url) { const data = getBrowseData(); if (!data.quickBar.includes(url)) { data.quickBar.push(url); saveBrowseData(data); renderQuickBar(); } }
  function removeFromQuickBar(url) { const data = getBrowseData(); data.quickBar = data.quickBar.filter(u => u !== url); saveBrowseData(data); renderQuickBar(); }
  function hideSite(url) { const data = getBrowseData(); if (!data.hidden.includes(url)) { data.hidden.push(url); saveBrowseData(data); renderBrowseSites(); } }
  function createCollection() { const name = prompt('Collection name:'); if (!name || !name.trim()) return; const data = getBrowseData(); if (!data.collections[name.trim()]) { data.collections[name.trim()] = []; saveBrowseData(data); renderCollections(); } }
  function deleteCollection(name) { if (!confirm('Delete collection "' + name + '"?')) return; const data = getBrowseData(); delete data.collections[name]; saveBrowseData(data); if (browseActiveCategory === 'collection:' + name) browseActiveCategory = ''; renderBrowseSites(); }
  function addToCollection(url, collectionName) { const data = getBrowseData(); if (!data.collections[collectionName]) data.collections[collectionName] = []; if (!data.collections[collectionName].includes(url)) { data.collections[collectionName].push(url); saveBrowseData(data); renderBrowseSites(); } }
  function showAddSiteModal() {
   const url = prompt('Site URL:'); if (!url) return;
   const name = prompt('Site name:') || new URL(url.startsWith('http') ? url : 'https://'+url).hostname;
   const desc = prompt('Description (optional):') || '';
   const category = prompt('Category (Tech, Shopping, News, Entertainment, Gaming, Education, Creative, Social, Tools):') || 'Tools';
   const data = getBrowseData();
   data.customSites.push({ url: url.startsWith('http') ? url : 'https://'+url, name, description: desc, category, icon: '🌐' });
   saveBrowseData(data); renderBrowseSites();
  }

  let siteContextMenu = null;
  function showSiteContextMenu(event, url, name) {
   if (siteContextMenu) siteContextMenu.remove();
   const data = getBrowseData();
   const inQuickBar = data.quickBar.includes(url);
   const collNames = Object.keys(data.collections);
   const menu = document.createElement('div');
   menu.style.cssText = 'position:fixed;z-index:9999;background:var(--bg-secondary);border:1px solid var(--border);border-radius:6px;padding:0.3rem 0;box-shadow:0 4px 12px rgba(0,0,0,0.3);min-width:160px;';
   menu.style.left = event.clientX + 'px'; menu.style.top = event.clientY + 'px';
   const items = [
    { label: inQuickBar ? 'âš¡ Remove from Quick Bar' : 'âš¡ Add to Quick Bar', action: () => inQuickBar ? removeFromQuickBar(url) : addToQuickBar(url) },
    { label: '↗ Open in New Tab', action: () => window.open(url, '_blank') },
    { label: '🚫 Hide Site', action: () => hideSite(url) },
   ];
   if (collNames.length > 0) { items.push({ label: '─────', action: null }); collNames.forEach(c => { items.push({ label: '📁 Add to ' + c, action: () => addToCollection(url, c) }); }); }
   menu.innerHTML = items.map((item, i) => {
    if (!item.action) return '<div style="border-top:1px solid var(--border);margin:0.2rem 0;"></div>';
    return `<div class="ctx-item" data-idx="${i}" style="padding:0.3rem 0.8rem;cursor:pointer;font-size:0.8rem;color:var(--text);" onmouseenter="this.style.background='var(--accent)'" onmouseleave="this.style.background=''">${item.label}</div>`;
   }).join('');
   menu.querySelectorAll('.ctx-item').forEach(el => { el.addEventListener('click', () => { items[parseInt(el.dataset.idx)].action(); menu.remove(); siteContextMenu = null; }); });
   document.body.appendChild(menu); siteContextMenu = menu;
   setTimeout(() => document.addEventListener('click', function dismiss() { if(siteContextMenu) siteContextMenu.remove(); siteContextMenu=null; document.removeEventListener('click',dismiss); }, { once: false }), 10);
  }

  function initBrowseTab() { renderQuickBar(); renderBrowseSites(); }

  // ── Dashboard Tab ──
  const DASHBOARD_STORAGE_KEY = 'humanity_dashboard';
  const WIDGET_TYPES = {
   clock: { name: 'Clock', icon: '🕐', description: 'Current time and date', defaultSize: 'small' },
   notes: { name: 'Notes', icon: '📝', description: 'Quick notes pad', defaultSize: 'medium' },
   todos: { name: 'Todos', icon: '✅', description: 'Your todo list', defaultSize: 'medium' },
   friends: { name: 'Friends Online', icon: '👥', description: 'Who is online now', defaultSize: 'small' },
   stats: { name: 'Server Stats', icon: '📈', description: 'Relay server statistics', defaultSize: 'small' },
   quicklinks: { name: 'Quick Links', icon: 'âš¡', description: 'Your favorite bookmarks', defaultSize: 'small' },
   embed: { name: 'Website Embed', icon: '🌐', description: 'Embed any website', defaultSize: 'large' },
   chat: { name: 'Chat Feed', icon: '💬', description: 'Live messages from a channel', defaultSize: 'large' },
   weather: { name: 'Weather', icon: '🌤️', description: 'Current weather', defaultSize: 'small' },
   activity: { name: 'Recent Activity', icon: '🔔', description: 'Latest messages across channels', defaultSize: 'medium' },
  };

  function loadDashboard() { try { return JSON.parse(localStorage.getItem(DASHBOARD_STORAGE_KEY)) || { widgets: [] }; } catch { return { widgets: [] }; } }
  function saveDashboard(data) { localStorage.setItem(DASHBOARD_STORAGE_KEY, JSON.stringify(data)); if (typeof scheduleSyncSave === 'function') scheduleSyncSave(); }

  function renderDashboard() {
   const data = loadDashboard();
   const grid = document.getElementById('dashboard-grid');
   const empty = document.getElementById('dashboard-empty');
   if (!grid) return;
   if (data.widgets.length === 0) { grid.style.display = 'none'; if (empty) empty.style.display = 'block'; return; }
   grid.style.display = 'grid'; if (empty) empty.style.display = 'none';
   grid.innerHTML = data.widgets.map((w, i) => {
    const sizeStyle = w.size === 'large' ? 'grid-column:span 2;min-height:300px;' : w.size === 'medium' ? 'min-height:200px;' : 'min-height:140px;';
    return `<div class="dashboard-widget" style="background:var(--bg-secondary);border:1px solid var(--border);border-radius:8px;overflow:hidden;${sizeStyle}display:flex;flex-direction:column;" data-widget-idx="${i}"><div style="display:flex;justify-content:space-between;align-items:center;padding:0.4rem 0.6rem;border-bottom:1px solid var(--border);flex-shrink:0;"><span style="font-size:0.8rem;font-weight:600;">${WIDGET_TYPES[w.type]?.icon || '📦'} ${WIDGET_TYPES[w.type]?.name || w.type}</span><div style="display:flex;gap:0.3rem;"><button onclick="cycleWidgetSize(${i})" style="background:none;border:none;color:var(--text-muted);cursor:pointer;font-size:0.7rem;" title="Resize">⇔</button><button onclick="moveWidget(${i},-1)" style="background:none;border:none;color:var(--text-muted);cursor:pointer;font-size:0.7rem;" title="Move left">◀</button><button onclick="moveWidget(${i},1)" style="background:none;border:none;color:var(--text-muted);cursor:pointer;font-size:0.7rem;" title="Move right">▶</button><button onclick="removeWidget(${i})" style="background:none;border:none;color:#c0392b;cursor:pointer;font-size:0.7rem;" title="Remove">Close</button></div></div><div class="widget-content" style="flex:1;padding:0.5rem;overflow:auto;" id="widget-content-${i}"></div></div>`;
   }).join('');
   data.widgets.forEach((w, i) => renderWidgetContent(w, i));
  }

  function renderWidgetContent(widget, index) {
   const container = document.getElementById('widget-content-' + index);
   if (!container) return;
   switch (widget.type) {
    case 'clock': {
     function updateClock() {
      if (!document.getElementById('widget-content-' + index)) return;
      const now = new Date();
      container.innerHTML = `<div style="text-align:center;padding:0.5rem;"><div style="font-size:2rem;font-weight:700;color:var(--accent);">${now.toLocaleTimeString()}</div><div style="font-size:0.9rem;color:var(--text-muted);margin-top:0.3rem;">${now.toLocaleDateString(undefined, {weekday:'long',year:'numeric',month:'long',day:'numeric'})}</div></div>`;
      setTimeout(updateClock, 1000);
     }
     updateClock(); break;
    }
    case 'notes': {
     const notes = localStorage.getItem('humanity_notes'); let notesList = []; try { notesList = JSON.parse(notes) || []; } catch {}
     container.innerHTML = notesList.length === 0 ? '<div style="color:var(--text-muted);text-align:center;padding:1rem;">No notes yet. Create them in the Reality tab.</div>' : notesList.map(n => `<div style="padding:0.3rem 0;border-bottom:1px solid var(--border);font-size:0.8rem;"><strong>${n.title || 'Untitled'}</strong><div style="color:var(--text-muted);font-size:0.72rem;max-height:40px;overflow:hidden;">${(n.content || '').substring(0, 100)}</div></div>`).join('');
     break;
    }
    case 'todos': {
     const todos = localStorage.getItem('humanity_todos'); let todoList = []; try { todoList = JSON.parse(todos) || []; } catch {}
     container.innerHTML = todoList.length === 0 ? '<div style="color:var(--text-muted);text-align:center;padding:1rem;">No todos yet. Create them in the Reality tab.</div>' : todoList.map(t => `<div style="padding:0.25rem 0;font-size:0.8rem;display:flex;gap:0.3rem;align-items:start;"><span>${t.done ? '☑' : '☐'}</span><span style="${t.done?'text-decoration:line-through;color:var(--text-muted);':''}">${t.text || t.title || ''}</span></div>`).join('');
     break;
    }
    case 'friends': {
     const peers = (typeof peerData !== 'undefined') ? Object.values(peerData).filter(p => !p.public_key?.startsWith('bot_') && !p.public_key?.startsWith('viewer_')) : [];
     container.innerHTML = peers.length === 0 ? '<div style="color:var(--text-muted);text-align:center;padding:1rem;">Connect to chat to see friends online</div>' : peers.map(p => `<div style="padding:0.2rem 0;font-size:0.8rem;">🟢 ${p.display_name || p.public_key?.substring(0,8) || 'Unknown'} ${p.role === 'admin' ? '👑' : p.role === 'mod' ? '🛡️' : ''}</div>`).join('');
     break;
    }
    case 'stats': {
     fetch('/api/stats').then(r => r.json()).then(stats => {
      container.innerHTML = `<div style="padding:0.3rem;"><div style="display:grid;grid-template-columns:1fr 1fr;gap:0.5rem;"><div style="text-align:center;"><div style="font-size:1.5rem;font-weight:700;color:var(--accent);">${stats.connected_peers || 0}</div><div style="font-size:0.7rem;color:var(--text-muted);">Online</div></div><div style="text-align:center;"><div style="font-size:1.5rem;font-weight:700;color:var(--accent);">${stats.total_messages || 0}</div><div style="font-size:0.7rem;color:var(--text-muted);">Messages</div></div></div></div>`;
     }).catch(() => { container.innerHTML = '<div style="color:var(--text-muted);text-align:center;">Could not load stats</div>'; });
     break;
    }
    case 'quicklinks': {
     const qlData = typeof getBrowseData === 'function' ? getBrowseData() : { quickBar: [] };
     const allSites = typeof getAllSites === 'function' ? getAllSites() : [];
     container.innerHTML = qlData.quickBar.length === 0 ? '<div style="color:var(--text-muted);text-align:center;padding:1rem;">No quick links yet</div>' :
      `<div style="display:flex;flex-wrap:wrap;gap:0.4rem;">${qlData.quickBar.map(url => { const site = allSites.find(s => s.url === url); return `<a href="${url}" target="_blank" rel="noopener" style="background:var(--bg);border:1px solid var(--border);border-radius:6px;padding:0.25rem 0.5rem;font-size:0.75rem;color:var(--text);text-decoration:none;display:flex;align-items:center;gap:0.2rem;">${site?.icon || '🌐'} ${site?.name || new URL(url).hostname}</a>`; }).join('')}</div>`;
     break;
    }
    case 'embed': {
     const embedUrl = widget.config?.url || '';
     if (!embedUrl) { container.innerHTML = `<div style="text-align:center;padding:1rem;"><input type="text" placeholder="Enter URL to embed..." id="embed-url-${index}" style="background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.3rem 0.6rem;border-radius:6px;font-size:0.8rem;width:80%;"><button onclick="setEmbedUrl(${index})" style="background:var(--accent);border:none;border-radius:6px;color:#fff;padding:0.3rem 0.6rem;cursor:pointer;font-size:0.8rem;margin-top:0.3rem;">Load</button></div>`; }
     else { container.style.padding = '0'; container.innerHTML = `<iframe src="${embedUrl}" style="width:100%;height:100%;border:none;" sandbox="allow-scripts allow-same-origin allow-forms allow-popups" referrerpolicy="no-referrer"></iframe>`; }
     break;
    }
    case 'weather': {
     container.innerHTML = '<div style="text-align:center;color:var(--text-muted);padding:1rem;">Loading weather...</div>';
     fetch('https://wttr.in/?format=j1').then(r => r.json()).then(w => {
      const current = w.current_condition?.[0] || {}; const area = w.nearest_area?.[0] || {};
      container.innerHTML = `<div style="text-align:center;padding:0.3rem;"><div style="font-size:1.8rem;">${current.temp_F || '?'}°F</div><div style="font-size:0.8rem;color:var(--text-muted);">${current.weatherDesc?.[0]?.value || 'Unknown'}</div><div style="font-size:0.72rem;color:var(--text-muted);margin-top:0.2rem;">${area.areaName?.[0]?.value || ''}, ${area.region?.[0]?.value || ''}</div><div style="font-size:0.7rem;color:var(--text-muted);margin-top:0.2rem;">💨 ${current.windspeedMiles || '?'} mph · 💧 ${current.humidity || '?'}%</div></div>`;
     }).catch(() => { container.innerHTML = '<div style="color:var(--text-muted);text-align:center;">Weather unavailable</div>'; });
     break;
    }
    case 'chat': {
     const channel = widget.config?.channel || 'general';
     container.innerHTML = '<div style="color:var(--text-muted);text-align:center;padding:1rem;">Loading chat...</div>';
     fetch('/api/messages?limit=15&channel=' + encodeURIComponent(channel)).then(r => r.json()).then(d => {
      if (d.messages && d.messages.length > 0) { container.innerHTML = d.messages.map(m => `<div style="padding:0.2rem 0;font-size:0.78rem;border-bottom:1px solid rgba(255,255,255,0.05);"><span style="color:var(--accent);font-weight:600;">${m.from_name || 'Unknown'}</span><span style="color:var(--text-muted);font-size:0.65rem;margin-left:0.3rem;">${new Date(m.timestamp).toLocaleTimeString()}</span><div style="color:var(--text);">${m.content.substring(0, 200)}</div></div>`).join(''); }
      else { container.innerHTML = '<div style="color:var(--text-muted);text-align:center;padding:1rem;">No messages in #' + channel + '</div>'; }
     }).catch(() => { container.innerHTML = '<div style="color:var(--text-muted);text-align:center;">Could not load chat</div>'; });
     break;
    }
    case 'activity': {
     fetch('/api/messages?limit=20').then(r => r.json()).then(d => {
      if (d.messages && d.messages.length > 0) { container.innerHTML = d.messages.map(m => `<div style="padding:0.2rem 0;font-size:0.78rem;border-bottom:1px solid rgba(255,255,255,0.05);"><span style="font-size:0.65rem;color:var(--text-muted);">#${m.channel || 'general'}</span><span style="color:var(--accent);font-weight:600;margin-left:0.3rem;">${m.from_name || 'Unknown'}</span><span style="color:var(--text-muted);font-size:0.65rem;margin-left:0.3rem;">${new Date(m.timestamp).toLocaleTimeString()}</span><div style="color:var(--text);">${m.content.substring(0, 150)}</div></div>`).join(''); }
      else { container.innerHTML = '<div style="color:var(--text-muted);text-align:center;padding:1rem;">No recent activity</div>'; }
     }).catch(() => { container.innerHTML = '<div style="color:var(--text-muted);text-align:center;">Could not load activity</div>'; });
     break;
    }
    default: container.innerHTML = '<div style="color:var(--text-muted);text-align:center;">Unknown widget type</div>';
   }
  }

  function showAddWidgetModal() {
   const types = Object.entries(WIDGET_TYPES);
   const modal = document.createElement('div');
   modal.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,0.7);z-index:9999;display:flex;align-items:center;justify-content:center;';
   modal.innerHTML = `<div style="background:var(--bg);border:1px solid var(--border);border-radius:12px;padding:1.5rem;max-width:500px;width:90%;max-height:80vh;overflow-y:auto;"><div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:1rem;"><h3 style="margin:0;color:var(--accent);">Add Widget</h3><button onclick="this.closest('[style*=fixed]').remove()" style="background:none;border:none;color:var(--text-muted);cursor:pointer;font-size:1.2rem;">Close</button></div><div style="display:grid;grid-template-columns:1fr 1fr;gap:0.5rem;">${types.map(([key, t]) => `<div onclick="addWidget('${key}');this.closest('[style*=fixed]').remove()" style="padding:0.8rem;background:var(--bg-secondary);border:1px solid var(--border);border-radius:8px;cursor:pointer;text-align:center;" onmouseenter="this.style.borderColor='var(--accent)'" onmouseleave="this.style.borderColor='var(--border)'"><div style="font-size:1.5rem;">${t.icon}</div><div style="font-weight:600;font-size:0.85rem;margin-top:0.2rem;">${t.name}</div><div style="font-size:0.7rem;color:var(--text-muted);margin-top:0.1rem;">${t.description}</div></div>`).join('')}</div></div>`;
   document.body.appendChild(modal);
   modal.addEventListener('click', e => { if (e.target === modal) modal.remove(); });
  }

  function addWidget(type) {
   const data = loadDashboard(); const config = {};
   if (type === 'chat') { const ch = prompt('Channel name:', 'general'); if (!ch) return; config.channel = ch; }
   else if (type === 'embed') { const url = prompt('URL to embed (or leave empty to set later):'); if (url) config.url = url.startsWith('http') ? url : 'https://' + url; }
   data.widgets.push({ type, config, size: WIDGET_TYPES[type]?.defaultSize || 'medium', id: 'w' + Date.now() });
   saveDashboard(data); renderDashboard();
  }
  function removeWidget(index) { const data = loadDashboard(); data.widgets.splice(index, 1); saveDashboard(data); renderDashboard(); }
  function moveWidget(index, direction) { const data = loadDashboard(); const newIndex = index + direction; if (newIndex < 0 || newIndex >= data.widgets.length) return; [data.widgets[index], data.widgets[newIndex]] = [data.widgets[newIndex], data.widgets[index]]; saveDashboard(data); renderDashboard(); }
  function cycleWidgetSize(index) { const data = loadDashboard(); const sizes = ['small', 'medium', 'large']; const current = sizes.indexOf(data.widgets[index].size); data.widgets[index].size = sizes[(current + 1) % sizes.length]; saveDashboard(data); renderDashboard(); }
  function setEmbedUrl(index) { const input = document.getElementById('embed-url-' + index); if (!input) return; let url = input.value; if (!url) return; if (!url.startsWith('http')) url = 'https://' + url; const data = loadDashboard(); data.widgets[index].config = data.widgets[index].config || {}; data.widgets[index].config.url = url; saveDashboard(data); renderDashboard(); }

  // ══════════════════════════════════════════
  // ── Skill DNA System ──
  // ══════════════════════════════════════════
  (function() {
   const SKILL_CATEGORIES = {
    crafting: { name: 'Crafting & Making', icon: '🔨', skills: ['woodworking', 'metalworking', 'welding', 'soldering', 'sewing', 'leatherwork', 'pottery', 'glassblowing', 'blacksmithing', 'cnc', '3d_printing', 'laser_cutting'] },
    engineering: { name: 'Engineering', icon: '⚙️', skills: ['mechanical', 'electrical', 'civil', 'chemical', 'software', 'aerospace', 'robotics', 'plumbing', 'hvac', 'automotive'] },
    agriculture: { name: 'Agriculture', icon: '🌱', skills: ['soil_gardening', 'hydroponics', 'aeroponics', 'aquaponics', 'permaculture', 'composting', 'seed_saving', 'beekeeping', 'animal_husbandry', 'foraging', 'food_preservation', 'mycology'] },
    digital: { name: 'Digital', icon: '💻', skills: ['programming', 'web_dev', 'game_dev', '3d_modeling', 'video_editing', 'audio_production', 'graphic_design', 'ui_ux', 'cybersecurity', 'networking', 'ai_ml', 'database'] },
    survival: { name: 'Survival', icon: '🏕️', skills: ['fire_starting', 'shelter_building', 'water_purification', 'navigation', 'first_aid', 'knot_tying', 'hunting', 'fishing', 'trapping', 'tracking', 'weather_reading'] },
    science: { name: 'Science', icon: '🔬', skills: ['biology', 'chemistry', 'physics', 'astronomy', 'geology', 'ecology', 'mathematics', 'statistics', 'medicine', 'nutrition'] },
    arts: { name: 'Arts & Expression', icon: '🎨', skills: ['drawing', 'painting', 'sculpting', 'music_instrument', 'singing', 'writing', 'photography', 'acting', 'dance', 'calligraphy'] },
    social: { name: 'Social', icon: '🤝', skills: ['teaching', 'leadership', 'negotiation', 'public_speaking', 'counseling', 'languages', 'conflict_resolution', 'mentoring', 'community_organizing'] },
    fitness: { name: 'Fitness & Body', icon: '💪', skills: ['strength_training', 'cardio', 'flexibility', 'martial_arts', 'swimming', 'climbing', 'cycling', 'yoga', 'sports'] },
    home: { name: 'Homesteading', icon: '🏠', skills: ['cooking', 'baking', 'canning', 'cleaning', 'organizing', 'budgeting', 'childcare', 'elder_care', 'home_repair', 'interior_design'] },
   };

   const SKILL_META = {
    woodworking: { name:'Woodworking', icon:'🪚', desc:'Working with wood to create structures and objects', prereqs:[], subskills:['carving','joinery','turning','finishing'] },
    metalworking: { name:'Metalworking', icon:'⚒️', desc:'Shaping and forming metals', prereqs:[], subskills:['forging','casting','machining','sheet_metal'] },
    welding: { name:'Welding', icon:'🔥', desc:'Joining metals using heat and filler material', prereqs:['metalworking'], subskills:['mig','tig','stick','oxy_acetylene','spot'] },
    soldering: { name:'Soldering', icon:'🔌', desc:'Joining electronic components with solder', prereqs:[], subskills:['through_hole','smd','desoldering'] },
    sewing: { name:'Sewing', icon:'🧵', desc:'Joining fabric with needle and thread', prereqs:[], subskills:['hand_sewing','machine','pattern_making'] },
    leatherwork: { name:'Leatherwork', icon:'👜', desc:'Crafting with leather', prereqs:[], subskills:['tooling','stitching','dying'] },
    pottery: { name:'Pottery', icon:'🏺', desc:'Creating objects from clay', prereqs:[], subskills:['wheel','hand_building','glazing'] },
    glassblowing: { name:'Glassblowing', icon:'🔮', desc:'Shaping molten glass', prereqs:[], subskills:['lampwork','furnace','fusing'] },
    blacksmithing: { name:'Blacksmithing', icon:'⚒️', desc:'Forging iron and steel', prereqs:['metalworking'], subskills:['blade_making','tool_making','decorative'] },
    cnc: { name:'CNC', icon:'🖥️', desc:'Computer numerical control machining', prereqs:['metalworking'], subskills:['cam_programming','milling','lathe'] },
    '3d_printing': { name:'3D Printing', icon:'🖨️', desc:'Additive manufacturing', prereqs:['3d_modeling'], subskills:['fdm','resin','slicing'] },
    laser_cutting: { name:'Laser Cutting', icon:'✂️', desc:'Precision cutting with lasers', prereqs:[], subskills:['vector_design','engraving','material_selection'] },
    mechanical: { name:'Mechanical Eng.', icon:'⚙️', desc:'Design and analysis of mechanical systems', prereqs:[], subskills:['cad','thermodynamics','materials'] },
    electrical: { name:'Electrical Eng.', icon:'âš¡', desc:'Electrical systems and circuits', prereqs:[], subskills:['circuit_design','power_systems','signal_processing'] },
    civil: { name:'Civil Eng.', icon:'🏗️', desc:'Infrastructure and structural engineering', prereqs:[], subskills:['structural','geotechnical','transportation'] },
    chemical: { name:'Chemical Eng.', icon:'🧪', desc:'Chemical processes and materials', prereqs:['chemistry'], subskills:['process_design','materials_science'] },
    software: { name:'Software Eng.', icon:'💾', desc:'Software design and architecture', prereqs:['programming'], subskills:['architecture','testing','devops'] },
    aerospace: { name:'Aerospace', icon:'🚀', desc:'Aircraft and spacecraft engineering', prereqs:['mechanical'], subskills:['aerodynamics','propulsion','avionics'] },
    robotics: { name:'Robotics', icon:'🤖', desc:'Design and programming of robots', prereqs:['programming','electrical'], subskills:['kinematics','control_systems','sensors'] },
    plumbing: { name:'Plumbing', icon:'🔧', desc:'Water supply and drainage systems', prereqs:[], subskills:['pipe_fitting','drainage','fixtures'] },
    hvac: { name:'HVAC', icon:'❄️', desc:'Heating, ventilation, and air conditioning', prereqs:[], subskills:['refrigeration','ductwork','controls'] },
    automotive: { name:'Automotive', icon:'🚗', desc:'Vehicle repair and maintenance', prereqs:[], subskills:['engine','transmission','electrical','bodywork'] },
    soil_gardening: { name:'Soil Gardening', icon:'🌻', desc:'Growing plants in soil', prereqs:[], subskills:['vegetables','flowers','fruit_trees','herbs'] },
    hydroponics: { name:'Hydroponics', icon:'💧', desc:'Growing plants without soil in water', prereqs:[], subskills:['nft','dwc','drip'] },
    aeroponics: { name:'Aeroponics', icon:'💨', desc:'Growing plants in air/mist', prereqs:['hydroponics'], subskills:['high_pressure','low_pressure'] },
    aquaponics: { name:'Aquaponics', icon:'🐟', desc:'Combined fish and plant growing', prereqs:['hydroponics','fishing'], subskills:['system_design','fish_care','plant_care'] },
    permaculture: { name:'Permaculture', icon:'🌿', desc:'Sustainable agricultural design', prereqs:['soil_gardening'], subskills:['design','guilds','water_management'] },
    composting: { name:'Composting', icon:'♻️', desc:'Decomposing organic matter into soil', prereqs:[], subskills:['hot','cold','vermicomposting'] },
    seed_saving: { name:'Seed Saving', icon:'🌰', desc:'Collecting and storing seeds', prereqs:['soil_gardening'], subskills:['selection','drying','storage'] },
    beekeeping: { name:'Beekeeping', icon:'🐝', desc:'Managing bee colonies', prereqs:[], subskills:['hive_management','honey_harvest','queen_rearing'] },
    animal_husbandry: { name:'Animal Husbandry', icon:'🐄', desc:'Raising and caring for animals', prereqs:[], subskills:['poultry','livestock','veterinary_basics'] },
    foraging: { name:'Foraging', icon:'🍄', desc:'Finding wild food sources', prereqs:[], subskills:['plant_id','mushroom_id','seasonal'] },
    food_preservation: { name:'Food Preservation', icon:'🫙', desc:'Preserving food for storage', prereqs:[], subskills:['canning_fp','drying','fermenting','smoking'] },
    mycology: { name:'Mycology', icon:'🍄', desc:'Study and cultivation of fungi', prereqs:['biology'], subskills:['identification','cultivation','spawn_production'] },
    programming: { name:'Programming', icon:'👨‍💻', desc:'Writing computer code', prereqs:[], subskills:['python','javascript','rust','c_cpp','go'] },
    web_dev: { name:'Web Dev', icon:'🌐', desc:'Building websites and web apps', prereqs:['programming'], subskills:['frontend','backend','fullstack'] },
    game_dev: { name:'Game Dev', icon:'🎮', desc:'Creating video games', prereqs:['programming'], subskills:['game_design','engine_dev','level_design'] },
    '3d_modeling': { name:'3D Modeling', icon:'🧊', desc:'Creating 3D digital models', prereqs:[], subskills:['hard_surface','organic','sculpting_3d','texturing'] },
    video_editing: { name:'Video Editing', icon:'PREVIEW', desc:'Editing and producing video', prereqs:[], subskills:['cutting','effects','color_grading','motion_graphics'] },
    audio_production: { name:'Audio Production', icon:'🎵', desc:'Recording and producing audio', prereqs:[], subskills:['recording','mixing','mastering','sound_design'] },
    graphic_design: { name:'Graphic Design', icon:'🎨', desc:'Visual communication design', prereqs:[], subskills:['typography','layout','branding','illustration'] },
    ui_ux: { name:'UI/UX Design', icon:'📱', desc:'User interface and experience design', prereqs:[], subskills:['wireframing','prototyping','user_research'] },
    cybersecurity: { name:'Cybersecurity', icon:'🔐', desc:'Protecting systems from threats', prereqs:['programming','networking'], subskills:['pen_testing','forensics','cryptography'] },
    networking: { name:'Networking', icon:'🔗', desc:'Computer network design and management', prereqs:[], subskills:['routing','switching','wireless','security'] },
    ai_ml: { name:'AI/ML', icon:'🧠', desc:'Artificial intelligence and machine learning', prereqs:['programming','mathematics'], subskills:['deep_learning','nlp','computer_vision'] },
    database: { name:'Database', icon:'🗄️', desc:'Database design and management', prereqs:['programming'], subskills:['sql','nosql','optimization'] },
    fire_starting: { name:'Fire Starting', icon:'🔥', desc:'Creating fire from various methods', prereqs:[], subskills:['friction','ferro_rod','bow_drill'] },
    shelter_building: { name:'Shelter Building', icon:'⛺', desc:'Constructing emergency shelters', prereqs:[], subskills:['debris_hut','lean_to','snow_shelter'] },
    water_purification: { name:'Water Purification', icon:'💧', desc:'Making water safe to drink', prereqs:[], subskills:['filtration','chemical','boiling','solar'] },
    navigation: { name:'Navigation', icon:'🧭', desc:'Finding your way without GPS', prereqs:[], subskills:['compass','celestial','map_reading','terrain'] },
    first_aid: { name:'First Aid', icon:'🏥', desc:'Emergency medical treatment', prereqs:[], subskills:['wound_care','cpr','splinting','triage'] },
    knot_tying: { name:'Knot Tying', icon:'🪢', desc:'Tying useful knots', prereqs:[], subskills:['hitches','bends','loops','lashings'] },
    hunting: { name:'Hunting', icon:'🏹', desc:'Hunting wild game', prereqs:[], subskills:['rifle','bow','tracking_hunt','field_dressing'] },
    fishing: { name:'Fishing', icon:'🎣', desc:'Catching fish', prereqs:[], subskills:['fly','spinning','bait','ice_fishing'] },
    trapping: { name:'Trapping', icon:'🪤', desc:'Setting traps for game', prereqs:[], subskills:['snares','deadfalls','cage_traps'] },
    tracking: { name:'Tracking', icon:'🐾', desc:'Following animal and human tracks', prereqs:[], subskills:['footprints','scat','trail_signs'] },
    weather_reading: { name:'Weather Reading', icon:'🌤️', desc:'Predicting weather from natural signs', prereqs:[], subskills:['clouds','wind','barometric','seasonal_patterns'] },
    biology: { name:'Biology', icon:'🧬', desc:'Study of living organisms', prereqs:[], subskills:['molecular','ecology_bio','genetics','microbiology'] },
    chemistry: { name:'Chemistry', icon:'⚗️', desc:'Study of matter and its properties', prereqs:[], subskills:['organic','inorganic','analytical','biochemistry'] },
    physics: { name:'Physics', icon:'⚛️', desc:'Study of matter, energy, and forces', prereqs:['mathematics'], subskills:['mechanics','electromagnetism','quantum','relativity'] },
    astronomy: { name:'Astronomy', icon:'🔭', desc:'Study of celestial objects', prereqs:[], subskills:['observation','astrophotography','cosmology'] },
    geology: { name:'Geology', icon:'🪨', desc:'Study of Earth and its processes', prereqs:[], subskills:['mineralogy','petrology','paleontology'] },
    ecology: { name:'Ecology', icon:'🌍', desc:'Study of ecosystems', prereqs:['biology'], subskills:['conservation','field_study','restoration'] },
    mathematics: { name:'Mathematics', icon:'📐', desc:'Study of numbers, quantities, and shapes', prereqs:[], subskills:['algebra','calculus','geometry','discrete'] },
    statistics: { name:'Statistics', icon:'Quality', desc:'Analysis and interpretation of data', prereqs:['mathematics'], subskills:['probability','regression','bayesian','experimental_design'] },
    medicine: { name:'Medicine', icon:'⚕️', desc:'Practice of healthcare', prereqs:['biology','chemistry'], subskills:['diagnosis','pharmacology','surgery','emergency'] },
    nutrition: { name:'Nutrition', icon:'🥗', desc:'Study of food and health', prereqs:[], subskills:['macro_nutrients','micro_nutrients','diet_planning'] },
    drawing: { name:'Drawing', icon:'✏️', desc:'Creating images with pencil, pen, or digital tools', prereqs:[], subskills:['sketching','figure_drawing','perspective','digital_drawing'] },
    painting: { name:'Painting', icon:'🖌️', desc:'Applying pigment to surfaces', prereqs:[], subskills:['oil','acrylic','watercolor','digital_painting'] },
    sculpting: { name:'Sculpting', icon:'🗿', desc:'Creating 3D art forms', prereqs:[], subskills:['clay_sculpt','stone','wood_sculpt','metal_sculpt'] },
    music_instrument: { name:'Music Instrument', icon:'🎸', desc:'Playing a musical instrument', prereqs:[], subskills:['guitar','piano','drums','strings','wind'] },
    singing: { name:'Singing', icon:'🎤', desc:'Vocal performance', prereqs:[], subskills:['technique','harmony','performance'] },
    writing: { name:'Writing', icon:'✍️', desc:'Creative and technical writing', prereqs:[], subskills:['fiction','nonfiction','poetry','technical'] },
    photography: { name:'Photography', icon:'📷', desc:'Capturing images', prereqs:[], subskills:['portrait','landscape','studio','editing'] },
    acting: { name:'Acting', icon:'🎭', desc:'Theatrical and film performance', prereqs:[], subskills:['stage','screen','voice','improv'] },
    dance: { name:'Dance', icon:'💃', desc:'Movement as artistic expression', prereqs:[], subskills:['ballet','contemporary','hip_hop','ballroom'] },
    calligraphy: { name:'Calligraphy', icon:'🖋️', desc:'Artistic handwriting', prereqs:[], subskills:['western','eastern','modern','lettering'] },
    teaching: { name:'Teaching', icon:'👩‍🏫', desc:'Educating others', prereqs:[], subskills:['curriculum','classroom','online','mentoring_t'] },
    leadership: { name:'Leadership', icon:'👑', desc:'Guiding and inspiring groups', prereqs:[], subskills:['team_building','decision_making','delegation'] },
    negotiation: { name:'Negotiation', icon:'🤝', desc:'Reaching agreements', prereqs:[], subskills:['preparation','persuasion','mediation'] },
    public_speaking: { name:'Public Speaking', icon:'🎙️', desc:'Speaking to audiences', prereqs:[], subskills:['presentation','debate','storytelling'] },
    counseling: { name:'Counseling', icon:'💬', desc:'Helping others through difficulties', prereqs:[], subskills:['active_listening','empathy','crisis_intervention'] },
    languages: { name:'Languages', icon:'🌐', desc:'Speaking foreign languages', prereqs:[], subskills:['reading_lang','writing_lang','conversation','translation'] },
    conflict_resolution: { name:'Conflict Resolution', icon:'☮️', desc:'Resolving disputes peacefully', prereqs:[], subskills:['mediation_cr','facilitation','restorative_justice'] },
    mentoring: { name:'Mentoring', icon:'🧙', desc:'Guiding personal and professional development', prereqs:['teaching'], subskills:['coaching','feedback','career_guidance'] },
    community_organizing: { name:'Community Organizing', icon:'📣', desc:'Building and mobilizing communities', prereqs:[], subskills:['outreach','event_planning','advocacy'] },
    strength_training: { name:'Strength Training', icon:'🏋️', desc:'Building muscular strength', prereqs:[], subskills:['powerlifting','bodybuilding','calisthenics','olympic_lifts'] },
    cardio: { name:'Cardio', icon:'❤️', desc:'Cardiovascular fitness', prereqs:[], subskills:['running','rowing','hiit','jump_rope'] },
    flexibility: { name:'Flexibility', icon:'🤸', desc:'Stretching and mobility', prereqs:[], subskills:['static','dynamic','pnf'] },
    martial_arts: { name:'Martial Arts', icon:'🥋', desc:'Combat and self-defense', prereqs:[], subskills:['striking','grappling','weapons','forms'] },
    swimming: { name:'Swimming', icon:'🏊', desc:'Aquatic movement', prereqs:[], subskills:['freestyle','backstroke','diving','open_water'] },
    climbing: { name:'Climbing', icon:'🧗', desc:'Rock and wall climbing', prereqs:[], subskills:['bouldering','sport','trad','ice'] },
    cycling: { name:'Cycling', icon:'🚴', desc:'Bicycle riding', prereqs:[], subskills:['road','mountain','touring','maintenance'] },
    yoga: { name:'Yoga', icon:'🧘', desc:'Physical and mental yoga practice', prereqs:[], subskills:['vinyasa','hatha','ashtanga','meditation'] },
    sports: { name:'Sports', icon:'âš½', desc:'Organized athletic activities', prereqs:[], subskills:['team_sports','individual','coaching_s','strategy'] },
    cooking: { name:'Cooking', icon:'👨‍🍳', desc:'Preparing food', prereqs:[], subskills:['techniques','cuisines','meal_planning','knife_skills'] },
    baking: { name:'Baking', icon:'🍞', desc:'Baking breads, pastries, and desserts', prereqs:[], subskills:['bread','pastry','cakes','sourdough'] },
    canning: { name:'Canning', icon:'🫙', desc:'Preserving food in jars', prereqs:[], subskills:['water_bath','pressure_canning','pickling','jam'] },
    cleaning: { name:'Cleaning', icon:'🧹', desc:'Maintaining cleanliness', prereqs:[], subskills:['deep_cleaning','organizing_c','laundry','natural_cleaners'] },
    organizing: { name:'Organizing', icon:'📦', desc:'Systematizing spaces and information', prereqs:[], subskills:['decluttering','storage_solutions','digital_org'] },
    budgeting: { name:'Budgeting', icon:'💰', desc:'Managing personal finances', prereqs:[], subskills:['tracking','saving','investing','debt_management'] },
    childcare: { name:'Childcare', icon:'👶', desc:'Caring for children', prereqs:[], subskills:['infant','toddler','education_cc','first_aid_cc'] },
    elder_care: { name:'Elder Care', icon:'👴', desc:'Caring for elderly', prereqs:[], subskills:['daily_assistance','medical_care','companionship'] },
    home_repair: { name:'Home Repair', icon:'🔨', desc:'Fixing and maintaining a home', prereqs:[], subskills:['drywall','painting_hr','basic_plumbing','basic_electrical'] },
    interior_design: { name:'Interior Design', icon:'🛋️', desc:'Designing living spaces', prereqs:[], subskills:['space_planning','color_theory','furniture','lighting'] },
   };

   const LEVEL_THRESHOLDS = [0, 1, 10, 50, 150, 500, 1000, 2500, 5000, 10000, 25000];
   const LEVEL_NAMES = ['Untrained','Novice','Beginner','Apprentice','Journeyman','Skilled','Expert','Master','Grandmaster','Legendary','Transcendent'];
   const LEVEL_COLORS = ['#666','#aaa','#4a8','#2a6','#28f','#a2f','#f2a','#fa2','#f52','#f00','#ff0'];

   function calcLevel(realityXp, fantasyXp) {
    const eff = (realityXp * 2) + fantasyXp;
    for (let i = LEVEL_THRESHOLDS.length - 1; i >= 0; i--) { if (eff >= LEVEL_THRESHOLDS[i]) return i; }
    return 0;
   }

   function xpToNextLevel(realityXp, fantasyXp) {
    const lv = calcLevel(realityXp, fantasyXp);
    if (lv >= 10) return { current: (realityXp*2)+fantasyXp, needed: Infinity, pct: 100 };
    const cur = (realityXp*2)+fantasyXp;
    const next = LEVEL_THRESHOLDS[lv+1];
    const prev = LEVEL_THRESHOLDS[lv];
    return { current: cur - prev, needed: next - prev, pct: Math.min(100, ((cur - prev) / (next - prev)) * 100) };
   }

   function loadSkills() {
    try { return JSON.parse(localStorage.getItem('humanity_skills')) || { skills: {}, verifications: [] }; }
    catch { return { skills: {}, verifications: [] }; }
   }
   function saveSkills(data) { localStorage.setItem('humanity_skills', JSON.stringify(data)); }

   function getSkillData(data, skillId) {
    return data.skills[skillId] || { realityXp: 0, fantasyXp: 0, logs: [], subskills: {} };
   }

   function checkPrereqs(data, skillId) {
    const meta = SKILL_META[skillId];
    if (!meta || !meta.prereqs || meta.prereqs.length === 0) return true;
    return meta.prereqs.every(pId => {
     const sd = getSkillData(data, pId);
     return calcLevel(sd.realityXp, sd.fantasyXp) >= 1;
    });
   }

   // ── Render Skill DNA panel ──
   function renderSkillDNA(containerId, mode) {
    const container = document.getElementById(containerId);
    if (!container) return;
    const data = loadSkills();
    const isFantasy = mode === 'fantasy';
    const accentColor = isFantasy ? 'var(--fantasy-accent,#9966ff)' : 'var(--accent)';

    // State
    if (!container._sdState) container._sdState = { category: 'crafting', view: 'grid' };
    const st = container._sdState;

    let html = '<div class="skill-dna">';
    // Sidebar
    html += '<div class="skill-dna-sidebar">';
    for (const [catId, cat] of Object.entries(SKILL_CATEGORIES)) {
     const isActive = st.category === catId;
     html += `<div class="sd-cat ${isActive?'active':''}" onclick="window._sdSelectCat('${containerId}','${catId}','${mode}')">${cat.icon} <span>${cat.name}</span></div>`;
    }
    // Stats & Find People
    html += `<div class="sd-cat" style="margin-top:auto;border-top:1px solid var(--border);" onclick="window._sdShowOverview('${containerId}','${mode}')">Quality Overview</div>`;
    html += `<div class="sd-cat" onclick="window._sdFindPeople()">👥 Find People</div>`;
    html += '</div>';

    // Main content
    const cat = SKILL_CATEGORIES[st.category];
    html += '<div class="skill-dna-main">';
    if (st.view === 'grid') {
     html += `<div class="skill-dna-header"><h3 style="color:${accentColor}">${cat.icon} ${cat.name}</h3>`;
     html += `<button onclick="window._sdLogActivity('${containerId}','${mode}','${st.category}')" style="background:${accentColor};color:#fff;border:none;padding:0.3rem 0.7rem;border-radius:6px;font-size:0.78rem;cursor:pointer;font-weight:600;">+ Log Activity</button>`;
     html += '</div>';
     html += '<div class="sd-grid">';
     for (const skillId of cat.skills) {
      const meta = SKILL_META[skillId];
      if (!meta) continue;
      const sd = getSkillData(data, skillId);
      const rXp = isFantasy ? sd.fantasyXp : sd.realityXp;
      const fXp = isFantasy ? sd.realityXp : sd.fantasyXp;
      const lv = calcLevel(sd.realityXp, sd.fantasyXp);
      const prog = xpToNextLevel(sd.realityXp, sd.fantasyXp);
      const locked = !checkPrereqs(data, skillId);
      const color = LEVEL_COLORS[lv];
      const verCount = (data.verifications || []).filter(v => v.skill === skillId).length;
      html += '<div class="sd-tile ' + (locked?'locked':'') + '" onclick="' + (locked?'':("window._sdOpenSkill('"+containerId+"','"+skillId+"','"+mode+"')")) + '">';
      html += `<div class="sd-icon">${meta.icon}</div>`;
      html += `<div class="sd-name">${meta.name}</div>`;
      html += `<div class="sd-level" style="color:${color}">Lv ${lv} ${verCount?'✓':''}</div>`;
      html += `<div class="sd-bar"><div class="sd-bar-fill" style="width:${prog.pct}%;background:${color}"></div></div>`;
      html += '</div>';
     }
     html += '</div>';

     // Stats summary
     let totalR = 0, totalF = 0, verified = 0, verifs = (data.verifications||[]).length;
     for (const [sid, sd] of Object.entries(data.skills)) { totalR += sd.realityXp||0; totalF += sd.fantasyXp||0; }
     const uniqueVerified = new Set((data.verifications||[]).map(v=>v.skill)).size;
     html += '<div class="sd-stats">';
     html += `<div>Total Reality XP: <span>${Math.round(totalR)}h</span></div>`;
     html += `<div>Total Fantasy XP: <span>${Math.round(totalF)}h</span></div>`;
     html += `<div>Verified Skills: <span>${uniqueVerified}</span> (${verifs} verifications)</div>`;
     html += '</div>';
    } else if (st.view === 'overview') {
     html += renderOverview(data, isFantasy, accentColor, containerId, mode);
    }
    html += '</div></div>';
    container.innerHTML = html;
   }

   function renderOverview(data, isFantasy, accentColor, containerId, mode) {
    let html = `<div class="skill-dna-header"><h3 style="color:${accentColor}">Quality Skill Overview</h3>`;
    html += `<button onclick="window._sdSelectCat('${containerId}','crafting','${mode}')" style="background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.3rem 0.7rem;border-radius:6px;font-size:0.78rem;cursor:pointer;">← Back</button></div>`;

    // Category bar chart
    html += '<div style="margin:0.6rem 0;">';
    for (const [catId, cat] of Object.entries(SKILL_CATEGORIES)) {
     let catHours = 0;
     for (const sid of cat.skills) { const sd = getSkillData(data, sid); catHours += (sd.realityXp||0) + (sd.fantasyXp||0); }
     const maxH = 500;
     const pct = Math.min(100, (catHours / maxH) * 100);
     html += `<div style="display:flex;align-items:center;gap:0.4rem;margin:0.2rem 0;font-size:0.78rem;">`;
     html += `<span style="width:120px;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;">${cat.icon} ${cat.name}</span>`;
     html += `<div style="flex:1;height:12px;background:rgba(255,255,255,0.06);border-radius:4px;overflow:hidden;">`;
     html += `<div style="height:100%;width:${pct}%;background:${accentColor};border-radius:4px;"></div></div>`;
     html += `<span style="width:50px;text-align:right;color:var(--text-muted);">${Math.round(catHours)}h</span></div>`;
    }
    html += '</div>';

    // Top skills
    const allSkills = [];
    for (const [sid, sd] of Object.entries(data.skills)) {
     const lv = calcLevel(sd.realityXp||0, sd.fantasyXp||0);
     if (lv > 0) allSkills.push({ id: sid, lv, total: (sd.realityXp||0)+(sd.fantasyXp||0) });
    }
    allSkills.sort((a,b) => b.total - a.total);
    if (allSkills.length) {
     html += '<h4 style="margin:0.8rem 0 0.3rem;font-size:0.85rem;">🏆 Top Skills</h4>';
     for (const s of allSkills.slice(0, 8)) {
      const meta = SKILL_META[s.id] || { name: s.id, icon: '❓' };
      html += `<div style="display:flex;align-items:center;gap:0.4rem;margin:0.15rem 0;font-size:0.8rem;">`;
      html += `${meta.icon} ${meta.name} — <span style="color:${LEVEL_COLORS[s.lv]};font-weight:700;">Lv ${s.lv} ${LEVEL_NAMES[s.lv]}</span> (${Math.round(s.total)}h)</div>`;
     }
    }

    // Achievements
    html += '<h4 style="margin:0.8rem 0 0.3rem;font-size:0.85rem;">🏅 Milestones</h4>';
    let totalR = 0, totalF = 0;
    for (const sd of Object.values(data.skills)) { totalR += sd.realityXp||0; totalF += sd.fantasyXp||0; }
    const milestones = [
     { name: 'First Steps', desc: 'Log your first activity', check: Object.keys(data.skills).length > 0 },
     { name: '100 Reality Hours', desc: 'Accumulate 100h of real-world XP', check: totalR >= 100 },
     { name: 'Verified', desc: 'Get your first peer verification', check: (data.verifications||[]).length > 0 },
     { name: 'Jack of All Trades', desc: 'Skills in 5+ categories', check: new Set(Object.keys(data.skills).map(s => { for (const [c,v] of Object.entries(SKILL_CATEGORIES)) if (v.skills.includes(s)) return c; return null; }).filter(Boolean)).size >= 5 },
     { name: '5 Verified', desc: '5 skills peer-verified', check: new Set((data.verifications||[]).map(v=>v.skill)).size >= 5 },
    ];
    for (const m of milestones) {
     html += `<div style="font-size:0.78rem;margin:0.15rem 0;opacity:${m.check?1:0.4};">${m.check?'✅':'⬜'} <strong>${m.name}</strong> — ${m.desc}</div>`;
    }

    // Radar chart (canvas)
    html += `<h4 style="margin:0.8rem 0 0.3rem;font-size:0.85rem;">🎯 Skill Radar</h4>`;
    html += `<canvas id="sd-radar-${containerId}" width="280" height="280" style="display:block;margin:0 auto;max-width:100%;"></canvas>`;
    setTimeout(() => drawRadar(containerId, data), 50);
    return html;
   }

   function drawRadar(containerId, data) {
    const canvas = document.getElementById('sd-radar-' + containerId);
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    const w = canvas.width, h = canvas.height, cx = w/2, cy = h/2, r = Math.min(cx,cy) - 30;
    ctx.clearRect(0,0,w,h);
    const cats = Object.entries(SKILL_CATEGORIES);
    const n = cats.length;
    // Grid
    ctx.strokeStyle = 'rgba(255,255,255,0.1)';
    for (let ring = 1; ring <= 5; ring++) {
     ctx.beginPath();
     for (let i = 0; i <= n; i++) {
      const a = (Math.PI*2*i/n) - Math.PI/2;
      const rr = r * ring/5;
      const x = cx + Math.cos(a)*rr, y = cy + Math.sin(a)*rr;
      i===0 ? ctx.moveTo(x,y) : ctx.lineTo(x,y);
     }
     ctx.stroke();
    }
    // Spokes + labels
    ctx.fillStyle = 'rgba(255,255,255,0.5)'; ctx.font = '9px sans-serif'; ctx.textAlign = 'center';
    for (let i = 0; i < n; i++) {
     const a = (Math.PI*2*i/n) - Math.PI/2;
     ctx.beginPath(); ctx.moveTo(cx,cy); ctx.lineTo(cx+Math.cos(a)*r, cy+Math.sin(a)*r); ctx.stroke();
     const lx = cx + Math.cos(a)*(r+18), ly = cy + Math.sin(a)*(r+18);
     ctx.fillText(cats[i][1].icon, lx, ly+3);
    }
    // Data
    const vals = cats.map(([catId, cat]) => {
     let total = 0;
     for (const sid of cat.skills) { const sd = getSkillData(data, sid); total += (sd.realityXp||0)*2 + (sd.fantasyXp||0); }
     return Math.min(1, total / 2000); // normalize
    });
    ctx.fillStyle = 'rgba(0,180,100,0.2)'; ctx.strokeStyle = 'rgba(0,180,100,0.8)'; ctx.lineWidth = 2;
    ctx.beginPath();
    for (let i = 0; i <= n; i++) {
     const idx = i % n;
     const a = (Math.PI*2*idx/n) - Math.PI/2;
     const v = Math.max(0.02, vals[idx]);
     const x = cx + Math.cos(a)*r*v, y = cy + Math.sin(a)*r*v;
     i===0 ? ctx.moveTo(x,y) : ctx.lineTo(x,y);
    }
    ctx.fill(); ctx.stroke();
   }

   // ── Skill Card (detail view) ──
   window._sdOpenSkill = function(containerId, skillId, mode) {
    const data = loadSkills();
    const meta = SKILL_META[skillId];
    if (!meta) return;
    const sd = getSkillData(data, skillId);
    const lv = calcLevel(sd.realityXp||0, sd.fantasyXp||0);
    const prog = xpToNextLevel(sd.realityXp||0, sd.fantasyXp||0);
    const verifs = (data.verifications||[]).filter(v => v.skill === skillId);
    const isFantasy = mode === 'fantasy';
    const accent = isFantasy ? '#9966ff' : 'var(--accent)';

    let html = '<div class="sd-card-overlay" onclick="if(event.target===this)this.remove()">';
    html += '<div class="sd-card">';
    html += `<h3>${meta.icon} ${meta.name} <span style="color:${LEVEL_COLORS[lv]}">Lv ${lv} ${LEVEL_NAMES[lv]}</span></h3>`;
    html += `<p style="font-size:0.78rem;color:var(--text-muted);margin:0 0 0.4rem;">${meta.desc}</p>`;

    // Progress bar
    const progLabel = lv >= 10 ? 'MAX' : `${Math.round(prog.needed - prog.current)}h to Lv ${lv+1}`;
    html += `<div style="display:flex;align-items:center;gap:0.4rem;font-size:0.75rem;color:var(--text-muted);">`;
    html += `<div style="flex:1;height:8px;background:rgba(255,255,255,0.08);border-radius:4px;overflow:hidden;"><div style="height:100%;width:${prog.pct}%;background:${LEVEL_COLORS[lv]};border-radius:4px;"></div></div> ${progLabel}</div>`;

    // XP bars
    html += '<div class="sd-xp-bars">';
    const maxXp = Math.max(sd.realityXp||1, sd.fantasyXp||1, 10);
    html += `<div class="sd-xp-row"><span style="width:70px;">Reality XP:</span> <span>${Math.round(sd.realityXp||0)}h</span> <div class="bar"><div class="bar-fill" style="width:${((sd.realityXp||0)/maxXp)*100}%;background:#4a8;"></div></div></div>`;
    html += `<div class="sd-xp-row"><span style="width:70px;">Fantasy XP:</span> <span>${Math.round(sd.fantasyXp||0)}h</span> <div class="bar"><div class="bar-fill" style="width:${((sd.fantasyXp||0)/maxXp)*100}%;background:#96f;"></div></div></div>`;
    html += '</div>';

    // Subskills
    if (meta.subskills && meta.subskills.length) {
     html += '<div class="sd-subskills"><strong style="font-size:0.8rem;">Subskills:</strong>';
     for (const sub of meta.subskills) {
      const subData = (sd.subskills||{})[sub] || { realityXp:0, fantasyXp:0 };
      const subLv = calcLevel(subData.realityXp, subData.fantasyXp);
      const subProg = xpToNextLevel(subData.realityXp, subData.fantasyXp);
      const subName = sub.replace(/_/g,' ').replace(/\b\w/g,c=>c.toUpperCase());
      html += `<div class="sd-subskill"><span style="width:100px;">${subName}</span>`;
      html += `<div style="flex:1;height:6px;background:rgba(255,255,255,0.08);border-radius:3px;overflow:hidden;"><div style="height:100%;width:${subProg.pct}%;background:${LEVEL_COLORS[subLv]};border-radius:3px;"></div></div>`;
      html += `<span style="color:${LEVEL_COLORS[subLv]};font-weight:600;min-width:30px;text-align:right;">Lv ${subLv}</span></div>`;
     }
     html += '</div>';
    }

    // Verifications
    if (verifs.length) {
     html += `<div class="sd-verifications"><strong style="font-size:0.8rem;">Verifications (${verifs.length}):</strong>`;
     for (const v of verifs) {
      html += `<div style="margin:0.15rem 0;">✓ <strong>${v.from||'Anonymous'}</strong> — "${v.note||'Verified'}" <span style="color:var(--text-muted);font-size:0.7rem;">${v.date ? new Date(v.date).toLocaleDateString() : ''}</span></div>`;
     }
     html += '</div>';
    }

    // Recent activity
    const logs = (sd.logs||[]).slice(-5).reverse();
    if (logs.length) {
     html += '<div class="sd-activity"><strong style="font-size:0.8rem;">Recent Activity:</strong>';
     for (const log of logs) {
      const d = log.date ? new Date(log.date).toLocaleDateString() : '?';
      const sub = log.subskill ? ` ${log.subskill.replace(/_/g,' ')}` : '';
      html += `<div style="margin:0.15rem 0;">${d} — ${log.hours||0}h${sub} (${log.type||'reality'}) ${log.note?'— '+log.note:''}</div>`;
     }
     html += '</div>';
    }

    // Actions
    html += '<div class="sd-actions">';
    html += `<button onclick="window._sdLogActivity('${containerId}','${mode}','${Object.entries(SKILL_CATEGORIES).find(([c,v])=>v.skills.includes(skillId))?.[0]||'crafting'}','${skillId}');this.closest('.sd-card-overlay').remove()">+ Log Hours</button>`;
    html += `<button onclick="window._sdRequestVerify('${skillId}');this.closest('.sd-card-overlay').remove()">Request Verify</button>`;
    html += '</div>';
    html += '</div></div>';
    document.body.insertAdjacentHTML('beforeend', html);
   };

   // ── Category selection ──
   window._sdSelectCat = function(containerId, catId, mode) {
    const container = document.getElementById(containerId);
    if (!container) return;
    container._sdState = { category: catId, view: 'grid' };
    renderSkillDNA(containerId, mode);
   };

   window._sdShowOverview = function(containerId, mode) {
    const container = document.getElementById(containerId);
    if (!container) return;
    container._sdState = container._sdState || {};
    container._sdState.view = 'overview';
    renderSkillDNA(containerId, mode);
   };

   // ── Log Activity Modal ──
   window._sdLogActivity = function(containerId, mode, defaultCat, defaultSkill) {
    const isFantasy = mode === 'fantasy';
    let html = '<div class="sd-log-overlay" onclick="if(event.target===this)this.remove()">';
    html += '<div class="sd-log-modal">';
    html += '<h3>+ Log Activity</h3>';
    html += '<div class="sd-type-toggle">';
    html += `<label><input type="radio" name="sd-log-type" value="reality" ${!isFantasy?'checked':''}> Reality</label>`;
    html += `<label><input type="radio" name="sd-log-type" value="fantasy" ${isFantasy?'checked':''}> Fantasy</label>`;
    html += '</div>';
    html += '<label>Category</label><select id="sd-log-cat" onchange="window._sdLogCatChange()">';
    for (const [catId, cat] of Object.entries(SKILL_CATEGORIES)) {
     html += `<option value="${catId}" ${catId===defaultCat?'selected':''}>${cat.icon} ${cat.name}</option>`;
    }
    html += '</select>';
    html += '<label>Skill</label><select id="sd-log-skill" onchange="window._sdLogSkillChange()"></select>';
    html += '<label>Subskill (optional)</label><select id="sd-log-sub"><option value="">— None —</option></select>';
    html += '<label>Hours</label><input type="number" id="sd-log-hours" min="0.25" step="0.25" value="1">';
    html += '<label>Notes</label><textarea id="sd-log-notes" rows="2" placeholder="What did you do?"></textarea>';
    html += `<label>Date</label><input type="date" id="sd-log-date" value="${new Date().toISOString().slice(0,10)}">`;
    html += '<div class="sd-log-actions">';
    html += `<button style="background:var(--bg-input);border:1px solid var(--border);color:var(--text);" onclick="this.closest('.sd-log-overlay').remove()">Cancel</button>`;
    html += `<button style="background:${isFantasy?'#9966ff':'var(--accent)'};color:#fff;" onclick="window._sdSubmitLog('${containerId}','${mode}')">Log Activity</button>`;
    html += '</div></div></div>';
    document.body.insertAdjacentHTML('beforeend', html);
    // Populate skill dropdown
    window._sdLogCatChange(defaultSkill);
   };

   window._sdLogCatChange = function(defaultSkill) {
    const catId = document.getElementById('sd-log-cat').value;
    const cat = SKILL_CATEGORIES[catId];
    const sel = document.getElementById('sd-log-skill');
    sel.innerHTML = '';
    for (const sid of cat.skills) {
     const meta = SKILL_META[sid];
     if (!meta) continue;
     sel.innerHTML += `<option value="${sid}" ${sid===defaultSkill?'selected':''}>${meta.icon} ${meta.name}</option>`;
    }
    window._sdLogSkillChange();
   };

   window._sdLogSkillChange = function() {
    const skillId = document.getElementById('sd-log-skill').value;
    const meta = SKILL_META[skillId];
    const sel = document.getElementById('sd-log-sub');
    sel.innerHTML = '<option value="">— None —</option>';
    if (meta && meta.subskills) {
     for (const sub of meta.subskills) {
      sel.innerHTML += `<option value="${sub}">${sub.replace(/_/g,' ').replace(/\b\w/g,c=>c.toUpperCase())}</option>`;
     }
    }
   };

   window._sdSubmitLog = function(containerId, mode) {
    const type = document.querySelector('input[name="sd-log-type"]:checked')?.value || 'reality';
    const skillId = document.getElementById('sd-log-skill').value;
    const subskill = document.getElementById('sd-log-sub').value;
    const hours = parseFloat(document.getElementById('sd-log-hours').value) || 0;
    const notes = document.getElementById('sd-log-notes').value;
    const date = document.getElementById('sd-log-date').value;
    if (!skillId || hours <= 0) return;

    const data = loadSkills();
    if (!data.skills[skillId]) data.skills[skillId] = { realityXp: 0, fantasyXp: 0, logs: [], subskills: {} };
    const sd = data.skills[skillId];

    if (type === 'reality') sd.realityXp = (sd.realityXp||0) + hours;
    else sd.fantasyXp = (sd.fantasyXp||0) + hours;

    if (subskill) {
     if (!sd.subskills) sd.subskills = {};
     if (!sd.subskills[subskill]) sd.subskills[subskill] = { realityXp: 0, fantasyXp: 0 };
     if (type === 'reality') sd.subskills[subskill].realityXp += hours;
     else sd.subskills[subskill].fantasyXp += hours;
    }

    if (!sd.logs) sd.logs = [];
    sd.logs.push({ date: date || new Date().toISOString(), type, hours, note: notes, subskill: subskill || null });

    saveSkills(data);
    document.querySelector('.sd-log-overlay')?.remove();
    renderSkillDNA(containerId, mode);

    // Broadcast skill update to relay if connected
    _sdBroadcastUpdate(skillId, sd);
   };

   function _sdBroadcastUpdate(skillId, sd) {
    // If we have a WebSocket connection, send skill update
    if (window._humanityWs && window._humanityWs.readyState === 1) {
     const lv = calcLevel(sd.realityXp||0, sd.fantasyXp||0);
     try {
      window._humanityWs.send(JSON.stringify({
       type: 'skill_update',
       skill_id: skillId,
       reality_xp: sd.realityXp||0,
       fantasy_xp: sd.fantasyXp||0,
       level: lv
      }));
     } catch(e) { /* silent */ }
    }
   }

   // ── Request Verification ──
   window._sdRequestVerify = function(skillId) {
    const meta = SKILL_META[skillId];
    if (!meta) return;
    const data = loadSkills();
    const sd = getSkillData(data, skillId);
    const lv = calcLevel(sd.realityXp||0, sd.fantasyXp||0);
    // Simple prompt-based verification request for now
    const who = prompt(`Request verification for ${meta.name} (Lv ${lv}).\n\nEnter the username of someone who can verify your skill:`);
    if (!who || !who.trim()) return;
    // Send via WS if connected
    if (window._humanityWs && window._humanityWs.readyState === 1) {
     try {
      window._humanityWs.send(JSON.stringify({
       type: 'skill_verify_request',
       skill_id: skillId,
       level: lv,
       to_name: who.trim()
      }));
      alert('Verification request sent to ' + who.trim() + '!');
     } catch(e) { alert('Could not send request. Are you connected?'); }
    } else {
     alert('Not connected to relay. Connect first to send verification requests.');
    }
   };

   // Handle incoming verification responses
   window._sdHandleVerifyResponse = function(msg) {
    if (msg.type === 'skill_verify_response' && msg.approved) {
     const data = loadSkills();
     if (!data.verifications) data.verifications = [];
     data.verifications.push({
      skill: msg.skill_id,
      from: msg.from_name || msg.from_key || 'Unknown',
      date: Date.now(),
      note: msg.note || 'Verified'
     });
     saveSkills(data);
     // Re-render if visible
     renderSkillDNA('skill-dna-reality', 'reality');
     renderSkillDNA('skill-dna-fantasy', 'fantasy');
    }
   };

   // ── Find People ──
   window._sdFindPeople = async function() {
    let html = '<div class="sd-find-overlay" onclick="if(event.target===this)this.remove()">';
    html += '<div class="sd-find-modal">';
    html += '<h3>👥 Find People by Skill</h3>';
    html += '<div style="display:flex;gap:0.4rem;margin-bottom:0.6rem;">';
    html += '<select id="sd-find-skill" style="flex:1;background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.35rem;border-radius:6px;font-size:0.82rem;">';
    for (const [catId, cat] of Object.entries(SKILL_CATEGORIES)) {
     html += `<optgroup label="${cat.icon} ${cat.name}">`;
     for (const sid of cat.skills) {
      const meta = SKILL_META[sid];
      if (meta) html += `<option value="${sid}">${meta.name}</option>`;
     }
     html += '</optgroup>';
    }
    html += '</select>';
    html += '<select id="sd-find-level" style="width:80px;background:var(--bg-input);border:1px solid var(--border);color:var(--text);padding:0.35rem;border-radius:6px;font-size:0.82rem;">';
    for (let i = 1; i <= 10; i++) html += `<option value="${i}">Lv ${i}+</option>`;
    html += '</select>';
    html += `<button onclick="window._sdDoSearch()" style="background:var(--accent);color:#fff;border:none;padding:0.35rem 0.7rem;border-radius:6px;font-size:0.82rem;cursor:pointer;font-weight:600;">Search</button>`;
    html += '</div>';
    html += '<div id="sd-find-results" style="font-size:0.82rem;color:var(--text-muted);">Select a skill and minimum level, then search.</div>';
    html += '</div></div>';
    document.body.insertAdjacentHTML('beforeend', html);
   };

   window._sdDoSearch = async function() {
    const skill = document.getElementById('sd-find-skill').value;
    const minLevel = document.getElementById('sd-find-level').value;
    const resultsDiv = document.getElementById('sd-find-results');
    resultsDiv.innerHTML = 'Searching...';
    try {
     const res = await fetch(`/api/skills/search?skill=${skill}&min_level=${minLevel}`);
     if (!res.ok) throw new Error('Search failed');
     const users = await res.json();
     if (!users.length) { resultsDiv.innerHTML = 'No users found with that skill level.'; return; }
     let rHtml = '';
     for (const u of users) {
      const meta = SKILL_META[skill];
      rHtml += `<div class="sd-find-result">`;
      rHtml += `<div><strong>${u.display_name || u.public_key.slice(0,8)+'...'}</strong>`;
      rHtml += `<div class="sd-fr-skills">${meta?meta.icon:''} ${meta?meta.name:skill} — <span style="color:${LEVEL_COLORS[u.level]||'#aaa'};font-weight:700;">Lv ${u.level} ${LEVEL_NAMES[u.level]||''}</span> (${Math.round(u.reality_xp||0)}h reality, ${Math.round(u.fantasy_xp||0)}h fantasy)</div>`;
      rHtml += `</div></div>`;
     }
     resultsDiv.innerHTML = rHtml;
    } catch(e) {
     resultsDiv.innerHTML = 'Search unavailable. Make sure you\'re connected to a relay.';
    }
   };

   // ── Initialize both panels ──
   function initSkillDNA() {
    renderSkillDNA('skill-dna-reality', 'reality');
    renderSkillDNA('skill-dna-fantasy', 'fantasy');
   }

   // Hook into tab switching to init when visible
   const origSwitchTabSD = switchTab;
   switchTab = function(tabId, pushState) {
    origSwitchTabSD(tabId, pushState);
    if (tabId === 'reality' || tabId === 'fantasy') initSkillDNA();
   };

   // Init on load if starting on reality/fantasy
   if (typeof initialTab !== 'undefined' && (initialTab === 'reality' || initialTab === 'fantasy')) {
    initSkillDNA();
   }
   // Also init after short delay for default tab
   setTimeout(initSkillDNA, 500);
  })();

  // ── Browse/Dashboard tab activation ──
  {
   const origSwitchTab3 = switchTab;
   switchTab = function(tabId, pushState) {
    origSwitchTab3(tabId, pushState);
    if (tabId === 'browse') initBrowseTab();
    if (tabId === 'dashboard') renderDashboard();
   };
   if (initialTab === 'browse') initBrowseTab();
   if (initialTab === 'dashboard') renderDashboard();
  }

  // ── Mobile responsive for browse/dashboard ──
  (function() {
   const style = document.createElement('style');
   style.textContent = '@media (max-width: 768px) { #tab-browse > div:nth-child(2) { flex-direction: column !important; } #browse-directory { width: 100% !important; min-width: auto !important; max-height: 40vh; border-right: none !important; border-bottom: 1px solid var(--border); } #dashboard-grid { grid-template-columns: 1fr !important; } .dashboard-widget { grid-column: span 1 !important; } }';
   document.head.appendChild(style);
  })();

