// Fake Web Audio AnalyserNode: just enough surface for the frozen detector (x1).
//
// x1 touches exactly three things on the analyser:
//   - analyser.frequencyBinCount        (constructor: size of its Uint8Array)
//   - analyser.context.sampleRate       (process: bin width in Hz)
//   - analyser.getByteFrequencyData(a)  (process: once per call)
//
// getByteFrequencyData follows the Web Audio spec ("Computing the frequency
// data") in f64 and is the exact same math as yume_beat::Analyser:
//   Blackman window (alpha = 0.16) -> FFT -> |X[k]| / fftSize
//   -> smoothing over successive calls: S = tau*S_prev + (1 - tau)*|X|
//   -> dB = 20*log10(S) -> byte = floor(255/(maxDb-minDb)*(dB-minDb)), clamped to 0..255
// Twiddles and window come from unitRoots() (exact operations only), so the
// only libm call is Math.log10.

import { fft, unitRoots } from './dsp.mjs';

const BLACKMAN_ALPHA = 0.16;

export class FakeAnalyser {
  constructor({ sampleRate, fftSize = 2048, smoothing = 0.08, minDb = -90, maxDb = -3 }) {
    this.context = { sampleRate };
    this.fftSize = fftSize;
    this.frequencyBinCount = fftSize / 2;
    this.smoothingTimeConstant = smoothing;
    this.minDecibels = minDb;
    this.maxDecibels = maxDb;
    this.roots = unitRoots(fftSize);
    const a0 = 0.5 * (1 - BLACKMAN_ALPHA);
    const a1 = 0.5;
    const a2 = 0.5 * BLACKMAN_ALPHA;
    this.window = new Float64Array(fftSize);
    for (let n = 0; n < fftSize; n++) {
      this.window[n] = a0 - a1 * this.roots.cos[n] + a2 * this.roots.cos[(2 * n) % fftSize];
    }
    this.smoothed = new Float64Array(fftSize / 2);
    this.re = new Float64Array(fftSize);
    this.im = new Float64Array(fftSize);
    this.samples = new Float32Array(0);
    this.end = 0;
  }

  /** Audio source (mono Float32Array) and the exclusive end sample of the current window. */
  setSource(samples) {
    this.samples = samples;
  }
  setEnd(endSample) {
    this.end = endSample;
  }

  getByteFrequencyData(out) {
    const n = this.fftSize;
    const start = this.end - n;
    for (let i = 0; i < n; i++) {
      const idx = start + i;
      const x = idx >= 0 && idx < this.samples.length ? this.samples[idx] : 0;
      this.re[i] = x * this.window[i];
      this.im[i] = 0;
    }
    fft(this.re, this.im, this.roots);
    const scale = 1 / n;
    const tau = this.smoothingTimeConstant;
    const byteScale = 255 / (this.maxDecibels - this.minDecibels);
    const bins = n / 2;
    for (let k = 0; k < bins; k++) {
      const mag = Math.sqrt(this.re[k] * this.re[k] + this.im[k] * this.im[k]) * scale;
      let s = tau * this.smoothed[k] + (1 - tau) * mag;
      if (!Number.isFinite(s)) s = 0;
      this.smoothed[k] = s;
    }
    const len = Math.min(out.length, bins);
    for (let k = 0; k < len; k++) {
      const db = 20 * Math.log10(this.smoothed[k]);
      let b = Math.floor(byteScale * (db - this.minDecibels));
      if (!(b >= 0)) b = 0;
      if (b > 255) b = 255;
      out[k] = b;
    }
  }
}
