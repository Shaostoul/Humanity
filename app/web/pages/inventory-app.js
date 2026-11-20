// Moved out of web/pages/inventory.html (2026-11-20) so the site's Content-Security-Policy can refuse
// inline script (docs/PRIORITIES.md item 6b). It runs at the same place in the page as before.
// ═════════════════════════════════════════════
// TAB SWITCHING
// ═════════════════════════════════════════════
let activeGearTab = 'inventory';

function switchGearTab(tab) {
  activeGearTab = tab;
  document.querySelectorAll('.gear-tab').forEach(el => el.classList.toggle('active', el.dataset.tab === tab));
  document.getElementById('inventory-content').style.display = tab === 'inventory' ? '' : 'none';
  document.getElementById('equipment-content').style.display = tab === 'equipment' ? '' : 'none';
  localStorage.setItem('hos_gear_active_tab', tab);
}

// ═════════════════════════════════════════════
// INVENTORY
// ═════════════════════════════════════════════
const INV_STORAGE_KEY = 'hos_inventory_v1';
let invItems = [];
let invEditingId = null;

function load() {
  try { invItems = JSON.parse(localStorage.getItem(INV_STORAGE_KEY)) || []; } catch (e) { invItems = []; }
}
function save() { localStorage.setItem(INV_STORAGE_KEY, JSON.stringify(invItems)); }
function nextId() { return Date.now().toString(36) + Math.random().toString(36).slice(2,6); }

// ── State ──
let currentFilter = 'player:worn';
let currentView = 'grid';

function getFilteredItems() {
  const q = document.getElementById('inv-search').value.toLowerCase().trim();
  let result;
  if (currentFilter === 'fleet:all') {
    result = invItems.filter(i => i.category.startsWith('fleet:'));
  } else {
    result = invItems.filter(i => i.category === currentFilter);
  }
  if (q) result = result.filter(i => i.name.toLowerCase().includes(q) || (i.location||'').toLowerCase().includes(q) || (i.notes||'').toLowerCase().includes(q));
  return result;
}

// ── Sidebar ──
function toggleSection(id) {
  document.getElementById('sec-' + id).classList.toggle('collapsed');
}

function setFilter(filter, el) {
  currentFilter = filter;
  document.querySelectorAll('.inv-cat').forEach(c => c.classList.remove('active'));
  el.classList.add('active');

  const labels = {
    'player:worn':     '🧥 Worn / Equipped',
    'player:backpack': '🎒 Backpack',
    'player:hotbar':   '⚡ Hotbar',
    'player:vehicle':  '🚗 Player Vehicles',
    'home:rooms':      hosIcon('home', 16) + ' Rooms / Areas',
    'home:storage':    hosIcon('box', 16) + ' Home Storage',
    'home:workshop':   '🔧 Workshop',
    'home:vehicle':    '🚙 Home Vehicles',
    'fleet:all':       '📋 All Fleet Items',
    'fleet:blueprints':hosIcon('blueprint', 16) + ' Blueprints',
    'fleet:resources': '⛏️ Resources',
  };
  document.getElementById('topbar-title').innerHTML = labels[filter] || filter;
  renderItems();
}

function updateCounts() {
  const cats = ['player:worn','player:backpack','player:hotbar','player:vehicle',
                 'home:rooms','home:storage','home:workshop','home:vehicle',
                 'fleet:all','fleet:blueprints','fleet:resources'];
  cats.forEach(cat => {
    const el = document.getElementById('cnt-' + cat);
    if (!el) return;
    if (cat === 'fleet:all') {
      el.textContent = invItems.filter(i => i.category.startsWith('fleet:')).length || '';
    } else {
      const n = invItems.filter(i => i.category === cat).length;
      el.textContent = n || '';
    }
  });
}

// ── Render ──
function setView(v) {
  currentView = v;
  document.getElementById('vbtn-grid').classList.toggle('active', v === 'grid');
  document.getElementById('vbtn-list').classList.toggle('active', v === 'list');
  renderItems();
}

function renderItems() {
  const content = document.getElementById('inv-content');
  const filtered = getFilteredItems();

  if (filtered.length === 0) {
    content.innerHTML = `<div class="inv-empty-state">
      <div class="big">📭</div>
      <p>Nothing here yet. This is your own inventory, saved on this device.</p>
      <button ${hosOn.attr('click', `openAddModal()`)}>+ Add your first item</button>
    </div>`;
    return;
  }

  if (currentView === 'grid') {
    content.innerHTML = `<div class="item-grid">${filtered.map(buildItemCard).join('')}</div>`;
  } else {
    content.innerHTML = `<div class="item-list">${filtered.map(buildItemRow).join('')}</div>`;
  }
}

