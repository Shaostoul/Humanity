// ══════════════════════════════════════════════
// HumanityOS Calculator
// ══════════════════════════════════════════════

const CALC_HISTORY_KEY = 'hos_calc_history_v1';
let expression = '';
let lastResult = '0';
let history = [];
let currentMode = 'basic';

// ── Mode switching ──

function setMode(mode) {
  currentMode = mode;
  document.querySelectorAll('.calc-tab').forEach(tab =>
    tab.classList.toggle('active', tab.dataset.mode === mode)
  );
  document.querySelectorAll('.mode-panel').forEach(p =>
    p.classList.toggle('active', p.id === 'panel-' + mode)
  );
  // Show history only for basic/scientific
  document.getElementById('calc-history').style.display =
    (mode === 'basic' || mode === 'scientific') ? '' : 'none';
}

// ── Display helpers ──

function getExprEl() {
  return currentMode === 'scientific'
    ? document.getElementById('sci-expr')
    : document.getElementById('basic-expr');
}

function getResultEl() {
  return currentMode === 'scientific'
    ? document.getElementById('sci-result')
    : document.getElementById('basic-result');
}

function updateDisplay() {
  getExprEl().textContent = expression;
  getResultEl().textContent = lastResult;
}

// ── Input ──

function inputNum(n) {
  expression += n;
  updateDisplay();
}

function inputOp(op) {
  expression += op;
  updateDisplay();
}

function inputFn(fn) {
  expression += fn;
  updateDisplay();
}

function backspace() {
  expression = expression.slice(0, -1);
  updateDisplay();
}

function clearCalc() {
  expression = '';
  lastResult = '0';
  updateDisplay();
}

// ── Evaluation ──

function factorial(n) {
  n = Math.round(n);
  if (n < 0) return NaN;
  if (n === 0 || n === 1) return 1;
  if (n > 170) return Infinity;
  let result = 1;
  for (let i = 2; i <= n; i++) result *= i;
  return result;
}

// Angles are in DEGREES, as on the desktop calculator (src/gui/pages/calculator.rs takes
// to_radians() before sin, cos and tan); the website worked in radians until 2026-10-24, so the
// same keys gave different answers on the two (sin 30 was -0.988 here and 0.5 there).
const DEG = Math.PI / 180;
function sinDeg(x) { return Math.sin(x * DEG); }
function cosDeg(x) { return Math.cos(x * DEG); }
function tanDeg(x) { return Math.tan(x * DEG); }

// The calculator's own reader of what was typed (2026-11-20). It replaced a `new Function` that the
// site's Content-Security-Policy has refused all along (it never allowed 'unsafe-eval'), so on the
// live site every sum answered Error; a string is never run as code here. It reads numbers, pi,
// + - * /, ^ (a power, right to left: 2^3^2 is 2^9; -2^2 is -4), % after a value (a hundredth of
// it: 50% is 0.5, 200*10% is 20), brackets, a value written against a bracket, a name or a number
// (2pi, 3(4+1), (2)3: times), and sin cos tan (in degrees), log, ln, sqrt, pow(a, b), fact.
// Anything else is NaN, which the page shows as its error word.
function safeEval(expr) {
  const src = String(expr);
  let i = 0;
  const FNS = {
    sin: sinDeg, cos: cosDeg, tan: tanDeg, log: Math.log10, ln: Math.log,
    sqrt: Math.sqrt, pow: Math.pow, fact: factorial,
  };
  const fail = () => { throw new SyntaxError('calc'); };
  const peek = () => { while (i < src.length && /\s/.test(src[i])) i++; return src[i] || ''; };
  // sum := term (('+' | '-') term)*
  function sum() {
    let v = term();
    for (;;) {
      const c = peek();
      if (c === '+') { i++; v += term(); }
      else if (c === '-') { i++; v -= term(); }
      else return v;
    }
  }
  // term := unary (('*' | '/') unary | unary written straight after)*
  function term() {
    let v = unary();
    for (;;) {
      const c = peek();
      if (c === '*') { i++; v *= unary(); }
      else if (c === '/') { i++; v /= unary(); }
      else if (c === '(' || /[a-z0-9.]/i.test(c)) v *= unary();
      else return v;
    }
  }
  // unary := ('-' | '+') unary | power
  function unary() {
    const c = peek();
    if (c === '-') { i++; return -unary(); }
    if (c === '+') { i++; return unary(); }
    return power();
  }
  // power := percent ('^' unary)?
  function power() {
    const base = percent();
    if (peek() === '^') { i++; return Math.pow(base, unary()); }
    return base;
  }
  // percent := primary '%'*
  function percent() {
    let v = primary();
    while (peek() === '%') { i++; v /= 100; }
    return v;
  }
  // primary := number | 'pi' | name '(' sum (',' sum)* ')' | '(' sum ')'
  function primary() {
    const c = peek();
    if (c === '(') {
      i++;
      const v = sum();
      if (peek() !== ')') fail();
      i++;
      return v;
    }
    const num = /^(\d+\.?\d*|\.\d+)(e[+-]?\d+)?/i.exec(src.slice(i));
    if (num) { i += num[0].length; return parseFloat(num[0]); }
    const word = /^[a-z]+/i.exec(src.slice(i));
    if (!word) return fail();
    const name = word[0].toLowerCase();
    i += word[0].length;
    if (name === 'pi') return Math.PI;
    const fn = Object.prototype.hasOwnProperty.call(FNS, name) ? FNS[name] : null;
    if (!fn || peek() !== '(') return fail();
    i++;
    const args = [];
    if (peek() !== ')') {
      args.push(sum());
      while (peek() === ',') { i++; args.push(sum()); }
    }
    if (peek() !== ')') fail();
    i++;
    return fn(...args);
  }
  try {
    const v = sum();
    return peek() === '' ? v : NaN;
  } catch (err) {
    return NaN;
  }
}

