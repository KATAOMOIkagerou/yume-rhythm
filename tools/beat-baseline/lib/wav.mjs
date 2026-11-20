// Minimal WAV reader: PCM 16/24/32-bit, IEEE float 32/64-bit, WAVE_FORMAT_EXTENSIBLE.
// Down-mixes to mono exactly like crates/beat/tests/common/wav.rs:
//   mono[i] = f32( (ch0 + ch1 + ...) / channels ), integer PCM scaled by 1 / 2^(bits-1).

import fs from 'node:fs';

export function readWav(path) {
  const buf = fs.readFileSync(path);
  const view = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
  const tag = (o) => buf.toString('latin1', o, o + 4);
  if (tag(0) !== 'RIFF' || tag(8) !== 'WAVE') throw new Error(`${path}: not a RIFF/WAVE file`);
  let fmt = null;
  let data = null;
  for (let o = 12; o + 8 <= buf.length; ) {
    const id = tag(o);
    const size = view.getUint32(o + 4, true);
    const body = o + 8;
    if (id === 'fmt ') {
      let format = view.getUint16(body, true);
      const channels = view.getUint16(body + 2, true);
      const sampleRate = view.getUint32(body + 4, true);
      const bits = view.getUint16(body + 14, true);
      if (format === 0xfffe && size >= 40) format = view.getUint16(body + 24, true);
      fmt = { format, channels, sampleRate, bits };
    } else if (id === 'data') {
      data = { offset: body, size: Math.min(size, buf.length - body) };
    }
    o = body + size + (size & 1);
  }
  if (!fmt || !data) throw new Error(`${path}: missing fmt or data chunk`);
  const { format, channels, sampleRate, bits } = fmt;
  const bytesPer = bits / 8;
  const frames = Math.floor(data.size / (bytesPer * channels));
  let read;
  if (format === 1 && bits === 16) read = (o) => view.getInt16(o, true) / 32768;
  else if (format === 1 && bits === 24)
    read = (o) => ((view.getUint8(o) | (view.getUint8(o + 1) << 8) | (view.getInt8(o + 2) << 16)) / 8388608);
  else if (format === 1 && bits === 32) read = (o) => view.getInt32(o, true) / 2147483648;
  else if (format === 3 && bits === 32) read = (o) => view.getFloat32(o, true);
  else if (format === 3 && bits === 64) read = (o) => view.getFloat64(o, true);
  else throw new Error(`${path}: unsupported WAV format ${format} / ${bits} bit`);
  const mono = new Float32Array(frames);
  for (let i = 0; i < frames; i++) {
    let sum = 0;
    for (let c = 0; c < channels; c++) sum += read(data.offset + (i * channels + c) * bytesPer);
    mono[i] = sum / channels;
  }
  return { sampleRate, channels, bits, format, samples: mono };
}
