'use strict';
/**
 * sevenzip.js: read a .7z archive's files (2026-11-06, for the instrument pipeline,
 * scripts/instrument-packs.js). FreePats ships its CC0 sample sets as .7z and this machine has no
 * 7-Zip; rather than ask anyone to install a tool, the pipeline reads them itself.
 *
 * Supports what FreePats' archives use: LZMA1 (03 01 01), LZMA2 (21), Deflate (04 01 08), Delta (03) or Copy (00), one coder or a chain of them (2026-11-13: the ukulele is Deflate then Delta), an
 * LZMA-encoded header, and solid blocks of many files. Every file's CRC32 from the archive is
 * checked; a mismatch throws. Anything else (BCJ, encryption) throws with its coder id.
 *
 *   readSevenZip(buffer) -> [{ name, data: Buffer, crc }]   (directories and empty files skipped)
 */

const zlib = require('zlib');
const { decodeLzma, decodeLzma2 } = require('./lzma');

const SIGNATURE = Buffer.from([0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c]);
const K = {
  End: 0x00, Header: 0x01, ArchiveProperties: 0x02, AdditionalStreamsInfo: 0x03,
  MainStreamsInfo: 0x04, FilesInfo: 0x05, PackInfo: 0x06, UnPackInfo: 0x07,
  SubStreamsInfo: 0x08, Size: 0x09, CRC: 0x0a, Folder: 0x0b, CodersUnPackSize: 0x0c,
  NumUnPackStream: 0x0d, EmptyStream: 0x0e, EmptyFile: 0x0f, Name: 0x11, EncodedHeader: 0x17,
  Dummy: 0x19,
};

class Reader {
  constructor(buf) { this.buf = buf; this.pos = 0; }
  byte() {
    if (this.pos >= this.buf.length) throw new Error('7z: header ends early');
    return this.buf[this.pos++];
  }
  number() {
    const first = this.byte();
    let mask = 0x80;
    let value = 0;
    for (let i = 0; i < 8; i++) {
      if ((first & mask) === 0) {
        const high = first & (mask - 1);
        return value + high * 2 ** (8 * i);
      }
      value += this.byte() * 2 ** (8 * i);
      mask >>>= 1;
    }
    return value;
  }
  uint32() { const v = this.buf.readUInt32LE(this.pos); this.pos += 4; return v; }
  bytes(n) { const b = this.buf.subarray(this.pos, this.pos + n); this.pos += n; return b; }
  bitVector(n) {
    const out = [];
    let b = 0, mask = 0;
    for (let i = 0; i < n; i++) {
      if (mask === 0) { b = this.byte(); mask = 0x80; }
      out.push((b & mask) !== 0);
      mask >>>= 1;
    }
    return out;
  }
  digests(n) {
    const allDefined = this.byte();
    const defined = allDefined ? new Array(n).fill(true) : this.bitVector(n);
    return defined.map((d) => (d ? this.uint32() : null));
  }
  expect(id) {
    const got = this.byte();
    if (got !== id) throw new Error(`7z: expected property ${id}, found ${got}`);
  }
}

function readPackInfo(r) {
  const packPos = r.number();
  const n = r.number();
  let sizes = [];
  for (;;) {
    const id = r.byte();
    if (id === K.End) break;
    if (id === K.Size) sizes = Array.from({ length: n }, () => r.number());
    else if (id === K.CRC) r.digests(n);
    else throw new Error(`7z: unexpected ${id} in pack info`);
  }
  return { packPos, sizes };
}

function readFolder(r) {
  const numCoders = r.number();
  const coders = [];
  let totalOut = 0, totalIn = 0;
  for (let i = 0; i < numCoders; i++) {
    const flag = r.byte();
    const id = r.bytes(flag & 0x0f).toString('hex');
    let nIn = 1, nOut = 1;
    if (flag & 0x10) { nIn = r.number(); nOut = r.number(); }
    let props = Buffer.alloc(0);
    if (flag & 0x20) props = Buffer.from(r.bytes(r.number()));
    if (flag & 0x80) throw new Error('7z: alternative coder methods are not supported');
    coders.push({ id, props, nIn, nOut });
    totalOut += nOut;
    totalIn += nIn;
  }
  // Bind pairs join one coder's output to another's input (a filter chain: FreePats' 2026 ukulele
  // is Deflate and a Delta filter). With one-in, one-out coders, in-stream i and out-stream i are
  // coder i's; the packed stream feeds the one input no pair binds, and the folder's output is the
  // one output no pair binds.
  const nBinds = totalOut - 1;
  const binds = [];
  for (let i = 0; i < nBinds; i++) binds.push({ inIndex: r.number(), outIndex: r.number() });
  const packed = totalIn - nBinds;
  const packedIn = [];
  if (packed > 1) for (let i = 0; i < packed; i++) packedIn.push(r.number());
  else for (let i = 0; i < totalIn; i++) if (!binds.some((b) => b.inIndex === i)) packedIn.push(i);
  let mainOut = 0;
  for (let i = 0; i < totalOut; i++) if (!binds.some((b) => b.outIndex === i)) mainOut = i;
  return { coders, totalOut, unpackSizes: [], binds, packedIn, mainOut };
}