function buildItemCard(item) {
  const icon = item.icon || hosIcon('box', 24);
  return `<div role="button" tabindex="0" class="item-card" ${hosOn.attr('click', `openEditModal(${hosArg(item.id)})`)}>
    <div class="item-img">${icon}</div>
    <div class="item-name">${esc(item.name)}</div>
    <div class="item-meta">${esc(item.location || '')}</div>
    ${item.qty > 1 ? `<div class="item-qty-badge">×${item.qty}</div>` : ''}
  </div>`;
}

function buildItemRow(item) {
  const icon = item.icon || hosIcon('box', 16);
  return `<div role="button" tabindex="0" class="item-row" ${hosOn.attr('click', `openEditModal(${hosArg(item.id)})`)}>
    <span class="row-icon">${icon}</span>
    <span class="row-name">${esc(item.name)}</span>
    <span class="row-location">${esc(item.location || '-')}</span>
    <span class="row-qty">×${item.qty}</span>
    <div class="row-actions">
      <button class="row-act-btn" ${hosOn.attr('click', `event.stopPropagation();openEditModal(${hosArg(item.id)})`)}>Edit</button>
    </div>
  </div>`;
}

function esc(s) {
  return String(s).replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
}

// ── Modal ──
function openAddModal() {
  invEditingId = null;
  document.getElementById('inv-modal-title').textContent = 'Add Item';
  document.getElementById('f-name').value = '';
  document.getElementById('f-qty').value = '1';
  document.getElementById('f-category').value = currentFilter.startsWith('fleet:all') ? 'fleet:all' : currentFilter;
  document.getElementById('f-icon').value = '';
  document.getElementById('f-location').value = '';
  document.getElementById('f-notes').value = '';
  document.getElementById('modal-delete-btn').style.display = 'none';
  document.getElementById('item-modal-backdrop').classList.add('open');
  document.getElementById('f-name').focus();
}

function openEditModal(id) {
  const item = invItems.find(i => i.id === id);
  if (!item) return;
  invEditingId = id;
  document.getElementById('inv-modal-title').textContent = 'Edit Item';
  document.getElementById('f-name').value = item.name;
  document.getElementById('f-qty').value = item.qty;
  document.getElementById('f-category').value = item.category;
  document.getElementById('f-icon').value = item.icon || '';
  document.getElementById('f-location').value = item.location || '';
  document.getElementById('f-notes').value = item.notes || '';
  document.getElementById('modal-delete-btn').style.display = '';
  document.getElementById('item-modal-backdrop').classList.add('open');
  document.getElementById('f-name').focus();
}

function closeModal() {
  document.getElementById('item-modal-backdrop').classList.remove('open');
  invEditingId = null;
}

function closeModalOnBackdrop(e) {
  if (e.target === document.getElementById('item-modal-backdrop')) closeModal();
}

function saveItem() {
  const name = document.getElementById('f-name').value.trim();
  if (!name) { document.getElementById('f-name').focus(); return; }
  const data = {
    name,
    qty: Math.max(1, parseInt(document.getElementById('f-qty').value) || 1),
    category: document.getElementById('f-category').value,
    icon: document.getElementById('f-icon').value.trim() || hosIcon('box', 20),
    location: document.getElementById('f-location').value.trim(),
    notes: document.getElementById('f-notes').value.trim(),
  };
  if (invEditingId) {
    const idx = invItems.findIndex(i => i.id === invEditingId);
    if (idx >= 0) invItems[idx] = { ...invItems[idx], ...data };
  } else {
    invItems.push({ id: nextId(), ...data });
  }
  save();
  closeModal();
  updateCounts();
  renderItems();
}

async function deleteItem() {
  if (!invEditingId) return;
  if (!await holdConfirm('Delete this item?', { seconds: 3 })) return;
  invItems = invItems.filter(i => i.id !== invEditingId);
  save();
  closeModal();
  updateCounts();
  renderItems();
}

// ═════════════════════════════════════════════
// EQUIPMENT (all functions namespaced with equip_)
// ═════════════════════════════════════════════
const EQUIP_STORAGE_KEY = 'hos_equipment_v1';
let equip_items = [];
let equip_activeCat = 'loadout';
let equip_editingId = null;