function evaluate() {
  if (!expression.trim()) return;
  const result = safeEval(expression);
  const resultStr = isNaN(result) ? i18n.t('calculator.error') : formatNumber(result);

  // Add to history
  if (!isNaN(result)) {
    addHistory(expression, resultStr);
  }

  getExprEl().textContent = expression + ' =';
  lastResult = resultStr;
  getResultEl().textContent = resultStr;
  expression = isNaN(result) ? '' : String(result);
}

function formatNumber(n) {
  if (Number.isInteger(n) && Math.abs(n) < 1e15) return n.toString();
  if (Math.abs(n) < 1e-10 && n !== 0) return n.toExponential(6);
  if (Math.abs(n) >= 1e15) return n.toExponential(6);
  // Round to avoid floating point display issues
  const s = parseFloat(n.toPrecision(12));
  return s.toString();
}

// ── Copy ──

function copyResult() {
  const text = getResultEl().textContent;
  navigator.clipboard.writeText(text).then(() => {
    const btn = document.querySelector('.mode-panel.active .calc-copy');
    if (btn) {
      const orig = btn.textContent;
      btn.textContent = i18n.t('calculator.copied');
      setTimeout(() => { btn.textContent = orig; }, 1200);
    }
  }).catch(() => {});
}

// ── History ──

function loadHistory() {
  try { history = JSON.parse(localStorage.getItem(CALC_HISTORY_KEY)) || []; }
  catch (e) { history = []; }
}

function saveHistory() {
  localStorage.setItem(CALC_HISTORY_KEY, JSON.stringify(history));
}

function addHistory(expr, result) {
  history.unshift({ expr, result, time: Date.now() });
  if (history.length > 20) history = history.slice(0, 20);
  saveHistory();
  renderHistory();
}

function clearHistory() {
  history = [];
  saveHistory();
  renderHistory();
}

function renderHistory() {
  const list = document.getElementById('hist-list');
  if (!history.length) {
    list.innerHTML = '<div class="hist-empty">' + esc(i18n.t('calculator.no_history')) + '</div>';
    return;
  }
  const reuseTitle = esc(i18n.t('calculator.history_reuse_title'));
  const reuseHint = esc(i18n.t('calculator.history_reuse_hint'));
  list.innerHTML = history.map((h, i) =>
    `<div role="button" tabindex="0" class="hist-item" data-on-click="useHistory(${hosNum(i)})" title="${reuseTitle}" data-i18n-tip="calculator.history_reuse_hint" data-tip-desc="${reuseHint}">
      <div class="hist-expr">${esc(h.expr)}</div>
      <div class="hist-result">= ${esc(h.result)}</div>
    </div>`
  ).join('');
}

function useHistory(idx) {
  const h = history[idx];
  if (!h) return;
  expression = h.expr;
  lastResult = h.result;
  updateDisplay();
}

// ── Unit Converter ──

