// Procedural, deterministic test signals. The full spec of each signal is
// written into its fixture, and crates/beat/tests/common/synth.rs regenerates
// the identical Float32 samples from that spec (same loops, same operation
// order, only exact arithmetic via dsp.mjs helpers). Keep both sides in sync.

import { XorShift32, dexp, sinCycles } from './dsp.mjs';

const LN2 = 0.6931471805599453;
const _ = null;

// 16-step (sixteenth-note) patterns. kick/snare/hat: velocity (0 = rest);
// bass: semitone offset from A1 (55 Hz) or null.
export const PATTERNS = {
  fourFloor: {
    kick: [1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0],
    snare: [0, 0, 0, 0, 0.8, 0, 0, 0, 0, 0, 0, 0, 0.8, 0, 0, 0],
    hat: [0, 0, 0.6, 0, 0, 0, 0.6, 0, 0, 0, 0.6, 0, 0, 0, 0.6, 0],
    bass: [_, _, 0, _, _, _, 0, _, _, _, 3, _, _, _, 5, _],
  },
  hiphop: {
    kick: [1, 0, 0, 0, 0, 0, 0, 0.8, 0, 0, 0.9, 0, 0, 0, 0, 0],
    snare: [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0],
    hat: [0.5, 0, 0.35, 0, 0.5, 0, 0.35, 0, 0.5, 0, 0.35, 0, 0.5, 0, 0.35, 0],
    bass: [0, _, _, _, _, _, _, 0, _, _, -2, _, _, _, _, _],
  },
  rock: {
    kick: [1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 0],
    snare: [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0],
    hat: [0.5, 0, 0.5, 0, 0.5, 0, 0.5, 0, 0.5, 0, 0.5, 0, 0.5, 0, 0.5, 0],
    bass: [0, _, _, _, _, _, _, _, 0, _, 0, _, _, _, _, _],
  },
  dnb: {
    kick: [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0],
    snare: [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0],
    hat: [0.4, 0, 0.4, 0, 0.4, 0, 0.4, 0, 0.4, 0, 0.4, 0, 0.4, 0, 0.4, 0],
    bass: [0, _, _, _, _, _, _, _, _, _, 0, _, _, _, _, _],
  },
  // Same step pattern as the baseline engine's synthetic demo drum (A1).
  demo: {
    kick: [1, 0, 0, 0, 1, 0, 0.75, 0, 1, 0, 0, 0, 1, 0, 0.75, 0],
    snare: [0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0],
    hat: [0.35, 0, 0.6, 0, 0.35, 0, 0.6, 0, 0.35, 0, 0.6, 0, 0.35, 0, 0.6, 0],
    bass: [0, _, _, 0, _, _, 7, _, 0, _, _, 0, _, 5, _, 3],
  },
};

const section = (start, end, bpm, pattern) => ({ start, end, bpm, ...PATTERNS[pattern], pattern });

/** Fixture catalog. `skip` = [startMs, endMs) ranges where the 60 Hz clock is paused (rAF stall). */
export const CATALOG = [
  {
    name: 'bpm094_hiphop',
    description: '94 BPM hip-hop groove (syncopated kick, backbeat snare)',
    spec: { sampleRate: 48000, duration: 24, gain: 0.6, seed: 94, sections: [section(0, 24, 94, 'hiphop')] },
  },
  {
    name: 'bpm120_four_floor',
    description: '120 BPM four-on-the-floor with offbeat hats and bass',
    spec: { sampleRate: 48000, duration: 24, gain: 0.6, seed: 120, sections: [section(0, 24, 120, 'fourFloor')] },
  },
  {
    name: 'bpm140_rock',
    description: '140 BPM rock beat (kick 1 and 3-and, snare 2 and 4, straight eighths)',
    spec: { sampleRate: 48000, duration: 24, gain: 0.6, seed: 140, sections: [section(0, 24, 140, 'rock')] },
  },
  {
    name: 'bpm175_dnb',
    description: '175 BPM drum and bass two-step (likely half-tempo lock)',
    spec: { sampleRate: 48000, duration: 24, gain: 0.6, seed: 175, sections: [section(0, 24, 175, 'dnb')] },
  },
  {
    name: 'tempo_change_100_128',
    description: 'Demo pattern at 100 BPM, switching to 128 BPM at 12 s',
    spec: {
      sampleRate: 48000,
      duration: 26,
      gain: 0.6,
      seed: 7,
      sections: [section(0, 12, 100, 'demo'), section(12, 26, 128, 'demo')],
    },
  },
  {
    name: 'silence_gap',
    description: '120 BPM four-on-the-floor, 1.5 s digital silence (unlock), then 108 BPM rock',
    spec: {
      sampleRate: 48000,
      duration: 24,
      gain: 0.6,
      seed: 3,
      sections: [section(0, 10, 120, 'fourFloor'), section(11.5, 24, 108, 'rock')],
    },
    recordBytes: true,
  },
  {
    name: 'quiet_demo112',
    description: 'Demo pattern at 112 BPM, master gain 0.004 (about -48 dB)',
    spec: { sampleRate: 48000, duration: 24, gain: 0.004, seed: 112, sections: [section(0, 24, 112, 'demo')] },
  },
  {
    name: 'stall_128',
    description: '128 BPM four-on-the-floor; the 60 Hz clock stalls for 350 ms at 12 s (gap > 200 ms path)',
    spec: { sampleRate: 48000, duration: 24, gain: 0.6, seed: 128, sections: [section(0, 24, 128, 'fourFloor')] },
    skip: [[12000, 12350]],
  },
];

