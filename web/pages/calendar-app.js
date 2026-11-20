// Moved out of web/pages/calendar.html (2026-11-20) so the site's Content-Security-Policy can refuse
// inline script (docs/PRIORITIES.md item 6b). It runs at the same place in the page as before.
// ─────────────────────────────────────────────
// Data
// ─────────────────────────────────────────────
const STORAGE_KEY = 'hos_calendar_v1';
let events = [];
let editingId = null;
let selectedColor = '#3498db';

const EVENT_COLORS = ['#e74c3c','#e67e22','#f1c40f','#2ecc71','#1abc9c','#3498db','#9b59b6','#e91e8c','#FF8811','#aaa'];

function load() {
  try { events = JSON.parse(localStorage.getItem(STORAGE_KEY)) || []; } catch(e) { events = []; }
}
function save() { localStorage.setItem(STORAGE_KEY, JSON.stringify(events)); }
function nextId() { return Date.now().toString(36) + Math.random().toString(36).slice(2,6); }

function eventMatchesDate(ev, dateStr) {
  if (!ev.recurrence) return false;
  const evDate = new Date(ev.date + 'T00:00:00');
  const check  = new Date(dateStr + 'T00:00:00');
  if (evDate > check) return false; // haven't reached start date yet
  if (ev.date === dateStr) return false; // already included as the base event
  switch (ev.recurrence) {
    case 'daily':   return true;
    case 'weekly':  return evDate.getDay() === check.getDay();
    case 'monthly': return evDate.getDate() === check.getDate();
    case 'yearly':  return evDate.getMonth() === check.getMonth() && evDate.getDate() === check.getDate();
    default: return false;
  }
}

function eventsOn(dateStr) {
  const direct   = events.filter(e => e.date === dateStr);
  const expanded = events
    .filter(e => e.recurrence && eventMatchesDate(e, dateStr))
    .map(e => ({ ...e, _recurring: true }));
  // Include quests with due dates on this day.
  const questsOnDay = [];
  try {
    const qs = JSON.parse(localStorage.getItem('hos_quests_v1') || '[]');
    for (const q of qs) {
      if (q.due === dateStr && !q.done) {
        questsOnDay.push({ id: 'quest:'+q.id, title: '⚔️ '+q.title, date: dateStr, color: '#7f5af0', _quest: true });
      }
    }
  } catch {}
  return [...direct, ...expanded, ...questsOnDay];
}

// ─────────────────────────────────────────────
// State
// ─────────────────────────────────────────────
const today = new Date();
let cursor = new Date(today);
let view = 'month';

// Day and month names in the person's language (the same keys as the desktop
// app's calendar): dayName(0) is Sunday, monthName(0) is January.
function dayName(i) { return i18n.t(`calendar.weekday_short_${i}`); }
function monthName(i) { return i18n.t(`calendar.month_${i + 1}`); }
function hourLabel(h) {
  if (h === 0) return '';
  if (h < 12) return i18n.t('calendar.hour_am', { h: h });
  return i18n.t('calendar.hour_pm', { h: h === 12 ? 12 : h - 12 });
}
// The words of a hover hint for a control this script makes, escaped for a
// data-tip-desc attribute (the key goes beside it as data-i18n-tip).
function hintWords(key) {
  return esc(i18n.t(key));
}

function fmtDate(d) {
  return `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}-${String(d.getDate()).padStart(2,'0')}`;
}
function todayStr() { return fmtDate(today); }

// ─────────────────────────────────────────────
// Navigation
// ─────────────────────────────────────────────
function navigate(dir) {
  if (view === 'month') cursor.setMonth(cursor.getMonth() + dir);
  else if (view === 'week') cursor.setDate(cursor.getDate() + dir * 7);
  else cursor.setDate(cursor.getDate() + dir);
  render();
}

function goToday() {
  cursor = new Date(today);
  render();
}

function setView(v) {
  view = v;
  ['month','week','day'].forEach(x => {
    document.getElementById(x + '-view').style.display = x === v ? 'flex' : 'none';
    document.getElementById('tab-' + x).classList.toggle('active', x === v);
  });
  render();
}

// ─────────────────────────────────────────────
// Render dispatch
// ─────────────────────────────────────────────
function render() {
  updateTitle();
  if (view === 'month') renderMonth();
  else if (view === 'week') renderWeek();
  else renderDay();
  renderTodaySidebar();
}

