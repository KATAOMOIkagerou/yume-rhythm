#!/usr/bin/env node
// Reference trace generator for crates/beat (see README.md).
//
//   node generate.mjs                      all synthetic fixtures -> crates/beat/tests/fixtures/
//   node generate.mjs --only silence_gap   one synthetic fixture
//   node generate.mjs --wav song.wav [--out song.wav.trace.json]
//                                          trace for a real song (never commit either file)

import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { jh, m1 } from './frozen-audio-core.js';
import { XorShift32 } from './lib/dsp.mjs';
import { CATALOG, synthesize } from './lib/signals.mjs';
import { ANALYSER, FRAME_HZ, NOD_OFFSET_MS, NOD_STRENGTH, runTrace } from './lib/trace.mjs';
import { readWav } from './lib/wav.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const fixturesDir = path.resolve(here, '../../crates/beat/tests/fixtures');
const engineFile = path.join(here, 'frozen-audio-core.js');
const engineSha256 = crypto.createHash('sha256').update(fs.readFileSync(engineFile)).digest('hex');

function parseArgs(argv) {
  const args = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === '--only' || a === '--wav' || a === '--out') args[a.slice(2)] = argv[++i];
    else if (a === '--help' || a === '-h') args.help = true;
    else throw new Error(`unknown argument ${a}`);
  }
  return args;
}

/** One top-level key / trace column per line: small diffs, still plain JSON. */
function writeJson(file, obj) {
  const lines = [];
  const entries = Object.entries(obj);
  entries.forEach(([k, v], i) => {
    const comma = i + 1 < entries.length ? ',' : '';
    if (v && typeof v === 'object' && !Array.isArray(v) && (k === 'columns' || k === 'bytes')) {
      const inner = Object.entries(v);
      lines.push(`  ${JSON.stringify(k)}: {`);
      inner.forEach(([ik, iv], j) => {
        lines.push(`    ${JSON.stringify(ik)}: ${JSON.stringify(iv)}${j + 1 < inner.length ? ',' : ''}`);
      });
      lines.push(`  }${comma}`);
    } else {
      lines.push(`  ${JSON.stringify(k)}: ${JSON.stringify(v)}${comma}`);
    }
  });
  fs.writeFileSync(file, `{\n${lines.join('\n')}\n}\n`);
}

function header(name, description, sampleRate) {
  return {
    format: 1,
    name,
    description,
    engine: { file: 'frozen-audio-core.js', sha256: engineSha256 },
    analyser: ANALYSER,
    sampleRate,
    clock: { hz: FRAME_HZ, nodOffsetMs: NOD_OFFSET_MS, nodStrength: NOD_STRENGTH },
  };
}

function generateSynthetic(entry) {
  const samples = synthesize(entry.spec);
  const skip = entry.skip ?? [];
  const trace = runTrace(samples, entry.spec.sampleRate, { skip, recordBytes: !!entry.recordBytes });
  const out = {
    ...header(entry.name, entry.description, entry.spec.sampleRate),
    skip,
    source: { kind: 'synth', spec: entry.spec },
    samplesHash: trace.samplesHash,
    frames: trace.frames,
    bands: trace.bands,
    columns: trace.columns,
  };
  if (trace.bytes) out.bytes = trace.bytes;
  const file = path.join(fixturesDir, `${entry.name}.json`);
  writeJson(file, out);
  const c = trace.columns;
  const beats = c.beats[c.beats.length - 1];
  const lockedPct = ((100 * [...c.locked].filter((x) => x === '1').length) / trace.frames).toFixed(1);
  const bpms = [...new Set(c.bpm.filter((b) => b > 0))].join('/');
  console.log(
    `${entry.name.padEnd(22)} frames=${trace.frames} beats=${beats} locked=${lockedPct}% bpm=${bpms || '-'}  ` +
      `${(fs.statSync(file).size / 1024).toFixed(0)} KiB`,
  );
}

/** Edge cases + random inputs for the locked beat nod curve (m1). */
function generateNodCases() {
  const rng = new XorShift32(2024);
  const u = () => rng.nextU32() / 4294967296;
  const cases = [];
  const push = (now, next, period, strength) => {
    const r = m1(now, next ?? undefined, period ?? undefined, strength);
    cases.push([now, next, period, strength, r]);
  };
  push(1000, null, 500, 1);
  push(1000, 1200, null, 1);
  push(1000, 1200, 249.9, 1);
  push(1000, 1200, 250, 1);
  push(1000, 1200, 1600, 1);
  push(1000, 1200, 1600.1, 1);
  push(1000, 1000, 500, 1);
  push(1000, 1150, 500, 5);
  push(1000, 850, 500, -2);
  push(-1234.5, 777, 640, 0.9);
  for (const p of [300, 500, 566.6666666666666]) {
    const x = Math.min(170, p * 0.3);
    push(5000, 5000 + x, p, 1);
    push(5000, 5000 - x, p, 1);
    push(5000, 5000 + p / 2, p, 1);
  }
  for (let i = 0; i < 2000; i++) {
    const period = 150 + u() * 1600;
    const now = u() * 200000;
    const next = now + (u() - 0.5) * 8000;
    push(now, next, period, -0.5 + u() * 4.5);
  }
  writeJson(path.join(fixturesDir, 'nod_cases.json'), {
    format: 1,
    engine: { file: 'frozen-audio-core.js', sha256: engineSha256 },
    idle: jh,
    columns: '[now, nextBeatAt|null, periodMs|null, strength, m1 result|null]',
    cases,
  });
  console.log(`nod_cases              cases=${cases.length}`);
}

function generateWav(wavPath, outPath) {
  const wav = readWav(wavPath);
  const trace = runTrace(wav.samples, wav.sampleRate);
  const out = {
    ...header(path.basename(wavPath), `real audio: ${path.basename(wavPath)}`, wav.sampleRate),
    skip: [],
    source: {
      kind: 'wav',
      file: path.basename(wavPath),
      samples: wav.samples.length,
      channels: wav.channels,
      bits: wav.bits,
      format: wav.format,
    },
    samplesHash: trace.samplesHash,
    frames: trace.frames,
    bands: trace.bands,
    columns: trace.columns,
  };
  writeJson(outPath, out);
  const c = trace.columns;
  console.log(`${outPath}: frames=${trace.frames} beats=${c.beats[c.beats.length - 1]}`);
}

const args = parseArgs(process.argv.slice(2));
if (args.help) {
  console.log('usage: node generate.mjs [--only <name>] | --wav <file.wav> [--out <trace.json>]');
} else if (args.wav) {
  generateWav(args.wav, args.out ?? `${args.wav}.trace.json`);
} else {
  fs.mkdirSync(fixturesDir, { recursive: true });
  const entries = args.only ? CATALOG.filter((e) => e.name === args.only) : CATALOG;
  if (entries.length === 0) throw new Error(`no fixture named ${args.only}`);
  for (const entry of entries) generateSynthetic(entry);
  if (!args.only) generateNodCases();
}
