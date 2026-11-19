import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { E1, jh } from './audio';
import { DeskCharacterScene } from './character';
import { EmotionTab } from './gui/EmotionTab';
import { FloatingConsole } from './gui/FloatingConsole';
import { SettingsTab } from './gui/SettingsTab';
import { useSettings } from './state/SettingsContext';
import { createCompletionSound, timerValue, type TimerMode } from './state/timer';
import { isRandomEventAllowed } from './character/useCharacterEvents';
import { MetronomeTab } from './gui/MetronomeTab';
import { Modals, type AppModalType } from './gui/Modals';
import { StatusTab, type TimerState } from './gui/StatusTab';
import { TodoDrawer, type TodoItem } from './gui/TodoDrawer';
import {
  DEFAULT_EVENT_SWITCHES,
  type ActivityMode,
  type AudioSourceMode,
  type ExpressionDefinition,
  type EventContextKey,
  type EventSwitchMatrix,
  type MotionFrame,
  type RandomEventType,
} from './types/contracts';

type MainTab = 'status' | 'emotion' | 'metronome' | 'settings';

const STORAGE_KEYS = {
  eventSwitches: 'desk_pet_event_switches_v1',
  todoList: 'desk_pet_todo_list_v1',
  studyMinutes: 'desk_pet_study_minutes_v1',
} as const;

const DEFAULT_TODO_ITEMS: TodoItem[] = [
  { id: 'todo-1', text: '', done: false },
  { id: 'todo-2', text: '', done: false },
  { id: 'todo-3', text: '', done: false },
];

function cloneDefaultSwitches(): EventSwitchMatrix {
  return {
    chill: { ...DEFAULT_EVENT_SWITCHES.chill },
    study: { ...DEFAULT_EVENT_SWITCHES.study },
    music: { ...DEFAULT_EVENT_SWITCHES.music, doze: false },
  };
}

function loadEventSwitches(): EventSwitchMatrix {
  const base = cloneDefaultSwitches();
  try {
    const raw = localStorage.getItem(STORAGE_KEYS.eventSwitches);
    if (!raw) return base;
    const parsed = JSON.parse(raw) as Partial<EventSwitchMatrix>;
    const contexts: EventContextKey[] = ['chill', 'study', 'music'];
    const events: RandomEventType[] = ['doze', 'think', 'shock', 'love'];
    for (const ctx of contexts) {
      const row = parsed?.[ctx];
      if (row && typeof row === 'object') {
        for (const ev of events) {
          if (typeof row[ev] === 'boolean') {
            base[ctx][ev] = row[ev];
          }
        }
      }
    }
  } catch {
    // Fallback cleanly to defaults if storage is corrupted
  }
  base.music.doze = false;
  return base;
}

function loadTodoList(): { listName: string; items: TodoItem[] } {
  try {
    const raw = localStorage.getItem(STORAGE_KEYS.todoList);
    if (!raw) return { listName: '', items: DEFAULT_TODO_ITEMS };
    const parsed = JSON.parse(raw) as { listName?: unknown; items?: unknown };
    const listName = typeof parsed.listName === 'string' ? parsed.listName : '';
    const items = Array.isArray(parsed.items)
      ? parsed.items
          .filter(
            (item): item is TodoItem =>
              Boolean(item) &&
              typeof item === 'object' &&
              typeof (item as TodoItem).id === 'string' &&
              typeof (item as TodoItem).text === 'string' &&
              typeof (item as TodoItem).done === 'boolean',
          )
      : [];
    return {
      listName,
      items: items.length > 0 ? items : DEFAULT_TODO_ITEMS,
    };
  } catch {
    return { listName: '', items: DEFAULT_TODO_ITEMS };
  }
}

function loadInitialMinutes(): number {
  try {
    const raw = localStorage.getItem(STORAGE_KEYS.studyMinutes);
    const num = raw ? parseInt(raw, 10) : 25;
    if (Number.isFinite(num) && num >= 1 && num <= 999) return num;
  } catch {
    // Ignore storage read error
  }
  return 25;
}

