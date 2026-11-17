import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);
const electronExecutable = require('electron');

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const PROJECT_ROOT = path.resolve(__dirname, '..');

const DEFAULT_SCRATCH_ROOT =
  'C:\\Users\\Ys\\.gemini\\antigravity\\brain\\1138bbfc-39e5-471e-ae22-6e139040bf5c\\scratch';
const SCRATCH_ROOT = process.env.PET_E2E_SCRATCH_DIR || DEFAULT_SCRATCH_ROOT;
const SHOT_DIR = process.env.PET_E2E_SHOT_DIR || path.join(SCRATCH_ROOT, 'electron_shots');
const DRIVER_PATH = path.join(SCRATCH_ROOT, 'electron-e2e-driver.cjs');
const USER_DATA_DIR = path.join(SCRATCH_ROOT, 'electron-e2e-userdata');
const REPORT_PATH = path.join(SCRATCH_ROOT, 'electron-e2e-report.json');
const DEMO_SONG_PATH =
  process.env.PET_E2E_DEMO_SONG ||
  'D:\\xwechat_files\\wxid_k9rsdwghjaxr22_f2d4\\msg\\file\\2026-11\\DemoSong.mp3';

function driverMain(win, app) {
  const fs = require('node:fs');
  const path = require('node:path');

  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

  function assert(condition, message) {
    if (!condition) {
      throw new Error('E2E Assertion Failed: ' + message);
    }
  }

  async function waitFor(fn, timeoutMs = 15000, intervalMs = 100, label = 'condition') {
    const start = Date.now();
    let lastError = null;
    while (Date.now() - start < timeoutMs) {
      try {
        const value = await fn();
        if (value) return value;
      } catch (err) {
        lastError = err;
      }
      await sleep(intervalMs);
    }
    const suffix = lastError ? ' (lastError: ' + lastError.message + ')' : '';
    throw new Error('Timed out waiting for ' + label + ' after ' + timeoutMs + 'ms' + suffix);
  }

  const shotDir = process.env.PET_E2E_SHOT_DIR;
  const demoSongPath = process.env.PET_E2E_DEMO_SONG;
  const reportPath = process.env.PET_E2E_REPORT_PATH;

  fs.mkdirSync(shotDir, { recursive: true });

  const consoleErrors = [];
  const failedAssets = [];

  win.webContents.on('console-message', (event, legacyLevel, legacyMessage, legacyLine, legacySourceId) => {
    const level = typeof event?.level === 'number' ? event.level : legacyLevel;
    const message = typeof event?.message === 'string' ? event.message : legacyMessage;
    const line = typeof event?.lineNumber === 'number' ? event.lineNumber : legacyLine;
    const sourceId = typeof event?.sourceId === 'string' ? event.sourceId : legacySourceId;
    if (level >= 3) {
      consoleErrors.push({ level, message, line, sourceId });
    }
  });

  win.webContents.on('did-fail-load', (_event, errorCode, errorDescription, validatedURL) => {
    failedAssets.push({ errorCode, errorDescription, validatedURL });
  });

  const evalPage = (code) => win.webContents.executeJavaScript(code, true);
  const callInPage = (fn, ...args) =>
    evalPage('(' + fn.toString() + ').apply(null, ' + JSON.stringify(args) + ')');

  const captureShot = async (filename) => {
    await callInPage(() => new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r))));
    await sleep(240);
    const img = await win.webContents.capturePage();
    const outPath = path.join(shotDir, filename);
    fs.writeFileSync(outPath, img.toPNG());
    const size = img.getSize();
    return {
      filename,
      outPath,
      width: size.width,
      height: size.height,
      bytes: fs.statSync(outPath).size,
    };
  };

  const clickByTestId = async (testId) => {
    const clicked = await callInPage((id) => {
      const el = document.querySelector('[data-testid="' + id + '"]');
      if (!el) return { ok: false, reason: 'missing' };
      el.scrollIntoView({ block: 'nearest' });
      el.click();
      return { ok: true, disabled: Boolean(el.disabled) };
    }, testId);
    assert(clicked.ok, 'Element not found for click: ' + testId);
    return clicked;
  };

  const setInputValueByTestId = async (testId, value) => {
    const res = await callInPage((id, val) => {
      const el = document.querySelector('[data-testid="' + id + '"]');
      if (!el) return { ok: false };
      const proto =
        el instanceof HTMLTextAreaElement ? HTMLTextAreaElement.prototype : HTMLInputElement.prototype;
      const setter = Object.getOwnPropertyDescriptor(proto, 'value').set;
      setter.call(el, String(val));
      el.dispatchEvent(new Event('input', { bubbles: true }));
      el.dispatchEvent(new Event('change', { bubbles: true }));
      return { ok: true, value: el.value };
    }, testId, value);
    assert(res.ok, 'Input not found: ' + testId);
    return res;
  };

  const sendKeyQ = async () => {
    await callInPage(() => {
      if (document.activeElement && document.activeElement.blur) {
        document.activeElement.blur();
      }
    });
    await sleep(60);
    win.webContents.sendInputEvent({ type: 'keyDown', keyCode: 'Q' });
    win.webContents.sendInputEvent({ type: 'char', keyCode: 'q' });
    win.webContents.sendInputEvent({ type: 'keyUp', keyCode: 'Q' });
  };

  const getText = (testId) =>
    callInPage((id) => {
      const el = document.querySelector('[data-testid="' + id + '"]');
      return el ? el.textContent : null;
    }, testId);

  const report = {
    startedAt: new Date().toISOString(),
    windowTitle: win.getTitle(),
    windowBounds: win.getBounds(),
    shots: [],
    scenarios: {},
  };

  return (async () => {
    try {
      // Wait for character assets & manifest to finish loading in Electron
      await waitFor(
        () =>
          callInPage(() => {
            const status = document.querySelector('[data-testid="asset-status"]');
            const scene = document.querySelector('[data-testid="character-scene"]');
            const head = document.querySelector('[data-testid="character-head-rig"]');
            if (!status || !scene || !head) return false;
            const text = status.textContent || '';
            if (text.includes('正在加载') || text.includes('⚠')) return false;
            const imgs = Array.from(scene.querySelectorAll('img'));
            if (imgs.length < 7) return false;
            return imgs.every((img) => img.complete && img.naturalWidth > 0);
          }),
        15000,
        150,
        'character assets loaded'
      );

      // =========================================================
      // Scenario A: Default UI (Chill mode + Status tab)
      // =========================================================
      const sceneAState = await callInPage(() => {
        const scene = document.querySelector('[data-testid="character-scene"]');
        const status = document.querySelector('[data-testid="asset-status"]');
        const imgs = Array.from(scene.querySelectorAll('img')).map((img) => ({
          testId: img.getAttribute('data-testid'),
          src: img.getAttribute('src'),
          w: img.naturalWidth,
          h: img.naturalHeight,
        }));
        const tabStatus = document.querySelector('[data-testid="tab-status"]');
        const tabEmotion = document.querySelector('[data-testid="tab-emotion"]');
        const tabMetronome = document.querySelector('[data-testid="tab-metronome"]');
        return {
          packName: scene.getAttribute('data-pack-name'),
          activityMode: scene.getAttribute('data-activity-mode'),
          musicRunning: scene.getAttribute('data-music-running'),
          timerActive: scene.getAttribute('data-timer-active'),
          assetStatusText: status ? status.textContent : '',
          hasComputerOff: Boolean(document.querySelector('[data-testid="layer-computer-off"]')),
          hasComputerOn: Boolean(document.querySelector('[data-testid="layer-computer-on"]')),
          hasBooks: Boolean(document.querySelector('[data-testid="layer-books"]')),
          hasArmsChill: Boolean(document.querySelector('[data-testid="layer-arms-chill"]')),
          hasTimerClock: Boolean(document.querySelector('[data-testid="layer-timer-clock"]')),
          hasHeadphones: Boolean(document.querySelector('[data-testid="layer-headphones"]')),
          tabsUnlocked: !tabStatus.disabled && !tabEmotion.disabled && !tabMetronome.disabled,
          imgCount: imgs.length,
          all800x800: imgs.every((i) => i.w === 800 && i.h === 800),
        };
      });

      assert(sceneAState.activityMode === 'chill', 'Expected initial chill mode');
      assert(sceneAState.hasComputerOff && !sceneAState.hasComputerOn, 'Expected computer off in chill mode');
      assert(sceneAState.hasBooks, 'Expected books prop on desk');
      assert(sceneAState.hasArmsChill, 'Expected chill arms');
      assert(!sceneAState.hasTimerClock, 'Expected timer clock hidden before study start');
      assert(!sceneAState.hasHeadphones, 'Expected headphones hidden when music not running');
      assert(sceneAState.tabsUnlocked, 'Expected all 3 tabs unlocked');
      assert(sceneAState.all800x800, 'Expected all character layer PNGs to be 800x800');

      report.scenarios.A = sceneAState;
      report.shots.push(await captureShot('scene_a_default.png'));

      // =========================================================
      // Scenario B: Status Tab Interactive Flow
      // =========================================================
      await setInputValueByTestId('input-study-minutes', '15');
      await sleep(100);
      const presetDisplay = await getText('timer-display');
      assert(presetDisplay === '15:00', 'Expected timer display 15:00, got ' + presetDisplay);

      await clickByTestId('btn-timer-play');
      await waitFor(
        () =>
          callInPage(() => {
            const el = document.querySelector('[data-testid="character-scene"]');
            return el && el.getAttribute('data-activity-mode') === 'study';
          }),
        5000,
        80,
        'study mode active'
      );
      await sleep(1250);

      const studyRunningState = await callInPage(() => {
        const scene = document.querySelector('[data-testid="character-scene"]');
        const timerText = document.querySelector('[data-testid="timer-display"]')?.textContent || '';
        return {
          activityMode: scene.getAttribute('data-activity-mode'),
          timerActive: scene.getAttribute('data-timer-active'),
          hasComputerOn: Boolean(document.querySelector('[data-testid="layer-computer-on"]')),
          hasTimerClock: Boolean(document.querySelector('[data-testid="layer-timer-clock"]')),
          hasArmsStudy: Boolean(document.querySelector('[data-testid="layer-arms-study"]')),
          timerText,
        };
      });
      assert(studyRunningState.hasComputerOn, 'Expected computer-on in study mode');
      assert(studyRunningState.hasTimerClock, 'Expected timer-clock when timer running');
      assert(studyRunningState.hasArmsStudy, 'Expected study arms');
      report.shots.push(await captureShot('scene_b1_study_running.png'));

      // Pause -> Modal 1: Time's not up yet! Do you still want to pause?
      await clickByTestId('btn-timer-pause');
      const modal1Text = await waitFor(() => getText('modal-card'), 4000, 60, 'pause modal 1');
      assert(modal1Text.includes("Time's not up yet!"), 'Missing pause confirm text: ' + modal1Text);
      report.shots.push(await captureShot('scene_b2_modal_pause_confirm.png'));

      // Click No -> Modal 2: Go back to study!
      await clickByTestId('modal-btn-no');
      const modal2Text = await waitFor(
        () =>
          callInPage(() => {
            const el = document.querySelector('[data-testid="modal-card"]');
            return el && el.textContent.includes('Go back to study!') ? el.textContent : null;
          }),
        4000,
        60,
        'go back to study modal'
      );
      report.shots.push(await captureShot('scene_b3_modal_go_back.png'));

      // Click Okay -> continue studying
      await clickByTestId('modal-btn-okay');
      await waitFor(
        () => callInPage(() => !document.querySelector('[data-testid="modal-card"]')),
        4000,
        60,
        'modal closed'
      );

      // Pause again -> Click Yes -> pausing... striped bar
      await clickByTestId('btn-timer-pause');
      await waitFor(() => getText('modal-btn-yes'), 4000, 60, 'pause modal yes btn');
      await clickByTestId('modal-btn-yes');
      await waitFor(() => getText('pausing-bar'), 4000, 60, 'pausing bar');

      const pausedRemaining1 = await getText('pausing-remaining');
      await sleep(1100);
      const pausedRemaining2 = await getText('pausing-remaining');
      assert(
        pausedRemaining1 === pausedRemaining2,
        'Paused remaining changed while paused: ' + pausedRemaining1 + ' vs ' + pausedRemaining2
      );
      assert(
        !pausedRemaining1.includes('15:00'),
        'Paused remaining should be < 15:00, got ' + pausedRemaining1
      );
      report.shots.push(await captureShot('scene_b4_pausing_bar.png'));

      // Click continue -> resume countdown preserving remaining time
      await clickByTestId('btn-timer-continue');
      await waitFor(
        () => callInPage(() => !document.querySelector('[data-testid="pausing-bar"]')),
        4000,
        60,
        'pausing bar closed'
      );
      const resumedDisplay = await getText('timer-display');
      assert(
        resumedDisplay !== '15:00' && resumedDisplay !== '25:00',
        'Expected preserved countdown after continue, got ' + resumedDisplay
      );

      // Check random event switch matrix: music.doze must be disabled & unchecked
      await clickByTestId('btn-toggle-event-switches');
      const musicDozeSwitch = await waitFor(
        () =>
          callInPage(() => {
            const el = document.querySelector('[data-testid="switch-music-doze"]');
            return el ? { disabled: el.disabled, checked: el.checked } : null;
          }),
        4000,
        60,
        'music-doze switch'
      );
      assert(
        musicDozeSwitch.disabled === true && musicDozeSwitch.checked === false,
        'music-doze must be disabled & unchecked'
      );
      await clickByTestId('btn-toggle-event-switches');

      // Open Todo Drawer, edit list name + items, check item 0, save & confirm
      await clickByTestId('btn-toggle-todo');
      await waitFor(
        () => callInPage(() => Boolean(document.querySelector('[data-testid="todo-drawer"]'))),
        4000,
        60,
        'todo drawer'
      );
      await setInputValueByTestId('todo-list-name', '今日节奏自习计划');
      await setInputValueByTestId('todo-input-0', '完成线性代数第三章');
      await setInputValueByTestId('todo-input-1', '背诵英语核心词50个');
      await clickByTestId('todo-check-0');
      await sleep(150);
      report.shots.push(await captureShot('scene_b5_todo_drawer.png'));

      await clickByTestId('btn-save-list');
      const saveModalText = await waitFor(() => getText('modal-card'), 4000, 60, 'save list modal');
      assert(saveModalText.includes('Save list?'), 'Expected Save list? modal, got ' + saveModalText);
      report.shots.push(await captureShot('scene_b6_modal_save_list.png'));

      await clickByTestId('modal-save-yes');
      await waitFor(() => getText('todo-saved-badge'), 4000, 60, 'todo saved badge');
      const savedTodoRaw = await callInPage(() => localStorage.getItem('desk_pet_todo_list_v1'));
      const savedTodo = JSON.parse(savedTodoRaw || '{}');
      assert(savedTodo.listName === '今日节奏自习计划', 'Saved todo listName mismatch');
      assert(
        savedTodo.items?.[0]?.done === true && savedTodo.items?.[0]?.text === '完成线性代数第三章',
        'Saved todo item 0 mismatch'
      );

      // Close todo drawer & reset timer cleanly
      await clickByTestId('btn-toggle-todo');
      await clickByTestId('btn-timer-pause');
      await waitFor(() => getText('modal-btn-yes'), 3000, 50, 'pause before reset');
      await clickByTestId('modal-btn-yes');
      await waitFor(() => getText('btn-timer-reset'), 3000, 50, 'reset button');
      await clickByTestId('btn-timer-reset');
      await clickByTestId('mode-chill');

      report.scenarios.B = {
        studyRunningState,
        modal1Matched: true,
        modal2Matched: Boolean(modal2Text),
        pausedRemaining1,
        pausedRemaining2,
        resumedDisplay,
        musicDozeSwitch,
        savedTodoListName: savedTodo.listName,
      };

      // =========================================================
      // Scenario C: Emotion Tab (Happy & Sad, strict 1 eye + 1 mouth)
      // =========================================================
      await clickByTestId('tab-emotion');
      await waitFor(() => getText('emotion-happy'), 4000, 60, 'emotion tab');

      await clickByTestId('emotion-happy');
      await sleep(220);
      const happyState = await callInPage(() => {
        const rig = document.querySelector('[data-testid="character-head-rig"]');
        const eyes = document.querySelectorAll('[data-testid="layer-eyes"]');
        const mouths = document.querySelectorAll('[data-testid="layer-mouth"]');
        return {
          emotion: rig?.getAttribute('data-emotion'),
          eyesCount: eyes.length,
          mouthCount: mouths.length,
          eyesFile: eyes[0]?.getAttribute('data-asset-file'),
          mouthFile: mouths[0]?.getAttribute('data-asset-file'),
        };
      });
      assert(happyState.emotion === 'happy', 'Expected happy emotion');
      assert(happyState.eyesCount === 1 && happyState.mouthCount === 1, 'Expected strictly 1 eyes and 1 mouth layer');
      assert(
        happyState.eyesFile === 'eyes_happy.png' && happyState.mouthFile === 'mouth_happy.png',
        'Expected happy eye/mouth files'
      );
      report.shots.push(await captureShot('scene_c1_emotion_happy.png'));

      await clickByTestId('emotion-sad');
      await sleep(220);
      const sadState = await callInPage(() => {
        const rig = document.querySelector('[data-testid="character-head-rig"]');
        const eyes = document.querySelectorAll('[data-testid="layer-eyes"]');
        const mouths = document.querySelectorAll('[data-testid="layer-mouth"]');
        return {
          emotion: rig?.getAttribute('data-emotion'),
          eyesCount: eyes.length,
          mouthCount: mouths.length,
          eyesFile: eyes[0]?.getAttribute('data-asset-file'),
          mouthFile: mouths[0]?.getAttribute('data-asset-file'),
        };
      });
      assert(sadState.emotion === 'sad', 'Expected sad emotion');
      assert(
        sadState.eyesCount === 1 && sadState.mouthCount === 1,
        'Expected strictly 1 eyes and 1 mouth layer on sad'
      );
      assert(
        sadState.eyesFile === 'eyes_sad.png' && sadState.mouthFile === 'mouth_sad.png',
        'Expected sad eye/mouth files'
      );
      report.shots.push(await captureShot('scene_c2_emotion_sad.png'));

      // Toggle sad off -> neutral
      await clickByTestId('emotion-sad');
      report.scenarios.C = { happyState, sadState };

      // =========================================================
      // Scenario D: Metronome Tab + Demo Drums + DemoSong.mp3 + Q Stop
      // =========================================================
      await clickByTestId('tab-metronome');
      await waitFor(() => getText('btn-demo'), 4000, 60, 'metronome tab');

      const placeholdersState = await callInPage(() => {
        const setBpm = document.querySelector('[data-testid="placeholder-set-bpm"]');
        const tapFollow = document.querySelector('[data-testid="placeholder-tap-follow"]');
        const tapCheck = document.querySelector('[data-testid="placeholder-tap-check"]');
        const qHint = document.querySelector('[data-testid="hint-press-q"]')?.textContent || '';
        const allSetBpmDisabled = Array.from(setBpm.querySelectorAll('input,button')).every(
          (el) => el.disabled
        );
        const tapFollowDisabled = Boolean(tapFollow.querySelector('input')?.disabled);
        const tapCheckDisabled = Boolean(tapCheck.querySelector('input')?.disabled);
        return { allSetBpmDisabled, tapFollowDisabled, tapCheckDisabled, qHint };
      });
      assert(placeholdersState.allSetBpmDisabled, 'Set BPM placeholder must be disabled');
      assert(placeholdersState.tapFollowDisabled, 'Tap to follow placeholder must be disabled');
      assert(placeholdersState.tapCheckDisabled, 'Tap to check BPM placeholder must be disabled');
      assert(placeholdersState.qHint.includes('Q'), 'Missing Press Q hint');

      // D1: Start Demo Drums
      await clickByTestId('btn-demo');
      const demoLocked = await waitFor(
        () =>
          callInPage(() => {
            const badge = document.querySelector('[data-testid="lock-badge"]')?.textContent || '';
            const bpm = Number(document.querySelector('[data-testid="bpm-value"]')?.textContent || '0');
            const hp = Boolean(document.querySelector('[data-testid="layer-headphones"]'));
            return badge.includes('节拍已锁定') && bpm > 0 && hp ? { badge, bpm } : null;
          }),
        15000,
        150,
        'demo drums BPM lock'
      );

      const demoTransforms = [];
      for (let i = 0; i < 28; i++) {
        const sample = await callInPage(() => {
          const rig = document.querySelector('[data-testid="character-head-rig"]');
          return {
            transform: rig?.style?.transform || '',
            activeEvent: rig?.getAttribute('data-active-event') || '',
          };
        });
        demoTransforms.push(sample);
        await sleep(45);
      }
      assert(
        demoTransforms.every((s) => s.activeEvent !== 'doze'),
        'doze must never occur in music mode (demo)'
      );
      report.shots.push(await captureShot('scene_d1_demo_drums.png'));

      // Stop demo with Q key
      await sendKeyQ();
      await waitFor(
        () =>
          callInPage(() => {
            const el = document.querySelector('[data-testid="character-scene"]');
            return el && el.getAttribute('data-music-running') === 'false';
          }),
        5000,
        80,
        'Q key stop demo'
      );

      // D2: Inject real local audio DemoSong.mp3 via DataTransfer drop
      assert(fs.existsSync(demoSongPath), 'DemoSong.mp3 not found at ' + demoSongPath);
      const mp3Base64 = fs.readFileSync(demoSongPath).toString('base64');

      const dropInjected = await callInPage((b64) => {
        const bin = atob(b64);
        const bytes = new Uint8Array(bin.length);
        for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
        const file = new File([bytes], 'DemoSong.mp3', { type: 'audio/mpeg' });
        const dt = new DataTransfer();
        dt.items.add(file);
        const dropTarget = document.querySelector('.anime-desktop-bg');
        const ev = new DragEvent('drop', { bubbles: true, cancelable: true, dataTransfer: dt });
        dropTarget.dispatchEvent(ev);
        return { injectedBytes: bytes.length, fileName: file.name };
      }, mp3Base64);

      const songLocked = await waitFor(
        () =>
          callInPage(() => {
            const status = document.querySelector('[data-testid="music-status"]')?.textContent || '';
            const fileEl = document.querySelector('[data-testid="current-file-name"]')?.textContent || '';
            const badge = document.querySelector('[data-testid="lock-badge"]')?.textContent || '';
            const bpmText = document.querySelector('[data-testid="bpm-value"]')?.textContent || '';
            const bpm = Number(bpmText);
            const beatsText = document.querySelector('[data-testid="beat-count"]')?.textContent || '';
            const hp = Boolean(document.querySelector('[data-testid="layer-headphones"]'));
            if (badge.includes('节拍已锁定') && bpm > 0 && hp) {
              return { status, fileName: fileEl, badge, bpm, beatsText };
            }
            return null;
          }),
        22000,
        200,
        'DemoSong.mp3 BPM lock'
      );

      // Sample headRig transform frames to prove beat-synced vertical nodding & zero doze
      const songSamples = [];
      for (let i = 0; i < 40; i++) {
        const sample = await callInPage(() => {
          const rig = document.querySelector('[data-testid="character-head-rig"]');
          const beats = document.querySelector('[data-testid="beat-count"]')?.textContent || '';
          const bpm = Number(document.querySelector('[data-testid="bpm-value"]')?.textContent || '0');
          return {
            transform: rig?.style?.transform || '',
            activeEvent: rig?.getAttribute('data-active-event') || '',
            beats,
            bpm,
          };
        });
        songSamples.push(sample);
        await sleep(45);
      }

      const parsedYs = songSamples
        .map((s) => {
          const m = /translate3d\([^,]+,\s*([-\d.]+)px/.exec(s.transform);
          return m ? Number(m[1]) : null;
        })
        .filter((v) => v !== null);
      const minY = Math.min(...parsedYs);
      const maxY = Math.max(...parsedYs);
      const nodRangePx = Number((maxY - minY).toFixed(2));
      assert(parsedYs.length >= 30, 'Failed to parse headRig transform samples');
      assert(nodRangePx > 4, 'Expected visible beat-synced head nod amplitude > 4px, got ' + nodRangePx);
      assert(
        songSamples.every((s) => s.activeEvent !== 'doze'),
        'doze must be forcibly disabled in music mode'
      );

      report.shots.push(await captureShot('scene_d2_demosong_locked.png'));

      // Stop DemoSong.mp3 immediately with Q key
      await sendKeyQ();
      await waitFor(
        () =>
          callInPage(() => {
            const scene = document.querySelector('[data-testid="character-scene"]');
            const status = document.querySelector('[data-testid="music-status"]')?.textContent || '';
            const hp = document.querySelector('[data-testid="layer-headphones"]');
            return scene?.getAttribute('data-music-running') === 'false' && status === '未开始' && !hp;
          }),
        5000,
        80,
        'Q key stops DemoSong.mp3'
      );
      report.shots.push(await captureShot('scene_d3_after_q_stop.png'));

      report.scenarios.D = {
        placeholdersState,
        demoLocked,
        dropInjected,
        songLocked,
        nodSampleCount: parsedYs.length,
        nodMinY: minY,
        nodMaxY: maxY,
        nodRangePx,
        finalBpmOnLock: songLocked.bpm,
        sampleTransformsPreview: songSamples.slice(0, 6).map((s) => s.transform),
        qKeyStoppedCleanly: true,
      };

      assert(consoleErrors.length === 0, 'Console errors detected: ' + JSON.stringify(consoleErrors));
      assert(failedAssets.length === 0, 'Failed loads detected: ' + JSON.stringify(failedAssets));

      report.consoleErrors = consoleErrors;
      report.failedAssets = failedAssets;
      report.finishedAt = new Date().toISOString();
      report.passed = true;

      if (reportPath) {
        fs.writeFileSync(reportPath, JSON.stringify(report, null, 2), 'utf8');
      }
      console.log('[E2E PASS]', JSON.stringify(report, null, 2));
      app.exit(0);
    } catch (err) {
      report.passed = false;
      report.error = err && err.stack ? err.stack : String(err);
      report.consoleErrors = consoleErrors;
      report.failedAssets = failedAssets;
      if (reportPath) {
        fs.writeFileSync(reportPath, JSON.stringify(report, null, 2), 'utf8');
      }
      console.error('[E2E FAIL]', report.error);
      app.exit(1);
    }
  })();
}