// The unit names are words, by key from data/i18n/ (the symbols m, km, ft ...
// are the same in every language and stay as they are).
const UNITS = {
  length: {
    label: i18n.t('calculator.cat_length'),
    units: {
      m: { name: i18n.t('calculator.unit_meters'), toBase: 1 },
      km: { name: i18n.t('calculator.unit_kilometers'), toBase: 1000 },
      cm: { name: i18n.t('calculator.unit_centimeters'), toBase: 0.01 },
      mm: { name: i18n.t('calculator.unit_millimeters'), toBase: 0.001 },
      ft: { name: i18n.t('calculator.unit_feet'), toBase: 0.3048 },
      in: { name: i18n.t('calculator.unit_inches'), toBase: 0.0254 },
      mi: { name: i18n.t('calculator.unit_miles'), toBase: 1609.344 },
      yd: { name: i18n.t('calculator.unit_yards'), toBase: 0.9144 },
    }
  },
  weight: {
    label: i18n.t('calculator.cat_weight'),
    units: {
      kg: { name: i18n.t('calculator.unit_kilograms'), toBase: 1 },
      g: { name: i18n.t('calculator.unit_grams'), toBase: 0.001 },
      mg: { name: i18n.t('calculator.unit_milligrams'), toBase: 0.000001 },
      lb: { name: i18n.t('calculator.unit_pounds'), toBase: 0.453592 },
      oz: { name: i18n.t('calculator.unit_ounces'), toBase: 0.0283495 },
      t: { name: i18n.t('calculator.unit_metric_tons'), toBase: 1000 },
    }
  },
  temperature: {
    label: i18n.t('calculator.cat_temperature'),
    units: {
      C: { name: i18n.t('calculator.unit_celsius') },
      F: { name: i18n.t('calculator.unit_fahrenheit') },
      K: { name: i18n.t('calculator.unit_kelvin') },
    },
    custom: true
  },
  volume: {
    label: i18n.t('calculator.cat_volume'),
    units: {
      L: { name: i18n.t('calculator.unit_liters'), toBase: 1 },
      mL: { name: i18n.t('calculator.unit_milliliters'), toBase: 0.001 },
      gal: { name: i18n.t('calculator.unit_gallons_us'), toBase: 3.78541 },
      qt: { name: i18n.t('calculator.unit_quarts_us'), toBase: 0.946353 },
      cup: { name: i18n.t('calculator.unit_cups_us'), toBase: 0.236588 },
      floz: { name: i18n.t('calculator.unit_fluid_oz_us'), toBase: 0.0295735 },
    }
  },
  area: {
    label: i18n.t('calculator.cat_area'),
    units: {
      sqm: { name: i18n.t('calculator.unit_square_meters'), toBase: 1 },
      sqkm: { name: i18n.t('calculator.unit_square_km'), toBase: 1e6 },
      sqft: { name: i18n.t('calculator.unit_square_feet'), toBase: 0.092903 },
      sqmi: { name: i18n.t('calculator.unit_square_miles'), toBase: 2.59e6 },
      acre: { name: i18n.t('calculator.unit_acres'), toBase: 4046.86 },
      ha: { name: i18n.t('calculator.unit_hectares'), toBase: 10000 },
    }
  },
  speed: {
    label: i18n.t('calculator.cat_speed'),
    units: {
      'km/h': { name: 'km/h', toBase: 1 }, // i18n-exempt: a unit symbol
      'mph': { name: 'mph', toBase: 1.60934 }, // i18n-exempt: a unit symbol
      'm/s': { name: 'm/s', toBase: 3.6 }, // i18n-exempt: a unit symbol
      'kn': { name: i18n.t('calculator.unit_knots'), toBase: 1.852 },
    }
  }
};

let convCategory = 'length';

function buildConverterUI() {
  const cats = document.getElementById('conv-cats');
  const hint = esc(i18n.t('calculator.category_hint'));
  cats.innerHTML = Object.entries(UNITS).map(([key, cat]) =>
    `<button class="conv-cat-btn${key === convCategory ? ' active' : ''}" data-cat="${key}" data-on-click="setConvCategory(${hosArg(key)})" data-i18n-tip="calculator.category_hint" data-tip-desc="${hint}">${esc(cat.label)}</button>` // i18n-exempt: ' active' is a class name
  ).join('');
  populateUnitSelects();
  doConvert();
}

function setConvCategory(cat) {
  convCategory = cat;
  document.querySelectorAll('.conv-cat-btn').forEach(b =>
    b.classList.toggle('active', b.dataset.cat === cat)
  );
  populateUnitSelects();
  doConvert();
}

