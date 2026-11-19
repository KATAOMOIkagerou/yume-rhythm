import fs from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import electron from 'electron';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
fs.mkdirSync(path.join(root, '.tmp'), { recursive: true });
const scratch = fs.mkdtempSync(path.join(root, '.tmp', 'desktop-check-'));

async function driver(pet, app) {
  const { BrowserWindow } = require('electron');
  const fs = require('node:fs');
  const path = require('node:path');
  const assert = (value, message) => { if (!value) throw new Error(message); };
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const wait = async (check, label, timeout = 12000) => {
    const until = Date.now() + timeout;
    while (Date.now() < until) { const result = await check(); if (result) return result; await sleep(100); }
    throw new Error(`等待超时：${label}`);
  };
  const errors = [];
  const observeConsole = (window) => window.webContents.on('console-message', (event) => {
    if (event.level === 'error') errors.push(event.message);
  });
  observeConsole(pet);
  pet.webContents.on('render-process-gone', (_event, details) => errors.push(JSON.stringify(details)));
  const petJs = (code) => pet.webContents.executeJavaScript(code, true).catch((error) => { console.error('桌宠脚本失败：', code, error.message); throw error; });
  let panel;
  let step = '打开控制台';
  const panelJs = (code) => panel.webContents.executeJavaScript(code, true).catch((error) => { console.error('控制台脚本失败：', code, error.message); throw error; });
  const click = (id) => panelJs(`document.querySelector('[data-testid="${id}"]').click()`);
  const text = (id) => panelJs(`document.querySelector('[data-testid="${id}"]')?.textContent`);
  const change = (id, value) => panelJs(`(() => {
    const input = document.querySelector('[data-testid="${id}"]');
    const proto = input.tagName === 'SELECT' ? HTMLSelectElement.prototype : HTMLInputElement.prototype;
    Object.getOwnPropertyDescriptor(proto, 'value').set.call(input, ${JSON.stringify(value)});
    input.dispatchEvent(new Event(input.tagName === 'SELECT' ? 'change' : 'input', { bubbles: true }));
  })()`);
  try {
    panel = await wait(() => BrowserWindow.getAllWindows().find((item) => item !== pet), '独立控制台窗口');
    observeConsole(panel);
    for (const target of [pet, panel]) await target.webContents.executeJavaScript(`window.__checkErrors = [];
      addEventListener('error', event => window.__checkErrors.push(String(event.error?.stack || event.message)));
      addEventListener('unhandledrejection', event => window.__checkErrors.push(String(event.reason?.stack || event.reason)));`);
    await wait(() => text('tab-status'), '控制台内容');
    await wait(() => petJs(`document.querySelector('[data-testid="layer-eyes"]')` + `?.getAttribute('src')`), '角色图层');
    assert(/^#000000(?:00)?$/i.test(pet.getBackgroundColor()), `主窗口底色必须透明：${pet.getBackgroundColor()}`);
    step = '验证透明网页与角色素材';
    assert(!pet.isResizable(), '桌宠窗口不应改变大小');
    assert(await petJs(`getComputedStyle(document.body).backgroundColor === 'rgba(0, 0, 0, 0)'`), '网页背景必须透明');
    assert(!await petJs(`Boolean(document.querySelector('[data-testid="layer-scene-bg"]'))`), '不能绘制不透明场景背景');

    // Observe native calls while still executing the real platform operation.
    const nativeIgnore = pet.setIgnoreMouseEvents.bind(pet);
    let ignoring = false;
    step = '锁定桌宠';
    pet.setIgnoreMouseEvents = (value, options) => { ignoring = value; nativeIgnore(value, options); };
    await petJs('window.desktopPetBridge.setLocked(true)');
    assert(ignoring && !pet.isMovable(), '锁定应禁止移动并开启点击穿透');
    await click('tab-settings');
    step = '从控制台解锁';
    await wait(() => text('btn-settings-lock').then((value) => value?.includes('解除')), '锁定状态共享');
    await click('btn-settings-lock');
    await wait(() => pet.isMovable(), '控制台解锁');
    assert(!ignoring, '解锁应关闭点击穿透');

    await click('tab-emotion');
    step = '验证基础表情';
    await click('random-enabled');
    await click('emotion-happy');
    await wait(() => petJs(`document.querySelector('[data-testid="character-head-rig"]')?.dataset.emotion === 'happy'`), '开心表情');
    await click('emotion-happy');
    await wait(() => petJs(`document.querySelector('[data-testid="character-head-rig"]')?.dataset.emotion === 'neutral'`), '恢复自然表情');
    await click('emotion-think').catch((error) => { console.error('思考表情点击失败', error.stack); throw error; });
    step = '验证配件表情';
    await wait(() => petJs(`document.querySelector('[data-testid="layer-event-fx"]')?.dataset.assetFile === 'fx_think_bubble.png'`), '配件叠加', 2000);
    assert(await petJs(`document.querySelectorAll('[data-testid="layer-eyes"]').length === 1 && document.querySelectorAll('[data-testid="layer-mouth"]').length === 1`), '眼嘴图层不能重影');
    await click('emotion-think');
    step = '验证画手扩展配置与整体动画';
    await petJs(`window.__fetch = window.fetch; window.fetch = async function(input, init) {
      const response = await window.__fetch(input, init);
      if (!String(input).includes('expressions.json')) return response;
      const config = await response.json();
      config.expressions.push(
        { id:'custom', name:'扩展整体动画', type:'base', resources:{ animation:'head_base.png' }, durationMs:1000, random:false },
        { id:'badge', name:'星光配件', type:'accessory', resources:{ overlay:'fx_love_hearts.png' }, durationMs:1000, random:false }
      );
      return new Response(JSON.stringify(config), { headers:{ 'Content-Type':'application/json' } });
    }; void 0`);
    await click('btn-reload-assets');
    await wait(() => text('emotion-custom'), '自定义表情自动生成选项');
    await click('emotion-custom');
    await wait(() => petJs(`Boolean(document.querySelector('[data-testid="layer-base-animation"]'))`), '整体动画替换主体');
    assert(await petJs(`document.querySelectorAll('[data-testid="layer-eyes"]').length === 0`), '整体动画不能叠加旧五官');
    await click('emotion-badge');
    assert(await petJs(`Boolean(document.querySelector('[data-testid="layer-base-animation"]'))`), '配件不能替换整体动画');
    await click('emotion-happy');
    await click('emotion-badge');
    await wait(() => petJs(`document.querySelector('[data-testid="layer-event-fx"]')?.dataset.assetFile === 'fx_love_hearts.png'`), '自定义配件叠加');
    assert(await petJs(`document.querySelector('[data-testid="layer-eyes"]')?.dataset.assetFile === 'eyes_happy.png'`), '配件应保留所选基础表情');
    await click('emotion-badge');
    await petJs('window.fetch = window.__fetch; void 0');
    await click('btn-reload-assets');
    await wait(() => panelJs(`!document.querySelector('[data-testid="emotion-custom"]')`), '恢复原表情配置');

    // Advance the renderer's wall clock; native polling continues in real time.
    await petJs(`window.__realNow = Date.now; window.__clockOffset = 0; Date.now = () => window.__realNow() + window.__clockOffset;
      window.__oscillators = 0;
      window.__realAudioContext = AudioContext;
      window.AudioContext = class {
        currentTime = 0; destination = {};
        resume() { return Promise.resolve(); } close() { return Promise.resolve(); }
        createOscillator() { window.__oscillators++; return { frequency: { value: 0 }, connect() { return this; }, start() {}, stop() {} }; }
        createGain() { return { gain: { setValueAtTime() {}, linearRampToValueAtTime() {}, exponentialRampToValueAtTime() {} }, connect() { return this; } }; }
      }; void 0`);
    await click('tab-status');
    step = '验证秒表与关闭后计时';
    await change('timer-mode', 'stopwatch');
    assert(await text('timer-display') === '00:00', '正计时应从零开始');
    await click('btn-timer-play');
    await petJs('window.__clockOffset += 5000');
    await wait(() => text('timer-display').then((value) => value !== '00:00'), '正计时增长');
    panel.close();
    assert(!panel.isDestroyed() && !panel.isVisible(), '关闭控制台应隐藏且保留状态');
    await petJs('window.__clockOffset += 5000');
    await sleep(300);
    await petJs('window.desktopPetBridge.showConsole()');
    await wait(() => panel.isVisible(), '重新打开控制台');
    await click('btn-timer-pause');
    step = '验证秒表暂停与续计';
    await wait(() => text('pausing-remaining'), '秒表暂停');
    const paused = await text('timer-display');
    await petJs('window.__clockOffset += 10000');
    await sleep(350);
    assert(await text('timer-display') === paused, '暂停后时间应保持');
    await click('btn-timer-continue');
    await petJs('window.__clockOffset += 5000');
    await wait(() => text('timer-display').then((value) => value !== paused), '秒表续计');
    await click('btn-timer-pause');
    await click('btn-timer-reset');
    await change('timer-mode', 'countdown');
    step = '验证倒计时音效';
    await change('input-study-minutes', '1');
    await click('btn-timer-play');
    await petJs('window.__clockOffset += 61000');
    await wait(() => text('timer-completed-banner'), '倒计时完成');
    const soundCount = await petJs('window.__oscillators');
    assert(soundCount === 3, `倒计时结束应播放一次三音提示，检测到 ${soundCount}`);
    await click('tab-settings');
    await click('setting-end-sound');
    await click('tab-status');
    // Completed timers can be started again directly.
    await click('btn-timer-play');
    await petJs('window.__clockOffset += 61000');
    await wait(() => text('timer-completed-banner'), '静音倒计时完成');
    assert(await petJs('window.__oscillators') === 3, '音效关闭后不应创建提示音');
    await petJs('window.AudioContext = window.__realAudioContext; void 0');
    await click('btn-toggle-todo');
    await change('todo-list-name', '回归测试清单');
    await change('todo-input-0', '验证保存');
    await click('btn-save-list');
    await click('modal-save-yes');
    assert(await petJs(`JSON.parse(localStorage.getItem('desk_pet_todo_list_v1')).listName === '回归测试清单'`), '待办保存应保留');

    await click('tab-metronome');
    step = '检查中文界面与监听设备';
    assert(await panelJs(`!document.querySelector('[data-testid="btn-file"],[data-testid="btn-mic"],[data-testid="btn-demo"]')`), '已移除的音源按钮不能出现');
    assert(await panelJs(`document.querySelector('[data-testid="slider-sensitivity"]').closest('details').open === false`), '灵敏度应默认折叠');
    assert(await panelJs(`document.querySelector('[data-testid="output-device"]').value === 'default'`), '应提供默认输出设备');
    const english = await panelJs(`document.body.innerText.replace(/Ctrl|Alt|[QHLC]/g, '').match(/[A-Za-z]{2,}/g)`);
    assert(!english, '界面存在英文文本：' + english);
    // Exercise the retained real file -> beat detector -> character -> Q path.
    await petJs(`(() => {
      const rate = 44100, length = rate * 14;
      const buffer = new ArrayBuffer(44 + length * 2), view = new DataView(buffer);
      const str = (at, value) => [...value].forEach((char, i) => view.setUint8(at + i, char.charCodeAt(0)));
      str(0,'RIFF'); view.setUint32(4,36+length*2,true); str(8,'WAVE'); str(12,'fmt ');
      view.setUint32(16,16,true); view.setUint16(20,1,true); view.setUint16(22,1,true);
      view.setUint32(24,rate,true); view.setUint32(28,rate*2,true); view.setUint16(32,2,true); view.setUint16(34,16,true);
      str(36,'data'); view.setUint32(40,length*2,true);
      for (let i=0; i<length; i++) {
        const t=i/rate, phase=t%0.5;
        view.setInt16(44+i*2, Math.sin(2*Math.PI*80*t)*Math.exp(-phase*35)*28000,true);
      }
      const dt = new DataTransfer(); dt.items.add(new File([buffer], '节奏测试.wav', { type:'audio/wav' }));
      document.querySelector('.pet-root').dispatchEvent(new DragEvent('drop', { bubbles:true, cancelable:true, dataTransfer:dt }));
    })()`);
    await wait(() => petJs(`document.querySelector('[data-testid="character-scene"]')?.dataset.musicRunning === 'true'`), '保留音频拖入功能');
    await wait(() => text('beat-count').then((value) => /[1-9]/.test(value)), '音频检测核心输出节拍');
    await panelJs(`window.dispatchEvent(new KeyboardEvent('keydown', { key:'q', bubbles:true }))`);
    await wait(() => petJs(`document.querySelector('[data-testid="character-scene"]')?.dataset.musicRunning === 'false'`), '控制台快捷键停止音频');
    // Deterministic random pool: the timer still chooses from the loaded JSON.
    await petJs('window.__random = Math.random; Math.random = () => 0.99; void 0');
    await click('tab-emotion');
    await change('random-interval', '5');
    await click('random-enabled');
    await wait(() => petJs(`document.querySelector('[data-testid="character-head-rig"]')?.dataset.emotion === 'love'`), '定时随机表情', 7000);
    await wait(() => petJs(`document.querySelector('[data-testid="character-head-rig"]')?.dataset.emotion === 'neutral'`), '随机结束恢复状态', 5000);
    await click('random-enabled');
    for (const tab of ['status', 'emotion', 'metronome', 'settings']) {
      await click(`tab-${tab}`);
      await panelJs(`new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve(true))))`);
      await panel.webContents.capturePage().then((shot) => fs.writeFileSync(path.join(process.env.PET_CHECK_DIR, `${tab}.png`), shot.toPNG()));
    }
    await wait(() => text('btn-settings-lock'), '设置页渲染');
    await panel.webContents.capturePage().then((shot) => fs.writeFileSync(path.join(process.env.PET_CHECK_DIR, 'console.png'), shot.toPNG()));
    const petShot = await pet.webContents.capturePage();
    assert(petShot.toBitmap()[3] === 0, '桌宠窗口角落像素必须完全透明');
    fs.writeFileSync(path.join(process.env.PET_CHECK_DIR, 'pet.png'), petShot.toPNG());
    assert(errors.length === 0, '渲染错误：' + errors.join('\n'));
    console.log('通过：透明双窗口、原生锁定与解锁、表情切换和随机恢复、双向计时与音效、待办保存、音频跟拍和快捷键。');
    app.exit(0);
  } catch (error) {
    console.error(`验证步骤：${step}`);
    console.error(error.stack ?? error);
    try { console.error('桌宠页面错误：', await petJs('window.__checkErrors')); } catch {}
    try { console.error('控制台页面错误：', await panelJs('window.__checkErrors')); } catch {}
    console.error('渲染错误：', errors);
    app.exit(1);
  }
}

const driverPath = path.join(scratch, 'driver.cjs');
fs.writeFileSync(driverPath, `module.exports = ${driver.toString()};\n`);
const env = { ...process.env, ELECTRON_FORCE_DIST: '1', PET_DISABLE_GPU: '1', PET_E2E_DRIVER: driverPath, PET_CHECK_DIR: scratch };
delete env.ELECTRON_RUN_AS_NODE;
const child = spawn(electron, ['.', `--user-data-dir=${path.join(scratch, 'profile')}`, '--autoplay-policy=no-user-gesture-required', '--in-process-gpu', '--disable-gpu-compositing'], {
  cwd: root, env, stdio: 'inherit', windowsHide: true,
});
const timeout = setTimeout(() => { child.kill(); process.exitCode = 1; }, 90000);
child.on('exit', (code) => { clearTimeout(timeout); console.log(`验证输出：${scratch}`); process.exitCode = code ?? 1; });
child.on('error', (error) => { clearTimeout(timeout); console.error(error); process.exitCode = 1; });