function renderTodaySidebar() {
  const td = todayStr();
  const evs = eventsOn(td);
  const dateLabel = document.getElementById('today-date-label');
  const list = document.getElementById('today-events-list');
  dateLabel.textContent = today.toLocaleDateString(i18n.getLanguage(), { month: 'short', day: 'numeric' });

  if (!evs.length) {
    list.innerHTML = '<div class="today-ev-empty">' + esc(i18n.t('calendar.no_events_today')) + '</div>';
    return;
  }

  // Sort: timed events first by time, then all-day
  evs.sort((a, b) => {
    if (a.startTime && !b.startTime) return -1;
    if (!a.startTime && b.startTime) return 1;
    if (a.startTime && b.startTime) return a.startTime.localeCompare(b.startTime);
    return 0;
  });

  const openHint = hintWords('calendar.event_open_hint');
  list.innerHTML = evs.map(e => {
    const timeStr = e.allday || !e.startTime ? esc(i18n.t('calendar.all_day')) :
      e.startTime + (e.endTime ? ' - ' + e.endTime : '');
    return `<div role="button" tabindex="0" class="today-ev-item" data-on-click="openEditEvent(${hosArg(e.id)})" title="${esc(e.notes || '')}" data-i18n-tip="calendar.event_open_hint" data-tip-desc="${openHint}">
      <div class="today-ev-dot" style="background:${e.color || '#3498db'}"></div>
      <div class="today-ev-info">
        <div class="today-ev-title">${esc(e.title)}</div>
        <div class="today-ev-time">${timeStr}${e._recurring ? ' ' + esc(i18n.t('calendar.recurring_note')) : ''}</div>
      </div>
    </div>`;
  }).join('');
}

function updateTitle() {
  let title;
  if (view === 'month') {
    title = i18n.t('calendar.month_year', { month: monthName(cursor.getMonth()), year: cursor.getFullYear() });
  } else if (view === 'week') {
    const ws = weekStart(cursor);
    const we = new Date(ws); we.setDate(we.getDate() + 6);
    const range = { month: monthName(ws.getMonth()), start: ws.getDate(), end: we.getDate(), year: we.getFullYear() };
    if (ws.getMonth() !== we.getMonth()) {
      range.end_month = monthName(we.getMonth());
      title = i18n.t('calendar.week_range_cross', range);
    } else {
      title = i18n.t('calendar.week_range_same', range);
    }
  } else {
    title = i18n.t('calendar.day_title', {
      weekday: dayName(cursor.getDay()), month: monthName(cursor.getMonth()),
      day: cursor.getDate(), year: cursor.getFullYear(),
    });
  }
  document.getElementById('cal-title').textContent = title;
}

// ─────────────────────────────────────────────
// Month view
// ─────────────────────────────────────────────
function renderMonth() {
  const el = document.getElementById('month-view');
  const year = cursor.getFullYear(), month = cursor.getMonth();
  const first = new Date(year, month, 1);
  const last  = new Date(year, month+1, 0);
  const startDow = first.getDay();
  const td = todayStr();

  let html = `<div class="weekday-header">${[0,1,2,3,4,5,6].map(d=>`<div>${esc(dayName(d))}</div>`).join('')}</div>
    <div class="month-grid">`;

  // leading days from prev month
  const prevLast = new Date(year, month, 0);
  for (let i = startDow - 1; i >= 0; i--) {
    const d = new Date(year, month-1, prevLast.getDate() - i);
    html += dayCell(d, true, td);
  }
  // current month
  for (let d = 1; d <= last.getDate(); d++) {
    html += dayCell(new Date(year, month, d), false, td);
  }
  // trailing days
  const trail = 42 - (startDow + last.getDate());
  for (let d = 1; d <= trail; d++) {
    html += dayCell(new Date(year, month+1, d), true, td);
  }

  el.innerHTML = html + '</div>';
}

