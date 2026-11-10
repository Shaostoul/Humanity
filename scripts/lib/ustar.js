// A PLAIN USTAR ARCHIVE, DETERMINISTIC (the packs' archive format; the app's reader is
// src/instrument_packs/tar.rs, which refuses anything that could write outside its folder).
//
// Moved here from scripts/simready/pack.js on 2026-11-10 so the garment texture packs
// (scripts/gen-makehuman-look.js --pack) write their archives the same way: names sorted, every
// entry mode 0644, owner 0, time 0, so the same files always make the same bytes (the SHA-256 the
// app checks a download against is then the converter's, run after run).

'use strict';

/** A ustar archive of `files` ({ "path/in/archive": Buffer }), names sorted, ending in two empty blocks. */
function tarOf(files) {
  const blocks = [];
  for (const name of Object.keys(files).sort()) {
    const data = files[name];
    if (Buffer.byteLength(name) > 100) throw new Error(`tar: name too long: ${name}`);
    const h = Buffer.alloc(512);
    h.write(name, 0, 'utf8');
    h.write('0000644\0', 100);
    h.write('0000000\0', 108);
    h.write('0000000\0', 116);
    h.write(data.length.toString(8).padStart(11, '0') + '\0', 124);
    h.write('00000000000\0', 136);
    h.write('        ', 148);
    h.write('0', 156);
    h.write('ustar\0', 257);
    h.write('00', 263);
    let sum = 0;
    for (let i = 0; i < 512; i++) sum += h[i];
    h.write(sum.toString(8).padStart(6, '0') + '\0 ', 148);
    blocks.push(h, data, Buffer.alloc((512 - (data.length % 512)) % 512));
  }
  blocks.push(Buffer.alloc(1024));
  return Buffer.concat(blocks);
}

module.exports = { tarOf };