async function main() {
  if (!fs.existsSync(path.join(PROJECT_ROOT, 'dist', 'index.html'))) {
    throw new Error('dist/index.html not found. Run "npm run build" before verify-electron-e2e.mjs.');
  }

  fs.mkdirSync(SCRATCH_ROOT, { recursive: true });
  fs.rmSync(SHOT_DIR, { recursive: true, force: true });
  fs.rmSync(USER_DATA_DIR, { recursive: true, force: true });
  fs.mkdirSync(SHOT_DIR, { recursive: true });
  fs.mkdirSync(USER_DATA_DIR, { recursive: true });

  const driverFileContent = `'use strict';\nmodule.exports = ${driverMain.toString()};\n`;
  fs.writeFileSync(DRIVER_PATH, driverFileContent, 'utf8');

  console.log('Launching real Electron desktop app for E2E verification...');
  console.log('Electron binary:', electronExecutable);
  console.log('Driver module  :', DRIVER_PATH);
  console.log('Shot directory :', SHOT_DIR);

  const child = spawn(
    electronExecutable,
    ['.', `--user-data-dir=${USER_DATA_DIR}`, '--autoplay-policy=no-user-gesture-required'],
    {
      cwd: PROJECT_ROOT,
      env: {
        ...process.env,
        ELECTRON_FORCE_DIST: '1',
        PET_E2E_DRIVER: DRIVER_PATH,
        PET_E2E_SHOT_DIR: SHOT_DIR,
        PET_E2E_DEMO_SONG: DEMO_SONG_PATH,
        PET_E2E_REPORT_PATH: REPORT_PATH,
      },
      stdio: 'inherit',
    }
  );

  const timeout = setTimeout(() => {
    console.error('[ERROR] Electron E2E timed out after 120s, terminating process...');
    child.kill('SIGKILL');
    process.exit(1);
  }, 120_000);

  child.on('exit', (code, signal) => {
    clearTimeout(timeout);
    if (code !== 0) {
      console.error(`[ERROR] Electron E2E exited with code=${code} signal=${signal}`);
      process.exit(code || 1);
    }
    if (!fs.existsSync(REPORT_PATH)) {
      console.error('[ERROR] Missing E2E report JSON.');
      process.exit(1);
    }
    const report = JSON.parse(fs.readFileSync(REPORT_PATH, 'utf8'));
    console.log(`[OK] Electron E2E passed with ${report.shots.length} screenshots in ${SHOT_DIR}`);
  });
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