function dayCell(d, otherMonth, todayStr) {
  const ds = fmtDate(d);
  const evs = eventsOn(ds);
  const isToday = ds === todayStr;
  const maxShow = 3;

  const openHint = hintWords('calendar.event_open_hint');
  const recurringTitle = '↻ ' + esc(i18n.t('calendar.recurring'));
  let chips = evs.slice(0, maxShow).map(e =>
    `<div role="button" tabindex="0" class="event-chip" style="background:${e.color||'#3498db'}" data-on-click="event.stopPropagation();openEditEvent(${hosArg(e.id)})" title="${e._recurring ? recurringTitle : ''}" data-i18n-tip="calendar.event_open_hint" data-tip-desc="${openHint}">${e._recurring ? '↻ ' : ''}${esc(e.title)}</div>`
  ).join('');
  if (evs.length > maxShow) chips += `<div class="more-chip">${esc(i18n.t('calendar.more', { n: evs.length - maxShow }))}</div>`;

  return `<div role="button" tabindex="0" class="cal-day${otherMonth?' other-month':''}${isToday?' today':''}" data-on-click="dayClick(${hosArg(ds)})" data-i18n-tip="calendar.day_add_hint" data-tip-desc="${hintWords('calendar.day_add_hint')}">
    <div class="day-num">${d.getDate()}</div>
    <div class="day-events">${chips}</div>
  </div>`;
}

// ─────────────────────────────────────────────
// Week view
// ─────────────────────────────────────────────
function weekStart(d) {
  const s = new Date(d);
  s.setDate(s.getDate() - s.getDay());
  return s;
}

function renderWeek() {
  const el = document.getElementById('week-view');
  const ws = weekStart(cursor);
  const td = todayStr();
  const days = Array.from({length:7}, (_,i) => { const x=new Date(ws); x.setDate(ws.getDate()+i); return x; });

  let header = `<div class="week-header"><div style="border-right:1px solid var(--bg-secondary)"></div>`;
  days.forEach(d => {
    const isToday = fmtDate(d) === td;
    header += `<div class="week-header-cell${isToday?' today-col':''}">
      <div class="wday">${esc(dayName(d.getDay()))}</div>
      <div class="wdate">${d.getDate()}</div>
    </div>`;
  });
  header += '</div>';

  // Time labels
  let timeCol = '';
  for (let h = 0; h < 24; h++) {
    timeCol += `<div class="week-time-label">${esc(hourLabel(h))}</div>`;
  }

  // Day columns with events
  const hourHint = hintWords('calendar.hour_add_hint');
  const openHint = hintWords('calendar.event_open_hint');
  let daysCols = days.map((d, di) => {
    const ds = fmtDate(d);
    const dayEvs = eventsOn(ds).filter(e => !e.allday && e.startTime);
    let cells = '';
    for (let h = 0; h < 24; h++) {
      cells += `<div role="button" tabindex="0" aria-label="${hourHint}" class="week-hour-cell" data-on-click="dayHourClick(${hosArg(ds)},${hosNum(h)})" data-i18n-tip="calendar.hour_add_hint" data-tip-desc="${hourHint}"></div>`;
    }
    let eventMarkup = dayEvs.map(e => {
      const [sh,sm] = e.startTime.split(':').map(Number);
      const [eh,em] = (e.endTime||e.startTime).split(':').map(Number);
      const top = (sh*60+sm) * 48/60;
      const height = Math.max(20, ((eh*60+em)-(sh*60+sm)) * 48/60);
      return `<div role="button" tabindex="0" class="week-event" style="top:${top}px;height:${height}px;background:${e.color||'#3498db'}" data-on-click="event.stopPropagation();openEditEvent(${hosArg(e.id)})" data-i18n-tip="calendar.event_open_hint" data-tip-desc="${openHint}">${esc(e.title)}</div>`;
    }).join('');
    return `<div class="week-day-col" style="position:relative">${cells}${eventMarkup}</div>`;
  }).join('');

  el.innerHTML = header + `<div class="week-grid-wrap">
    <div class="week-time-col">${timeCol}</div>
    <div class="week-days-grid">${daysCols}</div>
  </div>`;

  positionNowLine('week');
}

