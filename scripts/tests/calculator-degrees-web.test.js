// The website's calculator works in DEGREES, like the desktop one (src/gui/pages/calculator.rs uses
// to_radians() before sin, cos and tan), so the same keys give the same answer on both
// (2026-10-24). Run: node --test scripts/tests/calculator-degrees-web.test.js
//
// The page's own evaluator is read out of web/pages/calculator-app.js (from `function factorial`
// up to `function evaluate`) and run in a vm. CALC_APP=<path> points it at another copy.
// Seen red 2026-10-24 against the radian evaluator (git show HEAD~1:web/pages/calculator-app.js):
// "sin(30) is 0.5, as on the desktop" (actual -0.988).
'use strict';

const test = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const path = require('path');
const vm = require('vm');

const FILE = process.env.CALC_APP || path.join(__dirname, '..', '..', 'web', 'pages', 'calculator-app.js');

function safeEval() {
  const src = fs.readFileSync(FILE, 'utf8');
  const start = src.indexOf('function factorial');
  const end = src.indexOf('function evaluate()');
  assert.ok(start >= 0 && end > start, 'the evaluator is where the test expects it');
  const ctx = vm.createContext({ Math, Function, String, NaN, Infinity });
  vm.runInContext(src.slice(start, end) + '\n;globalThis.__safeEval = safeEval;', ctx);
  return ctx.__safeEval;
}

const near = (a, b, msg) => assert.ok(Math.abs(a - b) < 1e-9, `${msg} (actual ${a})`);

test('sine, cosine and tangent take degrees, as on the desktop', () => {
  const ev = safeEval();
  near(ev('sin(30)'), 0.5, 'sin(30) is 0.5, as on the desktop');
  near(ev('cos(60)'), 0.5, 'cos(60) is 0.5');
  near(ev('tan(45)'), 1, 'tan(45) is 1');
  near(ev('sin(90)+cos(0)'), 2, 'two functions in one expression');
});

test('the rest of the evaluator is unchanged', () => {
  const ev = safeEval();
  near(ev('2+3*4'), 14, 'arithmetic');
  near(ev('sqrt(16)'), 4, 'square root');
  near(ev('fact(5)'), 120, 'factorial');
  near(ev('2^10'), 1024, 'powers');
});