/** The size of a folder's whole output (its unbound out-stream). */
function folderSize(f) {
  return f.unpackSizes[f.mainOut || 0];
}

function readUnpackInfo(r) {
  r.expect(K.Folder);
  const n = r.number();
  if (r.byte() !== 0) throw new Error('7z: external folders are not supported');
  const folders = Array.from({ length: n }, () => readFolder(r));
  r.expect(K.CodersUnPackSize);
  for (const f of folders) f.unpackSizes = Array.from({ length: f.totalOut }, () => r.number());
  for (;;) {
    const id = r.byte();
    if (id === K.End) break;
    if (id === K.CRC) r.digests(n).forEach((c, i) => { folders[i].crc = c; });
    else throw new Error(`7z: unexpected ${id} in unpack info`);
  }
  return folders;
}

function readSubStreams(r, folders) {
  const counts = folders.map(() => 1);
  let id = r.byte();
  if (id === K.NumUnPackStream) {
    for (let i = 0; i < folders.length; i++) counts[i] = r.number();
    id = r.byte();
  }
  const sizes = [];
  const hasSizes = id === K.Size;
  folders.forEach((f, i) => {
    if (counts[i] === 0) return;
    const total = folderSize(f);
    let sum = 0;
    for (let j = 0; j < counts[i] - 1; j++) {
      const s = hasSizes ? r.number() : 0;
      sizes.push(s);
      sum += s;
    }
    sizes.push(total - sum);
  });
  if (hasSizes) id = r.byte();
  // CRCs: only for streams whose folder CRC does not already stand for them.
  const crcs = [];
  let unknown = 0;
  folders.forEach((f, i) => { if (!(counts[i] === 1 && f.crc != null)) unknown += counts[i]; });
  let digests = null;
  if (id === K.CRC) { digests = r.digests(unknown); id = r.byte(); }
  let k = 0;
  folders.forEach((f, i) => {
    if (counts[i] === 1 && f.crc != null) crcs.push(f.crc);
    else for (let j = 0; j < counts[i]; j++) crcs.push(digests ? digests[k++] : null);
  });
  if (id !== K.End) throw new Error(`7z: unexpected ${id} in substreams`);
  return { counts, sizes, crcs };
}

function readStreamsInfo(r) {
  let pack = null, folders = [], sub = null;
  for (;;) {
    const id = r.byte();
    if (id === K.End) break;
    if (id === K.PackInfo) pack = readPackInfo(r);
    else if (id === K.UnPackInfo) folders = readUnpackInfo(r);
    else if (id === K.SubStreamsInfo) sub = readSubStreams(r, folders);
    else throw new Error(`7z: unexpected ${id} in streams info`);
  }
  if (!sub) {
    sub = {
      counts: folders.map(() => 1),
      sizes: folders.map((f) => folderSize(f)),
      crcs: folders.map((f) => (f.crc == null ? null : f.crc)),
    };
  }
  return { pack, folders, sub };
}

/** Unpack every folder of a streams info into its whole output. */
function unpackFolders(buf, info) {
  let offset = 32 + info.pack.packPos;
  return info.folders.map((f, i) => {
    const packed = buf.subarray(offset, offset + info.pack.sizes[i]);
    offset += info.pack.sizes[i];
    if (f.coders.some((c) => c.nIn !== 1 || c.nOut !== 1) || f.packedIn.length !== 1) {
      throw new Error(`7z: a folder of ${f.coders.length} coders that is not a simple chain is not supported`);
    }
    // Follow the chain from the coder the packed stream feeds to the one whose output is the folder's.
    let k = f.packedIn[0];
    let data = decodeOne(f.coders[k], packed, f.unpackSizes[k]);
    for (let step = 1; step < f.coders.length; step++) {
      const bind = f.binds.find((b) => b.outIndex === k);
      if (!bind) throw new Error('7z: a broken coder chain');
      k = bind.inIndex;
      data = decodeOne(f.coders[k], data, f.unpackSizes[k]);
    }
    return data;
  });
}

