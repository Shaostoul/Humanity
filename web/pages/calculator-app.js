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
  document.querySelectorAll('.calc-tab').forEach(t =>
    t.classList.toggle('active', t.dataset.mode === mode)
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

function safeEval(expr) {
  // Replace scientific functions with Math equivalents
  let e = expr
    .replace(/sin\(/g, 'Math.sin(')
    .replace(/cos\(/g, 'Math.cos(')
    .replace(/tan\(/g, 'Math.tan(')
    .replace(/log\(/g, 'Math.log10(')
    .replace(/ln\(/g, 'Math.log(')
    .replace(/sqrt\(/g, 'Math.sqrt(')
    .replace(/pow\(/g, 'Math.pow(')
    .replace(/fact\(/g, '__fact__(')
    .replace(/\^/g, '**')
    .replace(/pi/g, String(Math.PI))
    .replace(/(\d)%/g, '($1/100)');

  // Sanitize: only allow numbers, operators, parentheses, dots, commas, Math.*, __fact__
  const sanitized = e.replace(/[^0-9+\-*/.(),%\s^]/g, function(ch) {
    return ch; // allow through for Math.* calls
  });

  // Create a safe scope
  const __fact__ = factorial;
  try {
    const fn = new Function('Math', '__fact__', '"use strict"; return (' + e + ')');
    return fn(Math, __fact__);
  } catch (err) {
    return NaN;
  }
}

function evaluate() {
  if (!expression.trim()) return;
  const result = safeEval(expression);
  const resultStr = isNaN(result) ? 'Error' : formatNumber(result);

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
      btn.textContent = 'Copied!';
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
    list.innerHTML = '<div class="hist-empty">No calculations yet.</div>';
    return;
  }
  list.innerHTML = history.map((h, i) =>
    `<div class="hist-item" onclick="useHistory(${i})" title="Click to reuse">
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

const UNITS = {
  length: {
    label: 'Length',
    units: {
      m: { name: 'Meters', toBase: 1 },
      km: { name: 'Kilometers', toBase: 1000 },
      cm: { name: 'Centimeters', toBase: 0.01 },
      mm: { name: 'Millimeters', toBase: 0.001 },
      ft: { name: 'Feet', toBase: 0.3048 },
      in: { name: 'Inches', toBase: 0.0254 },
      mi: { name: 'Miles', toBase: 1609.344 },
      yd: { name: 'Yards', toBase: 0.9144 },
    }
  },
  weight: {
    label: 'Weight',
    units: {
      kg: { name: 'Kilograms', toBase: 1 },
      g: { name: 'Grams', toBase: 0.001 },
      mg: { name: 'Milligrams', toBase: 0.000001 },
      lb: { name: 'Pounds', toBase: 0.453592 },
      oz: { name: 'Ounces', toBase: 0.0283495 },
      t: { name: 'Metric Tons', toBase: 1000 },
    }
  },
  temperature: {
    label: 'Temperature',
    units: {
      C: { name: 'Celsius' },
      F: { name: 'Fahrenheit' },
      K: { name: 'Kelvin' },
    },
    custom: true
  },
  volume: {
    label: 'Volume',
    units: {
      L: { name: 'Liters', toBase: 1 },
      mL: { name: 'Milliliters', toBase: 0.001 },
      gal: { name: 'Gallons (US)', toBase: 3.78541 },
      qt: { name: 'Quarts (US)', toBase: 0.946353 },
      cup: { name: 'Cups (US)', toBase: 0.236588 },
      floz: { name: 'Fluid Oz (US)', toBase: 0.0295735 },
    }
  },
  area: {
    label: 'Area',
    units: {
      sqm: { name: 'Square Meters', toBase: 1 },
      sqkm: { name: 'Square Km', toBase: 1e6 },
      sqft: { name: 'Square Feet', toBase: 0.092903 },
      sqmi: { name: 'Square Miles', toBase: 2.59e6 },
      acre: { name: 'Acres', toBase: 4046.86 },
      ha: { name: 'Hectares', toBase: 10000 },
    }
  },
  speed: {
    label: 'Speed',
    units: {
      'km/h': { name: 'km/h', toBase: 1 },
      'mph': { name: 'mph', toBase: 1.60934 },
      'm/s': { name: 'm/s', toBase: 3.6 },
      'kn': { name: 'Knots', toBase: 1.852 },
    }
  }
};

let convCategory = 'length';

function buildConverterUI() {
  const cats = document.getElementById('conv-cats');
  cats.innerHTML = Object.entries(UNITS).map(([key, cat]) =>
    `<button class="conv-cat-btn${key === convCategory ? ' active' : ''}" onclick="setConvCategory('${key}')">${cat.label}</button>`
  ).join('');
  populateUnitSelects();
  doConvert();
}

function setConvCategory(cat) {
  convCategory = cat;
  document.querySelectorAll('.conv-cat-btn').forEach(b =>
    b.classList.toggle('active', b.textContent === UNITS[cat].label)
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
  const fromCode = document.getElementById('cur-from').value.toUpperCase() || 'FROM';
  const toCode = document.getElementById('cur-to').value.toUpperCase() || 'TO';

  document.getElementById('cur-from-label').textContent = fromCode || 'unit';
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