// ─────────────────────────────────────────────
// Day view
// ─────────────────────────────────────────────
function renderDay() {
  const el = document.getElementById('day-view');
  const ds = fmtDate(cursor);
  const dayEvs = eventsOn(ds).filter(e => !e.allday && e.startTime);

  const hourHint = hintWords('calendar.hour_add_hint');
  const openHint = hintWords('calendar.event_open_hint');
  let timeLabels = '', hourCells = '';
  for (let h = 0; h < 24; h++) {
    timeLabels += `<div class="day-hour-label">${esc(hourLabel(h))}</div>`;
    hourCells   += `<div role="button" tabindex="0" aria-label="${hourHint}" class="day-hour-cell" data-on-click="dayHourClick(${hosArg(ds)},${hosNum(h)})" data-i18n-tip="calendar.hour_add_hint" data-tip-desc="${hourHint}"></div>`;
  }

  const eventMarkup = dayEvs.map(e => {
    const [sh,sm] = e.startTime.split(':').map(Number);
    const [eh,em] = (e.endTime||e.startTime).split(':').map(Number);
    const top = (sh*60+sm) * 56/60;
    const height = Math.max(24, ((eh*60+em)-(sh*60+sm)) * 56/60);
    return `<div role="button" tabindex="0" class="day-event" style="top:${top}px;height:${height}px;background:${e.color||'#3498db'}" data-on-click="event.stopPropagation();openEditEvent(${hosArg(e.id)})" data-i18n-tip="calendar.event_open_hint" data-tip-desc="${openHint}">${esc(e.title)}<br><span style="opacity:.7;font-size:.65rem">${e.startTime}-${e.endTime||''}</span></div>`;
  }).join('');

  const allDay = eventsOn(ds).filter(e => e.allday);
  const allDayBar = allDay.length ? `<div style="padding:.4rem .8rem;border-bottom:1px solid var(--bg-secondary);display:flex;gap:.4rem;flex-wrap:wrap">
    ${allDay.map(e=>`<span role="button" tabindex="0" class="event-chip" style="background:${e.color||'#3498db'};cursor:pointer" data-on-click="openEditEvent(${hosArg(e.id)})" data-i18n-tip="calendar.event_open_hint" data-tip-desc="${openHint}">${esc(e.title)}</span>`).join('')}
  </div>` : '';

  const dayHeader = i18n.t('calendar.day_header', {
    weekday: dayName(cursor.getDay()), month: monthName(cursor.getMonth()), day: cursor.getDate(),
  });
  el.innerHTML = `<div class="day-header">${esc(dayHeader)}</div>
    ${allDayBar}
    <div class="day-grid-wrap">
      <div class="day-time-col">${timeLabels}</div>
      <div class="day-events-col" style="position:relative">${hourCells}${eventMarkup}</div>
    </div>`;

  positionNowLine('day');
}

// ─────────────────────────────────────────────
// Now line
// ─────────────────────────────────────────────
function positionNowLine(v) {
  const now = new Date();
  if (v === 'day') {
    const ds = fmtDate(cursor);
    if (ds !== fmtDate(now)) return;
    const col = document.querySelector('.day-events-col');
    if (!col) return;
    const mins = now.getHours() * 60 + now.getMinutes();
    const top = mins * 56 / 60;
    col.insertAdjacentHTML('beforeend', `<div class="now-line" style="top:${top}px"><div class="now-dot"></div></div>`);
  }
  if (v === 'week') {
    const ws = weekStart(cursor);
    const dow = now.getDay();
    const cols = document.querySelectorAll('.week-day-col');
    const col = cols[dow];
    if (!col) return;
    const ds = fmtDate(now);
    if (fmtDate(ws) > fmtDate(now) || fmtDate(new Date(ws.getTime()+6*86400000)) < ds) return;
    const mins = now.getHours() * 60 + now.getMinutes();
    const top = mins * 48 / 60;
    col.insertAdjacentHTML('beforeend', `<div class="now-line" style="top:${top}px"><div class="now-dot"></div></div>`);
  }
}

// ─────────────────────────────────────────────
// Interactions
// ─────────────────────────────────────────────
function dayClick(ds) {
  openAddEvent(ds);
}

function dayHourClick(ds, hour) {
  const pad = n => String(n).padStart(2,'0');
  openAddEvent(ds, `${pad(hour)}:00`, `${pad(hour+1<24?hour+1:23)}:${hour+1<24?'00':'59'}`);
}

// ─────────────────────────────────────────────
// Modal
// ─────────────────────────────────────────────
function buildColorPicker() {
  const swatchHint = hintWords('calendar.color_swatch_hint');
  document.getElementById('color-picker').innerHTML = EVENT_COLORS.map(c =>
    `<div role="button" tabindex="0" aria-label="${swatchHint}" class="color-swatch${c===selectedColor?' selected':''}" style="background:${c}" data-on-click="pickColor(${hosArg(c)})" data-i18n-tip="calendar.color_swatch_hint" data-tip-desc="${swatchHint}"></div>` // i18n-exempt: a CSS class name, not words
  ).join('');
}

function pickColor(c) {
  selectedColor = c;
  document.querySelectorAll('.color-swatch').forEach(el => el.classList.toggle('selected', el.style.background === c));
}