function decodeOne(c, input, size) {
  if (c.id === '030101') return decodeLzma(c.props, input, size);
  if (c.id === '21') return decodeLzma2(input, size);
  if (c.id === '040108') return zlib.inflateRawSync(input);
  if (c.id === '00') return Buffer.from(input);
  if (c.id === '03') return decodeDelta(c.props, input);
  throw new Error(`7z: coder ${c.id} is not supported (LZMA1, LZMA2, Deflate, Delta and Copy are)`);
}

/** 7-Zip's Delta filter: each byte is stored as its difference from the byte `distance` before it
 * (the property byte is distance - 1), which helps the compressor on audio. */
function decodeDelta(props, input) {
  const distance = (props.length ? props[0] : 0) + 1;
  const out = Buffer.from(input);
  for (let i = distance; i < out.length; i++) out[i] = (out[i] + out[i - distance]) & 0xff;
  return out;
}

function readFilesInfo(r) {
  const n = r.number();
  const files = Array.from({ length: n }, () => ({ name: '', emptyStream: false, emptyFile: false }));
  let emptyStreams = null;
  for (;;) {
    const id = r.byte();
    if (id === K.End) break;
    const size = r.number();
    const end = r.pos + size;
    if (id === K.EmptyStream) {
      emptyStreams = r.bitVector(n);
      emptyStreams.forEach((e, i) => { files[i].emptyStream = e; });
    } else if (id === K.EmptyFile) {
      const nEmpty = emptyStreams ? emptyStreams.filter(Boolean).length : 0;
      const v = r.bitVector(nEmpty);
      let k = 0;
      files.forEach((f) => { if (f.emptyStream) f.emptyFile = v[k++]; });
    } else if (id === K.Name) {
      if (r.byte() !== 0) throw new Error('7z: external names are not supported');
      const text = r.buf.subarray(r.pos, end).toString('utf16le');
      text.split('\u0000').slice(0, n).forEach((name, i) => { files[i].name = name; });
    }
    r.pos = end;
  }
  return files;
}

function readSevenZip(buf) {
  if (!buf.subarray(0, 6).equals(SIGNATURE)) throw new Error('7z: not a 7z archive');
  const nextOffset = Number(buf.readBigUInt64LE(12));
  const nextSize = Number(buf.readBigUInt64LE(20));
  const nextCrc = buf.readUInt32LE(28);
  let header = buf.subarray(32 + nextOffset, 32 + nextOffset + nextSize);
  if (zlib.crc32(header) !== nextCrc) throw new Error('7z: header checksum mismatch');
  let r = new Reader(header);
  let id = r.byte();
  while (id === K.EncodedHeader) {
    const info = readStreamsInfo(r);
    header = unpackFolders(buf, info)[0];
    if (info.folders[0].crc != null && zlib.crc32(header) !== info.folders[0].crc) {
      throw new Error('7z: encoded header checksum mismatch');
    }
    r = new Reader(header);
    id = r.byte();
  }
  if (id !== K.Header) throw new Error(`7z: expected a header, found ${id}`);
  let main = null, files = [];
  for (;;) {
    id = r.byte();
    if (id === K.End) break;
    if (id === K.ArchiveProperties) {
      for (;;) { const p = r.byte(); if (p === K.End) break; r.bytes(r.number()); }
    } else if (id === K.AdditionalStreamsInfo) {
      readStreamsInfo(r);
    } else if (id === K.MainStreamsInfo) {
      main = readStreamsInfo(r);
    } else if (id === K.FilesInfo) {
      files = readFilesInfo(r);
    } else {
      throw new Error(`7z: unexpected ${id} in header`);
    }
  }
  if (!main) return [];
  const unpacked = unpackFolders(buf, main);
  // Walk the files in order, taking each non-empty one's stream from its folder.
  const out = [];
  let folder = 0, inFolder = 0, offset = 0, stream = 0;
  for (const f of files) {
    if (f.emptyStream) continue;
    while (folder < main.folders.length && inFolder >= main.sub.counts[folder]) {
      folder++;
      inFolder = 0;
      offset = 0;
    }
    const size = main.sub.sizes[stream];
    const data = unpacked[folder].subarray(offset, offset + size);
    const crc = main.sub.crcs[stream];
    if (crc != null && zlib.crc32(data) !== crc) throw new Error(`7z: ${f.name} fails its checksum`);
    out.push({ name: f.name.replace(/\\/g, '/'), data, crc });
    offset += size;
    inFolder++;
    stream++;
  }
  return out;
}

module.exports = { readSevenZip };