const EQUIP_LOADOUT_SLOTS = [
  {id:'head',     icon:'🪖', label:'Head'},
  {id:'neck',     icon:'📿', label:'Neck'},
  {id:'chest',    icon:'👕', label:'Chest'},
  {id:'back',     icon:'🎒', label:'Back'},
  {id:'hands',    icon:'🧤', label:'Hands'},
  {id:'waist',    icon:'🔗', label:'Waist'},
  {id:'legs',     icon:'👖', label:'Legs'},
  {id:'feet',     icon:'👟', label:'Feet'},
  {id:'ring-l',   icon:'💍', label:'Ring (L)'},
  {id:'ring-r',   icon:'💍', label:'Ring (R)'},
  {id:'main-hand',icon:'⚔️', label:'Main Hand'},
  {id:'off-hand', icon:'🛡️', label:'Off Hand'},
];

const EQUIP_CAT_LABELS = {
  loadout:'Loadout', daily:'Daily Carry', tools:'Tools', tech:'Tech / Devices',
  power:'Power Tools', hand:'Hand Tools', safety:'Safety Gear', vehicles:'Vehicles'
};

function equip_load() {
  try { equip_items = JSON.parse(localStorage.getItem(EQUIP_STORAGE_KEY)) || []; }
  catch(e) { equip_items = []; }
}
function equip_save() { localStorage.setItem(EQUIP_STORAGE_KEY, JSON.stringify(equip_items)); }
function equip_uid() { return Date.now().toString(36) + Math.random().toString(36).slice(2,6); }
function equip_esc(s) { return (s||'').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;').replace(/'/g,'&#39;'); }

function equip_setCategory(cat) {
  equip_activeCat = cat;
  document.querySelectorAll('.equip-cat').forEach(el => el.classList.toggle('active', el.dataset.cat === cat));
  document.getElementById('equip-heading').textContent = EQUIP_CAT_LABELS[cat] || cat;
  equip_renderList();
}

function equip_updateCounts() {
  const cats = Object.keys(EQUIP_CAT_LABELS);
  cats.forEach(c => {
    const el = document.getElementById('cnt-' + c);
    if (el) el.textContent = equip_items.filter(i => i.cat === c).length;
  });
}

function equip_renderList() {
  const search = (document.getElementById('equip-search').value || '').toLowerCase();
  const list = document.getElementById('equip-list');
  equip_updateCounts();

  let visible = equip_items.filter(i => i.cat === equip_activeCat);
  if (search) visible = visible.filter(i =>
    (i.name||'').toLowerCase().includes(search) ||
    (i.notes||'').toLowerCase().includes(search)
  );

  if (equip_activeCat === 'loadout') {
    // Show body-slot grid
    const slotMap = {};
    visible.forEach(i => { if (i.slot) slotMap[i.slot] = i; });
    list.innerHTML = '<div class="loadout-grid">' +
      EQUIP_LOADOUT_SLOTS.map(slot => {
        const item = slotMap[slot.id];
        return `<div class="loadout-slot${item ? '' : ' empty'}" ${item ? `${hosOn.attr('click', `equip_openModal(${hosArg(item.id)})`)}` : `${hosOn.attr('click', `equip_openModalSlot(${hosArg(slot.id)})`)}`}>
          <div class="slot-icon">${item ? equip_esc(item.icon || slot.icon) : slot.icon}</div>
          <div class="slot-label">${slot.label}</div>
          <div class="slot-item">${item ? equip_esc(item.name) : 'Empty'}</div>
          ${item ? `<div class="item-cond cond-${item.cond}">${item.cond}</div>` : ''}
        </div>`;
      }).join('') +
    '</div>' +
    (visible.filter(i => !i.slot).length ? `
      <div class="list-section-header" style="margin-top:var(--space-xl);">Unslotted Items</div>
      <div class="equip-item-list">` + visible.filter(i=>!i.slot).map(i => equip_renderItem(i)).join('') + '</div>' : '');
    return;
  }

  if (!visible.length) {
    list.innerHTML = `<div class="equip-empty-state"><div class="ei">${equip_getCatIcon(equip_activeCat)}</div>
      <p>No ${EQUIP_CAT_LABELS[equip_activeCat]} items yet.</p>
      <button class="equip-btn-save" ${hosOn.attr('click', `equip_openModal()`)} style="margin-top:var(--space-md);">+ Add First Item</button></div>`;
    return;
  }

  list.innerHTML = '<div class="equip-item-list">' + visible.map(i => equip_renderItem(i)).join('') + '</div>';
}

