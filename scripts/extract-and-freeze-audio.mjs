import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = path.resolve(__dirname, '..');

export const BASELINE_ASAR_PATH =
  process.env.BASELINE_ASAR_PATH ||
  'D:\\kataomoi_pj\\桌宠节奏师-Windows-x64-分享版-20261113\\桌宠节奏师\\resources\\app.asar';

export const EXPECTED_M1 = {
  charLen: 251,
  byteLen: 251,
  sha256: 'b3da3f61ab680eec9b09525ec79673117ca91e63f982dee1356d6a0db2d59161',
};

export const EXPECTED_ENGINE = {
  charLen: 21306,
  byteLen: 21452,
  sha256: 'c2a5579e3ac8ca5a7ec694d8a0fdea859725a894d05764134622494f825cfd0c',
};

export function readFileFromAsar(asarPath, relParts) {
  if (!fs.existsSync(asarPath)) {
    throw new Error(`Baseline app.asar not found: ${asarPath}`);
  }
  const buf = fs.readFileSync(asarPath);
  if (buf.length < 16) {
    throw new Error(`Invalid app.asar header (too small): ${asarPath}`);
  }
  const hdrSize = buf.readUInt32LE(4);
  const jsonLen = buf.readUInt32LE(12);
  const dataOffset = 8 + hdrSize;
  const headerJson = buf.subarray(16, 16 + jsonLen).toString('utf8');
  let node = JSON.parse(headerJson);

  for (const part of relParts) {
    if (!node.files || !Object.prototype.hasOwnProperty.call(node.files, part)) {
      throw new Error(`Entry not found in app.asar: ${relParts.join('/')}`);
    }
    node = node.files[part];
  }
  if (typeof node.size !== 'number' || node.offset === undefined) {
    throw new Error(`Entry is not a regular file in app.asar: ${relParts.join('/')}`);
  }
  const offset = dataOffset + Number.parseInt(String(node.offset), 10);
  return buf.subarray(offset, offset + node.size);
}

export function sha256Utf8(text) {
  return crypto.createHash('sha256').update(text, 'utf8').digest('hex');
}

export function extractBaselineAudioSlices(asarPath = BASELINE_ASAR_PATH) {
  const html = readFileFromAsar(asarPath, ['dist', 'index.html']).toString('utf8');
  const scriptMatches = [...html.matchAll(/<script[^>]*>([\s\S]*?)<\/script>/g)];
  if (scriptMatches.length === 0) {
    throw new Error('No <script> block found in baseline dist/index.html');
  }
  const script = scriptMatches.map((m) => m[1]).find((s) => s.includes('function m1(') && s.includes('const Lt=10,Ie=1600'));
  if (!script) {
    throw new Error('Could not locate target audio <script> block in baseline dist/index.html');
  }

  const m1Start = script.indexOf('function m1(');
  const m1End = script.indexOf('const Uh=', m1Start);
  if (m1Start < 0 || m1End < 0 || m1End <= m1Start) {
    throw new Error(`Invalid m1 slice boundaries: start=${m1Start}, end=${m1End}`);
  }
  const m1Code = script.slice(m1Start, m1End);

  const engStart = script.indexOf('const Lt=10,Ie=1600');
  const engEnd = script.indexOf(',T1=lt.memo(v1)', engStart);
  if (engStart < 0 || engEnd < 0 || engEnd <= engStart) {
    throw new Error(`Invalid engine slice boundaries: start=${engStart}, end=${engEnd}`);
  }
  const engineCode = script.slice(engStart, engEnd);

  const m1Info = {
    code: m1Code,
    charLen: m1Code.length,
    byteLen: Buffer.byteLength(m1Code, 'utf8'),
    sha256: sha256Utf8(m1Code),
  };
  const engineInfo = {
    code: engineCode,
    charLen: engineCode.length,
    byteLen: Buffer.byteLength(engineCode, 'utf8'),
    sha256: sha256Utf8(engineCode),
  };

  if (m1Info.sha256 !== EXPECTED_M1.sha256 || m1Info.byteLen !== EXPECTED_M1.byteLen) {
    throw new Error(
      `M1 hash/length mismatch! expected sha256=${EXPECTED_M1.sha256} (${EXPECTED_M1.byteLen}B), got sha256=${m1Info.sha256} (${m1Info.byteLen}B)`
    );
  }
  if (engineInfo.sha256 !== EXPECTED_ENGINE.sha256 || engineInfo.charLen !== EXPECTED_ENGINE.charLen) {
    throw new Error(
      `Engine hash/length mismatch! expected sha256=${EXPECTED_ENGINE.sha256} (${EXPECTED_ENGINE.charLen} chars), got sha256=${engineInfo.sha256} (${engineInfo.charLen} chars)`
    );
  }

  return { m1Info, engineInfo };
}

function main() {
  const { m1Info, engineInfo } = extractBaselineAudioSlices(BASELINE_ASAR_PATH);
  const output = [
    '/* DO NOT EDIT: Byte-exact frozen audio & beat detection core extracted from baseline app.asar */',
    `/* FROZEN_M1_START */${m1Info.code}/* FROZEN_M1_END */`,
    `/* FROZEN_ENGINE_START */${engineInfo.code}/* FROZEN_ENGINE_END */`,
    'export { m1, x1, A1, E1, jh };',
    '',
  ].join('\n');

  const outPath = path.join(PROJECT_ROOT, 'src', 'audio', 'frozen-audio-core.js');
  fs.mkdirSync(path.dirname(outPath), { recursive: true });
  fs.writeFileSync(outPath, output, 'utf8');

  console.log('[extract-and-freeze-audio] Wrote:', outPath);
  console.log(
    `[extract-and-freeze-audio] m1: ${m1Info.charLen} chars / ${m1Info.byteLen} bytes, sha256=${m1Info.sha256}`
  );
  console.log(
    `[extract-and-freeze-audio] engine: ${engineInfo.charLen} chars / ${engineInfo.byteLen} bytes, sha256=${engineInfo.sha256}`
  );
}

const isCli = process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isCli) {
  main();
}