function toggleTime() {
  const allday = document.getElementById('ef-allday').value === '1';
  document.getElementById('ef-time-row').style.display = allday ? 'none' : 'flex';
}

function openAddEvent(dateStr, startTime, endTime) {
  editingId = null;
  document.getElementById('ev-modal-title').textContent = i18n.t('calendar.new_event');
  document.getElementById('ef-title').value = '';
  document.getElementById('ef-date').value = dateStr || fmtDate(cursor);
  document.getElementById('ef-allday').value = '1';
  document.getElementById('ef-time-row').style.display = 'none';
  document.getElementById('ef-start').value = startTime || '09:00';
  document.getElementById('ef-end').value = endTime || '10:00';
  document.getElementById('ef-notes').value = '';
  document.getElementById('ef-recur').value = '';
  document.getElementById('ev-delete-btn').style.display = 'none';
  selectedColor = '#3498db';
  buildColorPicker();
  document.getElementById('ev-backdrop').classList.add('open');
  setTimeout(() => document.getElementById('ef-title').focus(), 50);
}

function openEditEvent(id) {
  const ev = events.find(e => e.id === id);
  if (!ev) return;
  editingId = id;
  document.getElementById('ev-modal-title').textContent = i18n.t('calendar.edit_event');
  document.getElementById('ef-title').value = ev.title;
  document.getElementById('ef-date').value = ev.date;
  document.getElementById('ef-allday').value = ev.allday ? '1' : '0';
  document.getElementById('ef-start').value = ev.startTime || '09:00';
  document.getElementById('ef-end').value = ev.endTime || '10:00';
  document.getElementById('ef-notes').value = ev.notes || '';
  document.getElementById('ef-recur').value = ev.recurrence || '';
  document.getElementById('ev-delete-btn').style.display = '';
  document.getElementById('ef-time-row').style.display = ev.allday ? 'none' : 'flex';
  selectedColor = ev.color || '#3498db';
  buildColorPicker();
  document.getElementById('ev-backdrop').classList.add('open');
}

function closeModal() {
  document.getElementById('ev-backdrop').classList.remove('open');
  editingId = null;
}

function closeBdrop(e) {
  if (e.target === document.getElementById('ev-backdrop')) closeModal();
}

function saveEvent() {
  const title = document.getElementById('ef-title').value.trim();
  if (!title) { document.getElementById('ef-title').focus(); return; }
  const allday = document.getElementById('ef-allday').value === '1';
  const data = {
    title,
    date: document.getElementById('ef-date').value,
    allday,
    startTime: allday ? null : document.getElementById('ef-start').value,
    endTime: allday ? null : document.getElementById('ef-end').value,
    color: selectedColor,
    notes: document.getElementById('ef-notes').value.trim(),
    recurrence: document.getElementById('ef-recur').value || null,
  };
  if (editingId) {
    const idx = events.findIndex(e => e.id === editingId);
    if (idx >= 0) events[idx] = { ...events[idx], ...data };
  } else {
    events.push({ id: nextId(), ...data });
  }
  save();
  closeModal();
  render();
}

async function deleteEvent() {
  if (!editingId) return;
  if (!await holdConfirm(i18n.t('calendar.delete_confirm'), { seconds: 3 })) return;
  events = events.filter(e => e.id !== editingId);
  save();
  closeModal();
  render();
}

function esc(s) {
  return String(s||'').replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;').replace(/"/g,'&quot;');
}

// ─────────────────────────────────────────────
// Keyboard shortcuts
// ─────────────────────────────────────────────
document.addEventListener('keydown', e => {
  if (e.key === 'Escape') closeModal();
  if ((e.ctrlKey||e.metaKey) && e.key === 'Enter') saveEvent();
  if (!document.getElementById('ev-backdrop').classList.contains('open')) {
    if (e.key === 'ArrowLeft') navigate(-1);
    if (e.key === 'ArrowRight') navigate(1);
    if (e.key === 't') goToday();
    if (e.key === 'm') setView('month');
    if (e.key === 'w') setView('week');
    if (e.key === 'd') setView('day');
  }
});

// ─────────────────────────────────────────────
// Init
// ─────────────────────────────────────────────
load();
buildColorPicker();

const savedView = localStorage.getItem('hos_cal_view') || 'month';
// init month-view flex
document.getElementById('month-view').style.display = 'flex';
setView(savedView);

// Save view preference
const origSetView = setView;
window.setView = function(v) {
  origSetView(v);
  localStorage.setItem('hos_cal_view', v);
};
