// Deterministic numeric helpers shared (by transliteration) with the Rust side.
//
// Everything here uses only IEEE-754 basic operations (+ - * / sqrt floor) and
// 32-bit integer ops, never Math.sin/cos/exp/log. Those are exactly rounded, so
// the Rust copies (crates/beat/src/fft.rs and crates/beat/tests/common/synth.rs)
// produce bit-identical results on every platform. Keep both sides in sync.

/** exp(x) via halving + Taylor + squaring. Accurate to ~1e-14 for |x| <= 50. */
export function dexp(x) {
  let k = 0;
  while (x > 0.5 || x < -0.5) {
    x *= 0.5;
    k++;
  }
  let term = 1;
  let sum = 1;
  for (let i = 1; i <= 14; i++) {
    term = (term * x) / i;
    sum += term;
  }
  for (let j = 0; j < k; j++) sum *= sum;
  return sum;
}

/** sin(2π·p) for p in [0, 1). */
export function sinCycles(p) {
  let sign = 1;
  if (p >= 0.5) {
    p -= 0.5;
    sign = -1;
  }
  if (p > 0.25) p = 0.5 - p;
  const x = p * 6.283185307179586;
  const x2 = x * x;
  let term = x;
  let sum = x;
  for (let i = 1; i <= 8; i++) {
    term = (-term * x2) / (2 * i * (2 * i + 1));
    sum += term;
  }
  return sign * sum;
}

/** xorshift32; nextSigned() is uniform in [-1, 1). */
export class XorShift32 {
  constructor(seed) {
    this.s = seed >>> 0 || 0x9e3779b9;
  }
  nextU32() {
    let x = this.s;
    x = (x ^ (x << 13)) >>> 0;
    x = (x ^ (x >>> 17)) >>> 0;
    x = (x ^ (x << 5)) >>> 0;
    this.s = x;
    return x;
  }
  nextSigned() {
    return (this.nextU32() / 4294967296) * 2 - 1;
  }
}

/** 32-bit FNV-1a over a byte array, as 8 lowercase hex digits. */
export function fnv1a(bytes) {
  let h = 0x811c9dc5;
  for (let i = 0; i < bytes.length; i++) {
    h ^= bytes[i];
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h.toString(16).padStart(8, '0');
}

/**
 * cos/sin(2πk/n) for k in [0, n), n a power of two >= 8, built from exact
 * operations only: half-angle recursion for the base angles 2π/2^j, products
 * over the set bits of k for the first octant, then exact symmetries.
 * Mirrors yume_beat::fft::UnitRoots.
 */
export function unitRoots(n) {
  let levels = 0;
  while (1 << levels < n) levels++;
  const bc = new Float64Array(levels + 1);
  const bs = new Float64Array(levels + 1);
  bc[0] = 1;
  bs[0] = 0;
  if (levels >= 1) {
    bc[1] = -1;
    bs[1] = 0;
  }
  if (levels >= 2) {
    bc[2] = 0;
    bs[2] = 1;
  }
  for (let j = 3; j <= levels; j++) {
    const c = Math.sqrt((1 + bc[j - 1]) / 2);
    bc[j] = c;
    bs[j] = bs[j - 1] / (2 * c);
  }
  const cos = new Float64Array(n);
  const sin = new Float64Array(n);
  const eighth = n / 8;
  const quarter = n / 4;
  const half = n / 2;
  for (let k = 0; k <= eighth; k++) {
    let c = 1;
    let s = 0;
    for (let b = 0; 1 << b <= k; b++) {
      if (k & (1 << b)) {
        const j = levels - b;
        const nc = c * bc[j] - s * bs[j];
        const ns = c * bs[j] + s * bc[j];
        c = nc;
        s = ns;
      }
    }
    cos[k] = c;
    sin[k] = s;
  }
  for (let k = eighth + 1; k <= quarter; k++) {
    cos[k] = sin[quarter - k];
    sin[k] = cos[quarter - k];
  }
  for (let k = quarter + 1; k <= half; k++) {
    cos[k] = -sin[k - quarter];
    sin[k] = cos[k - quarter];
  }
  for (let k = half + 1; k < n; k++) {
    cos[k] = -cos[k - half];
    sin[k] = -sin[k - half];
  }
  return { cos, sin };
}

/** In-place iterative radix-2 DIT FFT (forward, e^{-2πi kn/N}). Mirrors yume_beat::fft. */
export function fft(re, im, roots) {
  const n = re.length;
  for (let i = 1, j = 0; i < n; i++) {
    let bit = n >> 1;
    for (; j & bit; bit >>= 1) j ^= bit;
    j ^= bit;
    if (i < j) {
      let t = re[i];
      re[i] = re[j];
      re[j] = t;
      t = im[i];
      im[i] = im[j];
      im[j] = t;
    }
  }
  for (let size = 2; size <= n; size <<= 1) {
    const halfSize = size >> 1;
    const step = n / size;
    for (let start = 0; start < n; start += size) {
      for (let j = 0; j < halfSize; j++) {
        const wr = roots.cos[j * step];
        const wi = -roots.sin[j * step];
        const a = start + j;
        const b = a + halfSize;
        const tr = wr * re[b] - wi * im[b];
        const ti = wr * im[b] + wi * re[b];
        re[b] = re[a] - tr;
        im[b] = im[a] - ti;
        re[a] = re[a] + tr;
        im[a] = im[a] + ti;
      }
    }
  }
}