export function synthesize(spec) {
  const sr = spec.sampleRate;
  const total = Math.round(spec.duration * sr);
  const acc = new Float64Array(total);
  const rng = new XorShift32(spec.seed);
  for (const sec of spec.sections) {
    const stepSec = 60 / sec.bpm / 4;
    for (let i = 0; ; i++) {
      const t = sec.start + i * stepSec;
      if (t >= sec.end) break;
      const at = Math.floor(t * sr);
      const s = i % 16;
      if (sec.kick[s] > 0) kick(acc, at, sec.kick[s], sr);
      if (sec.snare[s] > 0) snare(acc, at, sec.snare[s], sr, rng);
      if (sec.hat[s] > 0) hat(acc, at, sec.hat[s], sr, rng);
      if (sec.bass[s] !== null) bass(acc, at, sec.bass[s], sr);
    }
  }
  const out = new Float32Array(total);
  for (let n = 0; n < total; n++) {
    let v = acc[n] * spec.gain;
    if (v > 1) v = 1;
    if (v < -1) v = -1;
    out[n] = v;
  }
  return out;
}

function kick(acc, at, vel, sr) {
  const len = Math.floor(0.32 * sr);
  const attack = 0.004 * sr;
  let phase = 0;
  for (let n = 0; n < len && at + n < acc.length; n++) {
    const t = n / sr;
    const env = (n < attack ? n / attack : 1) * dexp(-t / 0.13);
    acc[at + n] += vel * 0.9 * env * sinCycles(phase);
    const f = 46 + 114 * dexp(-t / 0.035);
    phase += f / sr;
    phase -= Math.floor(phase);
  }
}

function snare(acc, at, vel, sr, rng) {
  const len = Math.floor(0.2 * sr);
  let phase = 0;
  for (let n = 0; n < len && at + n < acc.length; n++) {
    const t = n / sr;
    const noise = rng.nextSigned();
    const v = 0.45 * noise * dexp(-t / 0.045) + 0.3 * sinCycles(phase) * dexp(-t / 0.03);
    acc[at + n] += vel * v;
    phase += 185 / sr;
    phase -= Math.floor(phase);
  }
}

function hat(acc, at, vel, sr, rng) {
  const len = Math.floor(0.05 * sr);
  let prev = 0;
  for (let n = 0; n < len && at + n < acc.length; n++) {
    const t = n / sr;
    const noise = rng.nextSigned();
    acc[at + n] += vel * 0.12 * (noise - prev) * dexp(-t / 0.012);
    prev = noise;
  }
}

function bass(acc, at, semi, sr) {
  const len = Math.floor(0.24 * sr);
  const freq = 55 * dexp((semi / 12) * LN2);
  const attack = 0.01 * sr;
  let phase = 0;
  let y = 0;
  for (let n = 0; n < len && at + n < acc.length; n++) {
    const t = n / sr;
    const saw = 2 * phase - 1;
    y += 0.06 * (saw - y);
    const env = (n < attack ? n / attack : 1) * dexp(-t / 0.09);
    acc[at + n] += 0.35 * y * env;
    phase += freq / sr;
    phase -= Math.floor(phase);
  }
}
