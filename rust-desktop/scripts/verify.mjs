import assert from 'node:assert/strict';
import fs from 'node:fs';
import ts from 'typescript';
import crypto from 'node:crypto';

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


const baseline = JSON.parse(fs.readFileSync('scripts/ui-baseline.json'));
for (const [file, hash] of Object.entries(baseline)) {
  assert.equal(crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex'), hash, '原有界面或核心文件发生变化：' + file);
}
assert.ok(!JSON.stringify(JSON.parse(fs.readFileSync('package.json'))).includes('electron'));
console.log('通过：双向计时、表情配置、提示音、中文界面及原有界面/素材/24 fps/节拍核心一致性。');
