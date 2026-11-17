import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  BASELINE_ASAR_PATH,
  extractBaselineAudioSlices,
  readFileFromAsar,
  sha256Utf8,
} from './extract-and-freeze-audio.mjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = path.resolve(__dirname, '..');

function extractBetweenMarkers(source, startMarker, endMarker, label) {
  const startIdx = source.indexOf(startMarker);
  const endIdx = source.indexOf(endMarker, startIdx + startMarker.length);
  if (startIdx < 0 || endIdx < 0) {
    throw new Error(`Missing markers ${startMarker} / ${endMarker} for ${label}`);
  }
  return source.slice(startIdx + startMarker.length, endIdx);
}

function extractConfigureMediaPermissions(source, label) {
  const startMarker = 'function configureMediaPermissions()';
  const startIdx = source.indexOf(startMarker);
  if (startIdx < 0) {
    throw new Error(`Missing configureMediaPermissions() in ${label}`);
  }
  const openBraceIdx = source.indexOf('{', startIdx);
  if (openBraceIdx < 0) {
    throw new Error(`Malformed configureMediaPermissions() in ${label}`);
  }
  let depth = 0;
  let inSingle = false;
  let inDouble = false;
  let inTemplate = false;
  let escaped = false;

  for (let i = openBraceIdx; i < source.length; i++) {
    const ch = source[i];
    if (escaped) {
      escaped = false;
      continue;
    }
    if (ch === '\\') {
      escaped = true;
      continue;
    }
    if (inSingle) {
      if (ch === "'") inSingle = false;
      continue;
    }
    if (inDouble) {
      if (ch === '"') inDouble = false;
      continue;
    }
    if (inTemplate) {
      if (ch === '`') inTemplate = false;
      continue;
    }
    if (ch === "'") {
      inSingle = true;
      continue;
    }
    if (ch === '"') {
      inDouble = true;
      continue;
    }
    if (ch === '`') {
      inTemplate = true;
      continue;
    }
    if (ch === '{') depth++;
    else if (ch === '}') {
      depth--;
      if (depth === 0) {
        return source.slice(startIdx, i + 1);
      }
    }
  }
  throw new Error(`Unclosed configureMediaPermissions() block in ${label}`);
}

function checkAudioIsolation(audioDir) {
  const forbiddenPatterns = [/\bgui\b/i, /\bcharacter\b/i, /\bDeskCharacterScene\b/, /\breact\b/i];
  const entries = fs.readdirSync(audioDir);
  for (const entry of entries) {
    const full = path.join(audioDir, entry);
    if (!fs.statSync(full).isFile()) continue;
    const content = fs.readFileSync(full, 'utf8');
    const importLines = content.split(/\r?\n/).filter((line) => /^\s*(import|export)\b.*from\b/.test(line));
    for (const line of importLines) {
      for (const pat of forbiddenPatterns) {
        if (pat.test(line)) {
          throw new Error(`Forbidden import in ${entry}: ${line}`);
        }
      }
    }
  }
}

function main() {
  const { m1Info: baseM1, engineInfo: baseEngine } = extractBaselineAudioSlices(BASELINE_ASAR_PATH);
  const baseMainJs = readFileFromAsar(BASELINE_ASAR_PATH, ['electron', 'main.cjs']).toString('utf8');
  const basePermCode = extractConfigureMediaPermissions(baseMainJs, 'baseline electron/main.cjs');

  const frozenPath = path.join(PROJECT_ROOT, 'src', 'audio', 'frozen-audio-core.js');
  const curFrozenJs = fs.readFileSync(frozenPath, 'utf8');
  const curM1Code = extractBetweenMarkers(
    curFrozenJs,
    '/* FROZEN_M1_START */',
    '/* FROZEN_M1_END */',
    'current m1'
  );
  const curEngineCode = extractBetweenMarkers(
    curFrozenJs,
    '/* FROZEN_ENGINE_START */',
    '/* FROZEN_ENGINE_END */',
    'current engine'
  );

  const curMainPath = path.join(PROJECT_ROOT, 'electron', 'main.cjs');
  const curMainJs = fs.readFileSync(curMainPath, 'utf8');
  const curPermCode = extractConfigureMediaPermissions(curMainJs, 'current electron/main.cjs');

  const checks = [
    {
      name: 'm1 (computeLockedBeatNod)',
      baseBytes: baseM1.byteLen,
      curBytes: Buffer.byteLength(curM1Code, 'utf8'),
      baseSha: baseM1.sha256,
      curSha: sha256Utf8(curM1Code),
    },
    {
      name: 'audio-engine (x1/A1/E1/jh)',
      baseBytes: baseEngine.byteLen,
      curBytes: Buffer.byteLength(curEngineCode, 'utf8'),
      baseSha: baseEngine.sha256,
      curSha: sha256Utf8(curEngineCode),
    },
    {
      name: 'configureMediaPermissions()',
      baseBytes: Buffer.byteLength(basePermCode, 'utf8'),
      curBytes: Buffer.byteLength(curPermCode, 'utf8'),
      baseSha: sha256Utf8(basePermCode),
      curSha: sha256Utf8(curPermCode),
    },
  ];

  let allMatch = true;
  console.log('=== Frozen Audio & Media Permissions Verification ===');
  console.log('Baseline app.asar:', BASELINE_ASAR_PATH);
  for (const c of checks) {
    const ok = c.baseBytes === c.curBytes && c.baseSha === c.curSha;
    if (!ok) allMatch = false;
    console.log(
      `[${ok ? 'PASS' : 'FAIL'}] ${c.name}\n` +
        `  baseline: ${c.baseBytes} bytes | sha256=${c.baseSha}\n` +
        `  current : ${c.curBytes} bytes | sha256=${c.curSha}`
    );
  }

  checkAudioIsolation(path.join(PROJECT_ROOT, 'src', 'audio'));
  console.log('[PASS] src/audio/ isolation check (no GUI/character/react dependencies)');

  if (!allMatch) {
    console.error('[ERROR] Frozen verification failed: byte length or SHA-256 mismatch detected.');
    process.exit(1);
  }
  console.log('[OK] All frozen audio slices and configureMediaPermissions() match baseline 100%.');
}

main();