function populateUnitSelects() {
  const cat = UNITS[convCategory];
  const keys = Object.keys(cat.units);
  const fromSel = document.getElementById('conv-from-unit');
  const toSel = document.getElementById('conv-to-unit');

  fromSel.innerHTML = keys.map(k =>
    `<option value="${k}">${k} (${cat.units[k].name})</option>`
  ).join('');
  toSel.innerHTML = keys.map(k =>
    `<option value="${k}">${k} (${cat.units[k].name})</option>`
  ).join('');

  // Default to second unit for "to"
  if (keys.length > 1) toSel.value = keys[1];
}

function convertTemperature(val, from, to) {
  // Convert to Celsius first
  let c;
  if (from === 'C') c = val;
  else if (from === 'F') c = (val - 32) * 5 / 9;
  else if (from === 'K') c = val - 273.15;
  else return NaN;

  // Convert from Celsius to target
  if (to === 'C') return c;
  if (to === 'F') return c * 9 / 5 + 32;
  if (to === 'K') return c + 273.15;
  return NaN;
}

function doConvert() {
  const val = parseFloat(document.getElementById('conv-from-val').value);
  if (isNaN(val)) {
    document.getElementById('conv-to-val').value = '';
    document.getElementById('conv-formula').textContent = '';
    return;
  }

  const fromUnit = document.getElementById('conv-from-unit').value;
  const toUnit = document.getElementById('conv-to-unit').value;
  const cat = UNITS[convCategory];

  let result;
  if (cat.custom && convCategory === 'temperature') {
    result = convertTemperature(val, fromUnit, toUnit);
  } else {
    const fromBase = cat.units[fromUnit].toBase;
    const toBase = cat.units[toUnit].toBase;
    result = val * fromBase / toBase;
  }

  document.getElementById('conv-to-val').value = formatNumber(result);
  document.getElementById('conv-formula').textContent =
    `1 ${fromUnit} = ${formatNumber(cat.custom ? convertTemperature(1, fromUnit, toUnit) : cat.units[fromUnit].toBase / cat.units[toUnit].toBase)} ${toUnit}`;
}

function swapUnits() {
  const fromSel = document.getElementById('conv-from-unit');
  const toSel = document.getElementById('conv-to-unit');
  const tmp = fromSel.value;
  fromSel.value = toSel.value;
  toSel.value = tmp;
  doConvert();
}

// ── Currency converter ──

function doCurrConvert() {
  const amount = parseFloat(document.getElementById('cur-amount').value) || 0;
  const rate = parseFloat(document.getElementById('cur-rate').value) || 0;
  const fromCode = document.getElementById('cur-from').value.toUpperCase() || i18n.t('calculator.code_from');
  const toCode = document.getElementById('cur-to').value.toUpperCase() || i18n.t('calculator.code_to');

  document.getElementById('cur-from-label').textContent = fromCode || i18n.t('calculator.unit_word');
  const result = amount * rate;
  document.getElementById('cur-result').value = isNaN(result) ? '' :
    formatNumber(result) + ' ' + toCode;
}

// ── Keyboard support ──

document.addEventListener('keydown', function(e) {
  // Only handle when in basic or scientific mode
  if (currentMode !== 'basic' && currentMode !== 'scientific') return;

  // Don't capture if user is in an input/textarea
  if (e.target.tagName === 'INPUT' || e.target.tagName === 'TEXTAREA' || e.target.tagName === 'SELECT') return;

  const key = e.key;

  if ('0123456789'.includes(key)) {
    e.preventDefault();
    inputNum(key);
  } else if ('+-*/%'.includes(key)) {
    e.preventDefault();
    inputOp(key);
  } else if (key === '.') {
    e.preventDefault();
    inputNum('.');
  } else if (key === '(' || key === ')') {
    e.preventDefault();
    inputOp(key);
  } else if (key === 'Enter' || key === '=') {
    e.preventDefault();
    evaluate();
  } else if (key === 'Escape') {
    e.preventDefault();
    clearCalc();
  } else if (key === 'Backspace') {
    e.preventDefault();
    backspace();
  } else if (key === '^') {
    e.preventDefault();
    inputOp('^');
  }
});

// ── Utility ──

function esc(s) {
  return String(s || '').replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
}

// ── Init ──

loadHistory();
renderHistory();
buildConverterUI();
updateDisplay();
