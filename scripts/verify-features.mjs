import assert from 'node:assert/strict';
import fs from 'node:fs';
import ts from 'typescript';
import { execFileSync } from 'node:child_process';

async function loadTs(file) {
  const source = fs.readFileSync(new URL(file, import.meta.url), 'utf8');
  const { outputText } = ts.transpileModule(source, {
    compilerOptions: { module: ts.ModuleKind.ESNext, target: ts.ScriptTarget.ES2022 },
  });
  return import(`data:text/javascript;base64,${Buffer.from(outputText).toString('base64')}`);
}
const { timerValue, createCompletionSound } = await loadTs('../src/state/timer.ts');
assert.equal(timerValue('countdown', 10000, 4000), 6000);
assert.equal(timerValue('countdown', 10000, 12000), 0);
assert.equal(timerValue('stopwatch', 4000, 10000), 6000);
// A paused value becomes the new anchor on resume, preserving milliseconds.
const paused = timerValue('stopwatch', 4000, 10000);
assert.equal(timerValue('stopwatch', 30000 - paused, 31000), 7000);
assert.equal(timerValue('countdown', 30000 + paused, 31000), 5000);

const { parseExpressions, legacyExpressions } = await loadTs('../src/character/expressions.ts');
const config = JSON.parse(fs.readFileSync(new URL('../public/character_assets/expressions.json', import.meta.url)));
const list = parseExpressions(config);
assert.equal(list.length, 7);
assert.ok(list.some((entry) => entry.type === 'accessory'));
for (const resources of list.map((entry) => entry.resources)) {
  for (const filename of Object.values(resources)) {
    assert.ok(fs.existsSync(new URL(`../public/character_assets/${filename}`, import.meta.url)));
  }
}
assert.throws(() => parseExpressions({ expressions: [...config.expressions, config.expressions[0]] }));
assert.throws(() => parseExpressions({ expressions: [] }));
assert.throws(() => parseExpressions({ expressions: [{ ...config.expressions[0], resources: { animation: '../outside.png' } }] }));
const manifest = JSON.parse(fs.readFileSync(new URL('../public/character_assets/manifest.json', import.meta.url)));
assert.equal(legacyExpressions(manifest).length, 7);

// Inspect user-visible JSX text, keeping identifier names and CSS untouched.
for (const file of [...fs.readdirSync('src/gui').filter((name) => name.endsWith('.tsx')).map((name) => `src/gui/${name}`), 'src/App.tsx']) {
  const source = ts.createSourceFile(file, fs.readFileSync(file, 'utf8'), ts.ScriptTarget.Latest, true, ts.ScriptKind.TSX);
  const visit = (node) => {
    if (ts.isJsxText(node)) {
      const text = node.text.replace(/Ctrl|Alt|[QHLC]/g, '');
      assert.ok(!/[A-Za-z]{2,}/.test(text), `界面残留英文：${file} ${text.trim()}`);
    }
    ts.forEachChild(node, visit);
  };
  visit(source);
}

// Observe sound generation with a small AudioContext double, without speakers.
let oscillators = 0;
let closed = false;
globalThis.AudioContext = class {
  currentTime = 0;
  destination = {};
  resume() { return Promise.resolve(); }
  close() { closed = true; return Promise.resolve(); }
  createOscillator() {
    oscillators++;
    return { frequency: { value: 0 }, connect: () => ({ connect() {} }), start() {}, stop() {} };
  }
  createGain() { return { gain: { setValueAtTime() {}, linearRampToValueAtTime() {}, exponentialRampToValueAtTime() {} } }; }
};
const sound = createCompletionSound();
await sound.play();
assert.equal(oscillators, 0);
sound.prepare();
await sound.play();
assert.equal(oscillators, 3);
sound.dispose();
assert.equal(closed, true);

// The original distribution is optional; ensure this change leaves the frozen
// core and permission handler byte-for-byte identical to the repository base.
const currentCore = fs.readFileSync('src/audio/frozen-audio-core.js', 'utf8');
const originalCore = execFileSync('git', ['show', 'HEAD:src/audio/frozen-audio-core.js'], { encoding: 'utf8' });
for (const marker of ['M1', 'ENGINE']) {
  const slice = (source) => source.split(`/* FROZEN_${marker}_START */`)[1].split(`/* FROZEN_${marker}_END */`)[0];
  assert.ok(slice(currentCore) === slice(originalCore), `冻结的 ${marker} 核心发生变化`);
}
const before = execFileSync('git', ['show', 'HEAD:electron/main.cjs'], { encoding: 'utf8' });
const after = fs.readFileSync('electron/main.cjs', 'utf8');
const permissions = (source) => source.slice(source.indexOf('function configureMediaPermissions()'), source.indexOf('function isViteServerReachable')).trim().replace(/\r\n/g, '\n');
assert.equal(permissions(after), permissions(before));
console.log('通过：双向计时、暂停续计、表情配置与资源、提示音、音频核心和权限逻辑保持不变。');