function equip_renderItem(i) {
  const condClass = 'cond-' + (i.cond || 'good');
  return `<div role="button" tabindex="0" class="equip-item" ${hosOn.attr('click', `equip_openModal(${hosArg(i.id)})`)}>
    <div class="item-icon">${i.icon || hosIcon('box', 20)}</div>
    <div class="item-body">
      <div class="item-name">${equip_esc(i.name)}</div>
      ${i.notes ? `<div class="item-sub">${equip_esc(i.notes.slice(0,80))}</div>` : ''}
    </div>
    <span class="item-cond ${condClass}">${i.cond || 'good'}</span>
    <button class="equip-item-delete" ${hosOn.attr('click', `event.stopPropagation();equip_deleteItem(${hosArg(i.id)})`)} title="Remove">✕</button>
  </div>`;
}

function equip_getCatIcon(cat) {
  return {loadout:'🧍',daily:'🎒',tools:'🔧',tech:'💻',power:'⚡',hand:'🪛',safety:'🦺',vehicles:'🚗'}[cat] || hosIcon('box', 16);
}

function equip_openModal(id) {
  equip_editingId = id || null;
  const item = id ? equip_items.find(x => x.id === id) : null;
  document.getElementById('equip-modal-title').textContent = item ? 'Edit Item' : 'Add Equipment';
  document.getElementById('eq-name').value  = item ? item.name : '';
  document.getElementById('eq-cat').value   = item ? item.cat  : equip_activeCat;
  document.getElementById('eq-cond').value  = item ? item.cond : 'good';
  document.getElementById('eq-icon').value  = item ? (item.icon || '') : '';
  document.getElementById('eq-slot').value  = item ? (item.slot || '') : '';
  document.getElementById('eq-notes').value = item ? (item.notes || '') : '';
  document.getElementById('equip-modal').classList.add('open');
  setTimeout(() => document.getElementById('eq-name').focus(), 50);
}

function equip_openModalSlot(slotId) {
  equip_openModal();
  document.getElementById('eq-slot').value = slotId;
  document.getElementById('eq-cat').value = 'loadout';
}

function equip_closeModal() {
  document.getElementById('equip-modal').classList.remove('open');
  equip_editingId = null;
}

function equip_saveItem() {
  const name = document.getElementById('eq-name').value.trim();
  if (!name) { document.getElementById('eq-name').focus(); return; }
  if (equip_editingId) {
    const i = equip_items.find(x => x.id === equip_editingId);
    if (i) {
      i.name = name;
      i.cat  = document.getElementById('eq-cat').value;
      i.cond = document.getElementById('eq-cond').value;
      i.icon = document.getElementById('eq-icon').value.trim();
      i.slot = document.getElementById('eq-slot').value || null;
      i.notes = document.getElementById('eq-notes').value.trim();
    }
  } else {
    equip_items.push({
      id: equip_uid(), name, icon: document.getElementById('eq-icon').value.trim(),
      cat:  document.getElementById('eq-cat').value,
      cond: document.getElementById('eq-cond').value,
      slot: document.getElementById('eq-slot').value || null,
      notes: document.getElementById('eq-notes').value.trim()
    });
  }
  equip_save(); equip_closeModal(); equip_renderList();
}

function equip_deleteItem(id) {
  equip_items = equip_items.filter(x => x.id !== id);
  equip_save(); equip_renderList();
}

document.getElementById('equip-modal').addEventListener('click', function(e) { if (e.target === this) equip_closeModal(); });

// ═════════════════════════════════════════════
// SHARED KEYBOARD SHORTCUTS
// ═════════════════════════════════════════════
document.addEventListener('keydown', e => {
  if (e.key === 'Escape') {
    closeModal();
    equip_closeModal();
  }
  if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
    if (activeGearTab === 'inventory') saveItem();
    else equip_saveItem();
  }
  if ((e.ctrlKey || e.metaKey) && e.key === 'n' &&
      !document.getElementById('item-modal-backdrop').classList.contains('open') &&
      !document.getElementById('equip-modal').classList.contains('open')) {
    e.preventDefault();
    if (activeGearTab === 'inventory') openAddModal();
    else equip_openModal();
  }
});

// ═════════════════════════════════════════════
// INIT
// ═════════════════════════════════════════════
load();
updateCounts();
renderItems();

equip_load();
equip_renderList();

// Restore last inventory filter
const lastFilter = localStorage.getItem('hos_inv_filter');
if (lastFilter) {
  const el = document.querySelector(`[data-filter="${lastFilter}"]`);
  if (el) setFilter(lastFilter, el);
}

// Restore last active gear tab
const savedGearTab = localStorage.getItem('hos_gear_active_tab');
if (savedGearTab === 'equipment') switchGearTab('equipment');