function isAudioFile(file: File): boolean {
  return (
    file.type.startsWith('audio/') ||
    /\.(mp3|wav|ogg|flac|m4a|aac|opus|webm)$/i.test(file.name)
  );
}

export default function App() {
  const { settings } = useSettings();
  const [consoleHost, setConsoleHost] = useState<Window | null>(null);
  const [petLocked, setPetLocked] = useState(false);
  const [expressions, setExpressions] = useState<ExpressionDefinition[]>([]);
  const [selectedId, setSelectedId] = useState('neutral');
  const [baseId, setBaseId] = useState('neutral');
  const [randomExpression, setRandomExpression] = useState<ExpressionDefinition | null>(null);
  const randomEndRef = useRef<number | null>(null);
  const soundRef = useRef<ReturnType<typeof createCompletionSound> | null>(null);
  const [timerMode, setTimerMode] = useState<TimerMode>('countdown');
  const timerModeRef = useRef(timerMode);
  timerModeRef.current = timerMode;
  const soundEnabledRef = useRef(settings.endSoundEnabled);
  soundEnabledRef.current = settings.endSoundEnabled;

  useEffect(() => {
    const bridge = window.desktopPetBridge;
    if (!bridge) return;
    void bridge.getLocked().then(setPetLocked);
    return bridge.onLocked(setPetLocked);
  }, []);
  const toggleLock = () => {
    const bridge = window.desktopPetBridge;
    if (bridge) void bridge.setLocked(!petLocked).then(setPetLocked);
    else setPetLocked((value) => !value);
  };

  // Audio Engine Refs (Frozen E1 / jh)
  const engineRef = useRef<E1 | null>(null);
  const getEngine = useCallback(() => {
    if (!engineRef.current) {
      engineRef.current = new E1();
    }
    return engineRef.current;
  }, []);

  const motionRef = useRef<MotionFrame>({ ...jh });
  const latestFrameRef = useRef<MotionFrame>({ ...jh });
  const lastModeRef = useRef<Exclude<AudioSourceMode, 'idle'>>('system');
  const lastFileRef = useRef<File | null>(null);
  const audioStartedAtRef = useRef<number>(0);

  // Audio UI State
  const [audioMode, setAudioMode] = useState<AudioSourceMode>('idle');
  const [musicRunning, setMusicRunning] = useState(false);
  const [audioBusy, setAudioBusy] = useState(false);
  const [audioError, setAudioError] = useState<string | null>(null);
  const [silenceHint, setSilenceHint] = useState(false);
  const [sensitivity, setSensitivity] = useState(1.22);
  const [fileName, setFileName] = useState('');
  const [autoCheckBpm, setAutoCheckBpm] = useState(true);
  const [autoFollow, setAutoFollow] = useState(true);
  const [telemetry, setTelemetry] = useState({
    bpm: 0,
    level: 0,
    bass: 0,
    beats: 0,
    locked: false,
  });

  // Panel / Tab / Stage State
  const [activeTab, setActiveTab] = useState<MainTab>('status');

  const [assetReloadToken, setAssetReloadToken] = useState(0);
  const [assetStatus, setAssetStatus] = useState<{
    loaded: boolean;
    packName?: string;
    assetDirHint?: string;
    error?: string;
  }>({ loaded: false });

  // Status / Study Timer / Emotion / Random Switches State
  const initialMinutes = useMemo(() => loadInitialMinutes(), []);
  const [activityMode, setActivityMode] = useState<ActivityMode>('chill');

  const [studyMinutes, setStudyMinutes] = useState(initialMinutes);
  const [timerState, setTimerState] = useState<TimerState>('idle');
  const [totalMs, setTotalMs] = useState(initialMinutes * 60_000);
  const [remainingMs, setRemainingMs] = useState(initialMinutes * 60_000);
  const timerEndAtRef = useRef<number | null>(null);
  const pendingPauseReasonRef = useRef<'pause' | 'leave-study'>('pause');

  const [eventSwitches, setEventSwitches] = useState<EventSwitchMatrix>(() => loadEventSwitches());

  // Todo Drawer & Modals State
  const initialTodo = useMemo(() => loadTodoList(), []);
  const [todoOpen, setTodoOpen] = useState(false);
  const [todoListName, setTodoListName] = useState(initialTodo.listName);
  const [todoItems, setTodoItems] = useState<TodoItem[]>(initialTodo.items);
  const [todoSavedNotice, setTodoSavedNotice] = useState(false);
  const todoNoticeTimerRef = useRef<number | null>(null);
  const [modal, setModal] = useState<AppModalType>('none');

  // Wire E1 frame & ended listeners
  useEffect(() => {
    const engine = getEngine();
    engine.on({
      frame: (frame) => {
        latestFrameRef.current = frame;
        motionRef.current = frame;
      },
      ended: () => {
        setMusicRunning(false);
        setAudioMode('idle');
        setSilenceHint(false);
      },
    });
    return () => engine.on({});
  }, [getEngine]);

  // Dispose audio engine on unmount
  useEffect(() => {
    return () => {
      engineRef.current?.dispose();
      engineRef.current = null;
      soundRef.current?.dispose();
      soundRef.current = null;
      if (todoNoticeTimerRef.current !== null) {
        window.clearTimeout(todoNoticeTimerRef.current);
      }
    };
  }, []);

  // Poll telemetry for UI display & silence hint
  useEffect(() => {
    const timer = window.setInterval(() => {
      const q = latestFrameRef.current;
      setTelemetry((prev) =>
        prev.bpm === q.bpm &&
        prev.beats === q.beats &&
        prev.locked === q.locked &&
        Math.abs(prev.level - q.level) < 0.004 &&
        Math.abs(prev.bass - q.bass) < 0.004
          ? prev
          : {
              bpm: q.bpm,
              level: q.level,
              bass: q.bass,
              beats: q.beats,
              locked: q.locked,
            },
      );
      const elapsed = performance.now() - audioStartedAtRef.current;
      setSilenceHint(musicRunning && elapsed > 3500 && q.level < 0.012);
    }, 120);
    return () => window.clearInterval(timer);
  }, [musicRunning]);

  // Sync sensitivity to running detector
  useEffect(() => {
    engineRef.current?.setSensitivity(sensitivity);
  }, [sensitivity]);

  // Audio Start / Stop helpers
  const startAudio = useCallback(
    async (mode: Exclude<AudioSourceMode, 'idle'>, file?: File) => {
      const engine = getEngine();
      setAudioBusy(true);
      setAudioError(null);
      setSilenceHint(false);
      try {
        if (mode === 'system') {
          await engine.startSystemAudio();
        } else if (mode === 'mic') {
          await engine.startMic();
        } else if (mode === 'file') {
          if (!file) throw new Error('请先选择一个音频文件');
          await engine.startFile(file);
          lastFileRef.current = file;
          setFileName(file.name);
        } else {
          engine.startDemo();
        }
        engine.setSensitivity(sensitivity);
        lastModeRef.current = mode;
        audioStartedAtRef.current = performance.now();
        setAudioMode(mode);
        setMusicRunning(true);
      } catch (err) {
        const name = err instanceof Error ? err.name : '';
        setAudioError(name === 'NotAllowedError'
          ? '未获得音频共享权限，请允许共享声音后重试。'
          : '无法连接音频，请检查系统输出设备和共享音频设置后重试。');
        setMusicRunning(false);
      } finally {
        setAudioBusy(false);
      }
    },
    [getEngine, sensitivity],
  );

  const stopAudio = useCallback(() => {
    engineRef.current?.stop();
    latestFrameRef.current = { ...jh };
    motionRef.current = { ...jh };
    setMusicRunning(false);
    setAudioMode('idle');
    setSilenceHint(false);
    setTelemetry({ bpm: 0, level: 0, bass: 0, beats: 0, locked: false });
  }, []);

  const togglePlayLastAudio = useCallback(() => {
    if (musicRunning) {
      stopAudio();
      return;
    }
    if (lastModeRef.current === 'file' && lastFileRef.current) {
      void startAudio('file', lastFileRef.current);
      return;
    }
    const target = lastModeRef.current === 'file' ? 'system' : lastModeRef.current;
    void startAudio(target);
  }, [musicRunning, startAudio, stopAudio]);

  // Global Keyboard Shortcuts: Q (stop), Space (toggle play/stop), H (collapse/expand)
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      const tag = target?.tagName;
      if (
        tag === 'INPUT' ||
        tag === 'TEXTAREA' ||
        tag === 'SELECT' ||
        target?.isContentEditable
      ) {
        return;
      }

      if (e.key === 'q' || e.key === 'Q') {
        e.preventDefault();
        stopAudio();
      } else if (e.code === 'Space') {
        e.preventDefault();
        togglePlayLastAudio();
      } else if (e.key === 'h' || e.key === 'H') {
        e.preventDefault();
        window.desktopPetBridge?.hideConsole();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    consoleHost?.addEventListener('keydown', onKeyDown);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
      consoleHost?.removeEventListener('keydown', onKeyDown);
    };
  }, [stopAudio, togglePlayLastAudio, consoleHost]);

  // Study Timer Countdown Effect
  useEffect(() => {
    if (timerState !== 'running') return;
    const tick = () => {
      if (timerEndAtRef.current === null) return;
      const left = timerValue(timerModeRef.current, timerEndAtRef.current, Date.now());
      setRemainingMs(left);
      if (timerModeRef.current === 'countdown' && left <= 0) {
        timerEndAtRef.current = null;
        setTimerState('completed');
        setModal('none');
        if (soundEnabledRef.current) void soundRef.current?.play().catch(() => {});
      }
    };
    tick();
    const id = window.setInterval(tick, 200);
    return () => window.clearInterval(id);
  }, [timerState]);

  // Drag & Drop audio file onto window
  const handleDrop = (e: React.DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    const file = Array.from(e.dataTransfer.files).find(isAudioFile);
    if (file) {
      void startAudio('file', file);
    }
  };

  // Study / Chill & Timer Handlers
  const handleSelectActivityMode = (nextMode: ActivityMode) => {
    if (nextMode === activityMode) return;
    if (
      nextMode === 'chill' &&
      activityMode === 'study' &&
      timerState === 'running' &&
      (timerMode === 'stopwatch' || remainingMs > 0)
    ) {
      pendingPauseReasonRef.current = 'leave-study';
      setModal('confirm-leave-study');
      return;
    }
    setActivityMode(nextMode);
  };

  const handleChangeStudyMinutes = (mins: number) => {
    if (!Number.isFinite(mins)) return;
    const clamped = Math.max(1, Math.min(999, Math.trunc(mins)));
    setStudyMinutes(clamped);
    try {
      localStorage.setItem(STORAGE_KEYS.studyMinutes, String(clamped));
    } catch {
      // Ignore storage error
    }
    if (timerMode === 'countdown' && (timerState === 'idle' || timerState === 'completed')) {
      const nextMs = clamped * 60_000;
      setTotalMs(nextMs);
      setRemainingMs(nextMs);
      if (timerState === 'completed') {
        setTimerState('idle');
      }
    }
  };

  const handleStartOrResumeTimer = () => {
    if (timerState === 'running') return;
    soundRef.current ??= createCompletionSound();
    soundRef.current.prepare();
    const duration = studyMinutes * 60_000;
    const value = timerState === 'paused' ? remainingMs : timerMode === 'countdown' ? duration : 0;
    setTotalMs(duration);
    setRemainingMs(value);
    setActivityMode('study');
    timerEndAtRef.current = timerMode === 'countdown' ? Date.now() + value : Date.now() - value;
    setTimerState('running');
  };

  const handleRequestPauseTimer = () => {
    if (timerState !== 'running') return;
    pendingPauseReasonRef.current = 'pause';
    if (timerMode === 'stopwatch') { handleConfirmPause(); return; }
    setModal('confirm-pause');
  };

  const handleConfirmPause = () => {
    const value = timerEndAtRef.current === null ? remainingMs
      : timerValue(timerMode, timerEndAtRef.current, Date.now());
    timerEndAtRef.current = null;
    setRemainingMs(value);
    if (timerMode === 'countdown' && value <= 0) {
      setTimerState('completed');
      if (soundEnabledRef.current) void soundRef.current?.play().catch(() => {});
    } else setTimerState('paused');
    if (pendingPauseReasonRef.current === 'leave-study') setActivityMode('chill');
    setModal('none');
  };

  const handleRejectPause = () => {
    setModal('go-back-study');
  };

  const handleDismissGoBack = () => {
    setModal('none');
  };

  const handleResetTimer = () => {
    timerEndAtRef.current = null;
    const ms = Math.max(1, Math.min(999, studyMinutes)) * 60_000;
    setTotalMs(ms);
    setRemainingMs(timerMode === 'countdown' ? ms : 0);
    setTimerState('idle');
  };

  // Event Switch Matrix Handler (music.doze strictly locked false)
  const handleToggleEventSwitch = (ctx: EventContextKey, ev: RandomEventType) => {
    if (ctx === 'music' && ev === 'doze') return;
    setEventSwitches((prev) => {
      const next: EventSwitchMatrix = {
        chill: { ...prev.chill },
        study: { ...prev.study },
        music: { ...prev.music, doze: false },
      };
      next[ctx][ev] = !prev[ctx][ev];
      next.music.doze = false;
      try {
        localStorage.setItem(STORAGE_KEYS.eventSwitches, JSON.stringify(next));
      } catch {
        // Ignore storage write error
      }
      return next;
    });
  };

  // Todo Drawer Handlers
  const handleToggleTodoItem = (id: string) => {
    setTodoItems((prev) =>
      prev.map((item) => (item.id === id ? { ...item, done: !item.done } : item)),
    );
  };

  const handleUpdateTodoText = (id: string, text: string) => {
    setTodoItems((prev) =>
      prev.map((item) => (item.id === id ? { ...item, text } : item)),
    );
  };

  const handleAddTodoItem = () => {
    setTodoItems((prev) => [
      ...prev,
      { id: `todo-${Date.now()}-${prev.length + 1}`, text: '', done: false },
    ]);
  };

  const handleRemoveTodoItem = (id: string) => {
    setTodoItems((prev) => (prev.length > 1 ? prev.filter((item) => item.id !== id) : prev));
  };

  const handleConfirmSaveTodo = () => {
    try {
      localStorage.setItem(
        STORAGE_KEYS.todoList,
        JSON.stringify({ listName: todoListName, items: todoItems }),
      );
      setTodoSavedNotice(true);
      if (todoNoticeTimerRef.current !== null) {
        window.clearTimeout(todoNoticeTimerRef.current);
      }
      todoNoticeTimerRef.current = window.setTimeout(() => {
        setTodoSavedNotice(false);
      }, 4000);
    } catch {
      // Ignore storage error
    }
    setModal('none');
  };

  const eligibleExpressions = expressions.filter((entry) => entry.random &&
    (!entry.event || isRandomEventAllowed(entry.event, activityMode, musicRunning, eventSwitches)));
  const poolKey = eligibleExpressions.map((entry) => entry.id).join('|');
  const poolRef = useRef(eligibleExpressions);
  poolRef.current = eligibleExpressions;
  useEffect(() => {
    setRandomExpression(null);
    if (randomEndRef.current !== null) window.clearTimeout(randomEndRef.current);
    if (!settings.randomEnabled) return;
    const timer = window.setInterval(() => {
      const pool = poolRef.current;
      if (!pool.length) return;
      if (randomEndRef.current !== null) window.clearTimeout(randomEndRef.current);
      const entry = pool[Math.floor(Math.random() * pool.length)];
      setRandomExpression(entry);
      randomEndRef.current = window.setTimeout(() => setRandomExpression(null), entry.durationMs);
    }, settings.randomIntervalSeconds * 1000);
    return () => {
      window.clearInterval(timer);
      if (randomEndRef.current !== null) window.clearTimeout(randomEndRef.current);
    };
  }, [settings.randomEnabled, settings.randomIntervalSeconds, poolKey, expressions]);
  const allowedExpression = (entry: ExpressionDefinition | undefined | null) =>
    entry && (!entry.event || isRandomEventAllowed(entry.event, activityMode, musicRunning, eventSwitches)) ? entry : null;
  const manualExpression = expressions.find((entry) => entry.id === selectedId);
  const expression = allowedExpression(randomExpression) ??
    (musicRunning && manualExpression?.event === 'doze' ? null : manualExpression);
  const baseExpression = expression?.type === 'base' ? expression : expressions.find((entry) => entry.id === baseId);
  const selectExpression = (id: string) => {
    setRandomExpression(null);
    if (randomEndRef.current !== null) window.clearTimeout(randomEndRef.current);
    const nextId = expression?.id === id ? 'neutral' : id;
    setSelectedId(nextId);
    if (expressions.find((entry) => entry.id === nextId)?.type === 'base') setBaseId(nextId);
  };
  const changeTimerMode = (mode: TimerMode) => {
    if (timerState === 'running' || timerState === 'paused') return;
    timerEndAtRef.current = null;
    setTimerMode(mode);
    setTimerState('idle');
    setRemainingMs(mode === 'countdown' ? studyMinutes * 60_000 : 0);
  };

  const silenceText =
    !audioError && silenceHint
      ? '似乎没有捕获到声音：请确认音乐正在播放，并已在共享弹窗里勾选「分享音频」。'
      : null;

  return <div className="pet-root anime-desktop-bg" onDrop={handleDrop} onDragOver={(e) => e.preventDefault()}>
    <div className="pet-toolbar">
      <button className="action-button" data-testid="btn-open-console" onClick={() => window.desktopPetBridge?.showConsole()}>打开控制台</button>
      <button className="action-button" data-testid="btn-lock-pet" onClick={toggleLock}>{petLocked ? '解除锁定' : '锁定'}</button>
    </div>
    <div data-testid="character-stage-wrapper" className={`pet-stage ${petLocked ? 'pet-locked' : ''}`}>
      <DeskCharacterScene motionRef={motionRef} musicRunning={musicRunning} autoFollow={autoFollow}
        activityMode={activityMode} timerActive={timerState === 'running'} emotion="neutral"
        expression={expression} baseExpression={baseExpression} forcedEventPreview={expression?.event ?? null} eventSwitches={eventSwitches}
        assetReloadToken={assetReloadToken} onAssetStatus={setAssetStatus} onExpressionsLoaded={setExpressions} />
    </div>
    <FloatingConsole onWindow={setConsoleHost}>
      <div className="console-content" onDrop={handleDrop} onDragOver={(e) => e.preventDefault()}>
        <header className="console-titlebar">
          <strong>桌宠节奏师 · 控制台</strong>
          <div className="flex gap-2">
            <button className="action-button" data-testid="btn-reload-assets" onClick={() => setAssetReloadToken((n) => n + 1)}>重载素材</button>
            <button className="action-button" aria-label="关闭控制台" data-testid="btn-close-console" onClick={() => window.desktopPetBridge?.hideConsole()}>关闭</button>
          </div>
        </header>
        <nav className="grid grid-cols-4 gap-2 p-3">
          {([{ id: 'status', label: '状态' }, { id: 'emotion', label: '情绪' },
            { id: 'metronome', label: '音频同步' }, { id: 'settings', label: '设置' }] as const).map((tab) =>
            <button key={tab.id} data-testid={`tab-${tab.id}`} className={`action-button ${activeTab === tab.id ? 'selected-expression' : ''}`}
              aria-pressed={activeTab === tab.id} onClick={() => setActiveTab(tab.id)}>{tab.label}</button>)}
        </nav>
        <div className="anime-scroll console-scroll">
          <p data-testid="asset-status" className="text-xs mb-3" title={assetStatus.assetDirHint}>
            {assetStatus.error ? `素材加载失败：${assetStatus.error}` : assetStatus.loaded ? '角色素材已就绪' : '正在加载角色素材…'}
          </p>
          {activeTab === 'status' && <>
            <StatusTab activityMode={activityMode} onSelectActivityMode={handleSelectActivityMode}
              timerMode={timerMode} onChangeTimerMode={changeTimerMode}
              studyMinutes={studyMinutes} onChangeStudyMinutes={handleChangeStudyMinutes}
              timerState={timerState} remainingMs={remainingMs} totalMs={totalMs}
              onStartOrResumeTimer={handleStartOrResumeTimer} onRequestPauseTimer={handleRequestPauseTimer}
              onResetTimer={handleResetTimer} eventSwitches={eventSwitches} onToggleEventSwitch={handleToggleEventSwitch}
              todoOpen={todoOpen} onToggleTodo={() => setTodoOpen((v) => !v)} />
            <TodoDrawer open={todoOpen} listName={todoListName} onListNameChange={setTodoListName}
              items={todoItems} onToggleItem={handleToggleTodoItem} onUpdateItemText={handleUpdateTodoText}
              onAddItem={handleAddTodoItem} onRemoveItem={handleRemoveTodoItem}
              onRequestSave={() => setModal('confirm-save-todo')} savedNotice={todoSavedNotice} />
          </>}
          {activeTab === 'emotion' && <EmotionTab expressions={expressions}
            selectedId={expression?.id ?? 'neutral'} musicRunning={musicRunning} onSelectExpression={selectExpression} />}
          {activeTab === 'metronome' && <MetronomeTab mode={audioMode} running={musicRunning} busy={audioBusy}
            bpm={telemetry.bpm} level={telemetry.level} bass={telemetry.bass} beats={telemetry.beats} locked={telemetry.locked}
            sensitivity={sensitivity} fileName={fileName} error={audioError} silenceHint={silenceText}
            autoCheckBpm={autoCheckBpm} onToggleAutoCheckBpm={() => setAutoCheckBpm((v) => !v)}
            autoFollow={autoFollow} onToggleAutoFollow={() => setAutoFollow((v) => !v)}
            onSelectMode={(mode) => void startAudio(mode)} onSensitivityChange={setSensitivity} onStop={stopAudio} />}
          {activeTab === 'settings' && <SettingsTab locked={petLocked} onToggleLock={toggleLock} />}
        </div>
        <footer className="p-3 text-xs">空格键：开始／停止 · Q 键：停止 · H 键：隐藏控制台<br />全局快捷键：Ctrl＋Alt＋C 打开控制台 · Ctrl＋Alt＋L 锁定／解锁</footer>
        <Modals modal={modal} onConfirmPause={handleConfirmPause} onRejectPause={handleRejectPause}
          onDismissGoBack={handleDismissGoBack} onConfirmSaveTodo={handleConfirmSaveTodo}
          onCancelSaveTodo={() => setModal('none')} />
      </div>
    </FloatingConsole>
  </div>;
}
