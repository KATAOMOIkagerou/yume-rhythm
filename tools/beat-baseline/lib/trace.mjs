// Drives the frozen baseline detector (x1) over a mono Float32 signal at a
// fixed simulated 60 Hz clock and records one row per processed frame.

import { m1, x1 } from '../frozen-audio-core.js';
import { FakeAnalyser } from './analyser.mjs';
import { fnv1a } from './dsp.mjs';

/** Analyser settings used by the baseline host (E1.ensureGraph). */
export const ANALYSER = { fftSize: 2048, smoothing: 0.08, minDb: -90, maxDb: -3 };
export const FRAME_HZ = 60;
/** The trace also samples m1 (locked beat nod) at now + NOD_OFFSET_MS with this strength. */
export const NOD_OFFSET_MS = 8;
export const NOD_STRENGTH = 1.2;

/** now_k = k * 1000 / 60 for k in [0, frameCount), minus frames inside a [startMs, endMs) skip range. */
export function frameTimes(frameCount, skip) {
  const times = [];
  for (let k = 0; k < frameCount; k++) {
    const now = (k * 1000) / FRAME_HZ;
    if (skip.some(([a, b]) => now >= a && now < b)) continue;
    times.push(now);
  }
  return times;
}

export function frameCountFor(sampleCount, sampleRate) {
  return Math.floor((sampleCount * FRAME_HZ) / sampleRate);
}

/**
 * @param {Float32Array} samples mono signal
 * @param {number} sampleRate
 * @param {{skip?: number[][], recordBytes?: boolean}} opts
 */
export function runTrace(samples, sampleRate, { skip = [], recordBytes = false } = {}) {
  const analyser = new FakeAnalyser({ sampleRate, ...ANALYSER });
  analyser.setSource(samples);
  const det = new x1(analyser);
  const frameCount = frameCountFor(samples.length, sampleRate);
  const times = frameTimes(frameCount, skip);

  const cols = {
    beat: '',
    onset: '',
    locked: '',
    strength: [],
    level: [],
    bass: [],
    bpm: [],
    beats: [],
    nextBeatAt: [],
    periodMs: [],
    nod: [],
    byteHash: '',
  };
  const byteChunks = [];
  let bins = 0;

  for (const now of times) {
    analyser.setEnd(Math.floor((now * sampleRate) / 1000));
    const f = det.process(now);
    for (const key of ['strength', 'level', 'bass', 'bpm', 'beats']) {
      if (!Number.isFinite(f[key])) throw new Error(`non-finite ${key} at ${now}`);
    }
    cols.beat += f.beat ? '1' : '0';
    cols.onset += f.onset ? '1' : '0';
    cols.locked += f.locked ? '1' : '0';
    cols.strength.push(f.strength);
    cols.level.push(f.level);
    cols.bass.push(f.bass);
    cols.bpm.push(f.bpm);
    cols.beats.push(f.beats);
    cols.nextBeatAt.push(f.nextBeatAt ?? null);
    cols.periodMs.push(f.periodMs ?? null);
    cols.nod.push(m1(now + NOD_OFFSET_MS, f.nextBeatAt, f.periodMs, NOD_STRENGTH));
    cols.byteHash += fnv1a(det.freq);
    if (recordBytes) {
      if (bins === 0) bins = Math.max(...det.bandHi) + 1;
      byteChunks.push(Buffer.from(det.freq.subarray(0, bins)));
    }
  }

  const trace = {
    samplesHash: fnv1a(new Uint8Array(samples.buffer, samples.byteOffset, samples.byteLength)),
    frames: times.length,
    bands: { lo: det.bandLo, hi: det.bandHi, w: det.bandW },
    columns: cols,
  };
  if (recordBytes) {
    trace.bytes = { bins, base64: Buffer.concat(byteChunks).toString('base64') };
  }
  return trace;
}
