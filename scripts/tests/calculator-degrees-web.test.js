// The website's calculator works in DEGREES, like the desktop one (src/gui/pages/calculator.rs uses
// to_radians() before sin, cos and tan), so the same keys give the same answer on both
// (2026-10-24). Run: node --test scripts/tests/calculator-degrees-web.test.js
//
// The page's own evaluator is read out of web/pages/calculator-app.js (from `function factorial`
// up to `function evaluate`) and run in a vm. CALC_APP=<path> points it at another copy.
// Seen red 2026-10-24 against the radian evaluator (git show HEAD~1:web/pages/calculator-app.js):
// "sin(30) is 0.5, as on the desktop" (actual -0.988).
//
// The vm refuses to run a string as code (codeGeneration.strings false), as a browser does under
// the site's Content-Security-Policy, which never allowed 'unsafe-eval' (2026-11-20). Seen red
// against the new Function evaluator (git show c2493b272:web/pages/calculator-app.js): every case
// failed with NaN, as every sum on the live site answered Error.
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
  const ctx = vm.createContext({ Math, String, NaN, Infinity }, { codeGeneration: { strings: false } });
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

test('the reader: order, powers, percent, pi, written-against times, and errors as NaN', () => {
  const ev = safeEval();
  near(ev('(2+3)*4'), 20, 'brackets first');
  near(ev('2^3^2'), 512, 'powers right to left');
  near(ev('-2^2'), -4, 'a minus applies after the power');
  near(ev('2^-1'), 0.5, 'a negative power');
  near(ev('50%'), 0.5, 'a percent is a hundredth');
  near(ev('200*10%'), 20, 'ten percent of 200');
  near(ev('2pi'), 2 * Math.PI, '2pi is two times pi');
  near(ev('3(4+1)'), 15, 'a number against a bracket multiplies');
  near(ev('pow(2,8)'), 256, 'pow takes two values');
  near(ev('log(1000)+ln(1)'), 3, 'log is base ten, ln natural');
  near(ev('7/2'), 3.5, 'division');
  for (const bad of ['', '2+', 'sin(30', 'foo(2)', '2)(', 'constructor(1)', '1;2', 'alert(1)']) {
    assert.ok(Number.isNaN(ev(bad)), 'NaN for ' + JSON.stringify(bad));
  }
});
