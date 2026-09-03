// Cloud radial-artifact metric (the "rosette" / "pinwheel" / "the pinch at
// my feet"). Average brightness along each RADIUS from a chosen centre, then
// measure the high-frequency content of that angular profile: a radial sliver
// survives radial averaging because it occupies one angle at every radius,
// while a cloud blob washes out.
//
// It only means something WITH A CONTROL. Capture the coverage-alpha channel
// (map_diag 1) in the same run as the reference, and compare A vs B inside one
// sweep - never across runs. Cross-run comparison of cloud captures is invalid
// unless the advection clock is pinned (showcase {"cloud_clock":"120"}, or the
// F10 "Freeze cloud drift" box); without the pin two runs of the SAME build
// differ in 20% of pixels by more than 40 levels, which silently turned several
// "measured improvements" in this arc into noise.
//
//   node scripts/cloud-spoke-metric.js "label=path/to/a.png" "label=path/to/b.png"
//
// Repeat-capture noise floor is about 0.7, so treat smaller gaps as nothing.
const sharp = require('sharp');
// Centre defaults to the screen centre. Pass CX/CY/R1 via env to aim at the
// NADIR pixel on a tilted look: y_nadir = 720 + 720*tan(look_offset_deg) at
// 90 deg FOV / 1440 rows (758 at 3 deg, 873 at 12 deg, 1324 at 40 deg).
const CX = +(process.env.CX || 1280), CY = +(process.env.CY || 690),
      R0 = 30, R1 = +(process.env.R1 || 320), NA = 720;

async function profile(file) {
  const { data, info } = await sharp(file).greyscale().raw()
    .toBuffer({ resolveWithObject: true });
  const W = info.width;
  const A = new Float64Array(NA), N = new Float64Array(NA);
  for (let ai = 0; ai < NA; ai++) {
    const th = ai * 2 * Math.PI / NA, cs = Math.cos(th), sn = Math.sin(th);
    for (let r = R0; r < R1; r += 0.5) {
      const x = Math.round(CX + cs * r), y = Math.round(CY + sn * r);
      A[ai] += data[y * W + x]; N[ai]++;
    }
    A[ai] /= N[ai];
  }
  // high-pass: subtract a +-12 deg boxcar so one big cloud does not count
  const half = Math.round(24 / 360 * NA);
  let s = 0;
  for (let i = 0; i < NA; i++) {
    let m = 0;
    for (let k = -half; k <= half; k++) m += A[(i + k + NA) % NA];
    m /= (2 * half + 1);
    s += (A[i] - m) ** 2;
  }
  return Math.sqrt(s / NA);
}

(async () => {
  for (const [tag, f] of process.argv.slice(2).map(a => a.split('='))) {
    console.log(tag.padEnd(22), 'spoke =', (await profile(f)).toFixed(3));
  }
})();
