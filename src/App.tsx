import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { E1, jh } from './audio';
import { DeskCharacterScene } from './character';
import { EmotionTab } from './gui/EmotionTab';
import { ClockIcon, HeartIcon, MetronomeIcon } from './gui/Icons';
import { MetronomeTab } from './gui/MetronomeTab';
import { Modals, type AppModalType } from './gui/Modals';
import { StatusTab, type TimerState } from './gui/StatusTab';
import { TodoDrawer, type TodoItem } from './gui/TodoDrawer';
import {
  DEFAULT_EVENT_SWITCHES,
  type ActivityMode,
  type AudioSourceMode,
  type EmotionType,
  type EventContextKey,
  type EventSwitchMatrix,
  type MotionFrame,
  type RandomEventType,
} from './types/contracts';

type MainTab = 'status' | 'emotion' | 'metronome';

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
  const lastModeRef = useRef<Exclude<AudioSourceMode, 'idle'>>('demo');
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
  const [collapsed, setCollapsed] = useState(false);
  const [assetReloadToken, setAssetReloadToken] = useState(0);
  const [assetStatus, setAssetStatus] = useState<{
    loaded: boolean;
    packName?: string;
    assetDirHint?: string;
    error?: string;
  }>({ loaded: false });

  // Draggable Character Stage State
  const stageContainerRef = useRef<HTMLDivElement | null>(null);
  const dragOffsetRef = useRef<{ dx: number; dy: number } | null>(null);
  const [isDragging, setIsDragging] = useState(false);
  const [dragPos, setDragPos] = useState<{ x: number; y: number } | null>(null);

  // Status / Study Timer / Emotion / Random Switches State
  const initialMinutes = useMemo(() => loadInitialMinutes(), []);
  const [activityMode, setActivityMode] = useState<ActivityMode>('chill');
  const [emotion, setEmotion] = useState<EmotionType>('neutral');
  const [setTimeEnabled, setSetTimeEnabled] = useState(true);
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
        const msg = err instanceof Error ? err.message : String(err);
        setAudioError(
          msg.includes('Permission') || msg.includes('NotAllowed')
            ? '权限被拒绝了。请检查权限设置，或改用「本地音乐」模式。'
            : msg,
        );
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
    const target = lastModeRef.current === 'file' ? 'demo' : lastModeRef.current;
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
        setCollapsed((v) => !v);
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [stopAudio, togglePlayLastAudio]);

  // Study Timer Countdown Effect
  useEffect(() => {
    if (timerState !== 'running') return;
    const tick = () => {
      if (timerEndAtRef.current === null) return;
      const left = Math.max(0, timerEndAtRef.current - Date.now());
      setRemainingMs(left);
      if (left <= 0) {
        timerEndAtRef.current = null;
        setTimerState('completed');
      }
    };
    tick();
    const id = window.setInterval(tick, 200);
    return () => window.clearInterval(id);
  }, [timerState]);

  // Clamp dragged character inside window on resize
  useEffect(() => {
    const onResize = () => {
      setDragPos((pos) => {
        if (!pos) return pos;
        const el = stageContainerRef.current;
        if (!el) return pos;
        return {
          x: Math.min(pos.x, Math.max(0, window.innerWidth - el.offsetWidth)),
          y: Math.min(pos.y, Math.max(0, window.innerHeight - el.offsetHeight)),
        };
      });
    };
    window.addEventListener('resize', onResize);
    return () => window.removeEventListener('resize', onResize);
  }, []);

  const handlePointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    const el = stageContainerRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    dragOffsetRef.current = { dx: e.clientX - rect.left, dy: e.clientY - rect.top };
    try {
      el.setPointerCapture(e.pointerId);
    } catch {
      // Ignore pointer capture errors
    }
    setIsDragging(true);
  };

  const handlePointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    const offset = dragOffsetRef.current;
    const el = stageContainerRef.current;
    if (!offset || !el) return;
    const nextX = Math.min(
      Math.max(0, e.clientX - offset.dx),
      Math.max(0, window.innerWidth - el.offsetWidth),
    );
    const nextY = Math.min(
      Math.max(0, e.clientY - offset.dy),
      Math.max(0, window.innerHeight - el.offsetHeight),
    );
    setDragPos({ x: nextX, y: nextY });
  };

  const handlePointerUp = (e: React.PointerEvent<HTMLDivElement>) => {
    dragOffsetRef.current = null;
    setIsDragging(false);
    try {
      stageContainerRef.current?.releasePointerCapture(e.pointerId);
    } catch {
      // Ignore release errors
    }
  };

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
      remainingMs > 0
    ) {
      pendingPauseReasonRef.current = 'leave-study';
      setModal('confirm-leave-study');
      return;
    }
    setActivityMode(nextMode);
  };

  const handleChangeStudyMinutes = (mins: number) => {
    const clamped = Math.max(1, Math.min(999, mins));
    setStudyMinutes(clamped);
    setSetTimeEnabled(true);
    try {
      localStorage.setItem(STORAGE_KEYS.studyMinutes, String(clamped));
    } catch {
      // Ignore storage error
    }
    if (timerState === 'idle' || timerState === 'completed') {
      const nextMs = clamped * 60_000;
      setTotalMs(nextMs);
      setRemainingMs(nextMs);
      if (timerState === 'completed') {
        setTimerState('idle');
      }
    }
  };

  const handleStartOrResumeTimer = () => {
    if (timerState === 'paused' && remainingMs > 0) {
      setActivityMode('study');
      timerEndAtRef.current = Date.now() + remainingMs;
      setTimerState('running');
      return;
    }
    setSetTimeEnabled(true);
    const ms = Math.max(1, Math.min(999, studyMinutes)) * 60_000;
    setTotalMs(ms);
    setRemainingMs(ms);
    setActivityMode('study');
    timerEndAtRef.current = Date.now() + ms;
    setTimerState('running');
  };

  const handleRequestPauseTimer = () => {
    if (timerState !== 'running' || remainingMs <= 0) return;
    pendingPauseReasonRef.current = 'pause';
    setModal('confirm-pause');
  };

  const handleConfirmPause = () => {
    const left =
      timerEndAtRef.current !== null
        ? Math.max(0, timerEndAtRef.current - Date.now())
        : remainingMs;
    timerEndAtRef.current = null;
    setRemainingMs(left);
    setTimerState('paused');
    if (pendingPauseReasonRef.current === 'leave-study') {
      setActivityMode('chill');
    }
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
    setRemainingMs(ms);
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

  const platformTip = useMemo(() => {
    if (typeof navigator === 'undefined') return null;
    const ua = navigator.userAgent;
    if (/Mac|iPod|iPhone|iPad/.test(ua)) {
      return 'macOS 提示：如无法直接捕获系统声音，可选择标签页共享并勾选音频，或直接使用「本地音乐」。';
    }
    if (/Firefox/.test(ua)) {
      return 'Firefox 提示：请选择「标签页」共享并勾选分享音频，或直接使用「本地音乐」。';
    }
    return null;
  }, []);

  const silenceText =
    !audioError && silenceHint
      ? '似乎没有捕获到声音：请确认音乐正在播放，并已在共享弹窗里勾选「分享音频」。'
      : null;

  return (
    <div
      className={`anime-desktop-bg relative flex h-full w-full overflow-hidden ${isDragging ? 'dragging' : ''}`}
      onDrop={handleDrop}
      onDragOver={(e) => e.preventDefault()}
    >
      {/* Subtle Pastel Dot Background */}
      <div className="anime-desktop-dots pointer-events-none absolute inset-0" />

      {/* Top-Left Brand & Asset Status Badge */}
      <header className="pointer-events-none absolute left-4 top-3.5 z-10 flex flex-wrap items-center gap-2">
        <div className="anime-pill pointer-events-auto flex items-center gap-2 rounded-2xl px-3.5 py-2">
          <span className="flex h-7 w-7 items-center justify-center rounded-xl border border-[#6E4F75] bg-gradient-to-br from-[#FFD4E8] to-[#E9D5FF] text-sm">
            🎵
          </span>
          <div>
            <h1 className="text-[14px] font-extrabold tracking-wide text-[#4A3246]">
              桌宠节奏师
            </h1>
            <p className="text-[10.5px] font-medium text-[#755873]">
              二次元桌面伴侣 · 听歌律动 &amp; 自习打卡
            </p>
          </div>
        </div>

        <div
          data-testid="asset-status"
          className={`anime-pill pointer-events-auto rounded-full px-3 py-1 text-[11px] font-bold ${
            assetStatus.error
              ? 'text-[#BE123C]'
              : assetStatus.loaded
                ? 'text-[#6D28D9]'
                : 'text-[#8A6B89]'
          }`}
          title={assetStatus.assetDirHint ?? ''}
        >
          {assetStatus.error
            ? `⚠ ${assetStatus.error}`
            : assetStatus.loaded
              ? `✨ ${assetStatus.packName ?? '角色素材已就绪'}`
              : '⏳ 正在加载角色素材…'}
        </div>
      </header>

      {/* Main Layout: Left/Center Stage + Right Control Window */}
      <div className="relative z-20 flex h-full w-full items-center justify-between gap-3 px-4 pb-3 pt-16 sm:px-5">
        {/* Left/Center Character Stage */}
        <div className="relative flex h-full min-w-0 flex-1 items-center justify-center">
          <div
            ref={stageContainerRef}
            data-testid="character-stage-wrapper"
            style={
              dragPos
                ? { position: 'fixed', left: dragPos.x, top: dragPos.y, zIndex: 25 }
                : undefined
            }
            onPointerDown={handlePointerDown}
            onPointerMove={handlePointerMove}
            onPointerUp={handlePointerUp}
            onPointerCancel={handlePointerUp}
            onDoubleClick={() => setCollapsed((v) => !v)}
            className={`relative aspect-square w-[min(540px,95%,74vh)] touch-none transition-shadow ${
              isDragging ? 'cursor-grabbing' : 'cursor-grab'
            }`}
          >
            {/* Friendly Bubble Above Head (Never Covers Face) */}
            {!musicRunning && timerState !== 'running' && (
              <div className="pointer-events-none absolute -top-3 left-1/2 z-30 -translate-x-1/2 whitespace-nowrap">
                <div className="anime-pill rounded-full px-3.5 py-1 text-[11.5px] font-bold text-[#583B5E]">
                  {audioBusy
                    ? '正在连接音源…'
                    : '选个音源跟拍，或开启自习陪你一起加油 ✨'}
                </div>
              </div>
            )}

            <div className="h-full w-full overflow-hidden rounded-[30px] border-2 border-[#E6C7DA]/80 bg-white/40 shadow-[0_20px_48px_-14px_rgba(110,79,117,0.22)]">
              <DeskCharacterScene
                motionRef={motionRef}
                musicRunning={musicRunning}
                autoFollow={autoFollow}
                activityMode={activityMode}
                timerActive={timerState === 'running'}
                emotion={emotion}
                eventSwitches={eventSwitches}
                assetReloadToken={assetReloadToken}
                onAssetStatus={setAssetStatus}
              />
            </div>
          </div>
        </div>

        {/* Right Floating Control Window (or Collapsed Compact Pill) */}
        {collapsed ? (
          <div
            data-testid="collapsed-bar"
            className="anime-window fixed bottom-4 right-4 z-30 flex items-center gap-2.5 rounded-2xl px-3.5 py-2"
          >
            <span className="text-[12.5px] font-extrabold text-[#4A3246]">🎵 桌宠节奏师</span>
            {musicRunning && (
              <span className="rounded-full bg-[#F3E8FF] px-2 py-0.5 font-mono text-[11px] font-bold text-[#7C3AED]">
                {telemetry.bpm > 0 ? `${telemetry.bpm} BPM` : '测速中'}
              </span>
            )}
            <button
              type="button"
              data-testid="btn-reload-assets"
              onClick={() => setAssetReloadToken((n) => n + 1)}
              className="cursor-pointer rounded-xl border border-[#D7C1D9] bg-white px-2 py-1 text-[11px] font-bold text-[#6E4F75] hover:bg-[#FFF0F6]"
              title="重载素材"
            >
              🔄
            </button>
            <button
              type="button"
              data-testid="btn-toggle-collapse"
              onClick={() => setCollapsed(false)}
              className="cursor-pointer rounded-xl border-2 border-[#6E4F75] bg-[#FFF2B8] px-2.5 py-1 text-[11.5px] font-bold text-[#4A3246] hover:bg-[#FFE991]"
              title="展开面板 (H)"
            >
              展开 [+]
            </button>
          </div>
        ) : (
          <aside className="z-30 flex max-h-[calc(100vh-28px)] w-[404px] shrink-0 flex-col">
            {/* Main Window Box */}
            <div
              data-testid="main-control-window"
              className="anime-window flex min-h-0 flex-col overflow-hidden rounded-[26px]"
            >
              {/* Title Bar */}
              <div className="flex shrink-0 items-center justify-between gap-2 border-b-2 border-[#6E4F75] bg-gradient-to-r from-[#FFE4F1] via-[#F3E8FF] to-[#FFF4D1] px-3.5 py-2.5">
                <div className="flex items-center gap-2">
                  <span className="flex h-6 w-6 items-center justify-center rounded-lg border border-[#6E4F75] bg-white text-xs">
                    ✦
                  </span>
                  <span className="text-[13.5px] font-extrabold text-[#4A3246]">
                    桌宠节奏师
                  </span>
                </div>

                <div className="flex items-center gap-1.5">
                  <button
                    type="button"
                    data-testid="btn-reload-assets"
                    onClick={() => setAssetReloadToken((n) => n + 1)}
                    className="cursor-pointer rounded-xl border border-[#CFAFC9] bg-white/90 px-2 py-1 text-[11px] font-bold text-[#583B5E] transition hover:bg-white active:translate-y-0.5"
                    title="重载角色素材 (无需重启)"
                  >
                    🔄 重载素材
                  </button>
                  <button
                    type="button"
                    data-testid="btn-toggle-collapse"
                    onClick={() => setCollapsed(true)}
                    className="flex h-7 w-7 cursor-pointer items-center justify-center rounded-xl border border-[#6E4F75] bg-white/90 text-[12px] font-extrabold text-[#4A3246] transition hover:bg-[#FFF2B8]"
                    title="收起面板 (快捷键 H)"
                  >
                    —
                  </button>
                </div>
              </div>

              {/* 3 Unlocked Tabs */}
              <div className="grid shrink-0 grid-cols-3 gap-1.5 border-b border-[#EBD8E7] bg-[#FFF8FC] px-3 py-2">
                <button
                  type="button"
                  data-testid="tab-status"
                  onClick={() => setActiveTab('status')}
                  className={`flex cursor-pointer items-center justify-center gap-1 whitespace-nowrap rounded-xl border-2 px-1.5 py-1.5 text-[11px] font-bold transition ${
                    activeTab === 'status'
                      ? 'border-[#6E4F75] bg-[#FFF5D6] text-[#4A3246] shadow-[0_2px_0_#E6CE80]'
                      : 'border-transparent bg-white/60 text-[#755873] hover:border-[#DFC6D8] hover:bg-white'
                  }`}
                >
                  <ClockIcon size={14} />
                  <span>Status 状态</span>
                </button>

                <button
                  type="button"
                  data-testid="tab-emotion"
                  onClick={() => setActiveTab('emotion')}
                  className={`flex cursor-pointer items-center justify-center gap-1 whitespace-nowrap rounded-xl border-2 px-1.5 py-1.5 text-[11px] font-bold transition ${
                    activeTab === 'emotion'
                      ? 'border-[#6E4F75] bg-[#FFE4EF] text-[#4A3246] shadow-[0_2px_0_#E6B9CE]'
                      : 'border-transparent bg-white/60 text-[#755873] hover:border-[#DFC6D8] hover:bg-white'
                  }`}
                >
                  <HeartIcon size={14} />
                  <span>Emotion 情绪</span>
                </button>

                <button
                  type="button"
                  data-testid="tab-metronome"
                  onClick={() => setActiveTab('metronome')}
                  className={`flex cursor-pointer items-center justify-center gap-1 whitespace-nowrap rounded-xl border-2 px-1.5 py-1.5 text-[11px] font-bold transition ${
                    activeTab === 'metronome'
                      ? 'border-[#6E4F75] bg-[#EDE9FE] text-[#4A3246] shadow-[0_2px_0_#C7B8F5]'
                      : 'border-transparent bg-white/60 text-[#755873] hover:border-[#DFC6D8] hover:bg-white'
                  }`}
                >
                  <MetronomeIcon size={14} />
                  <span>Metronome 节拍器</span>
                </button>
              </div>

              {/* Active Tab Content */}
              <div className="anime-scroll min-h-0 flex-1 overflow-y-auto px-3.5 py-3">
                {activeTab === 'status' && (
                  <StatusTab
                    activityMode={activityMode}
                    onSelectActivityMode={handleSelectActivityMode}
                    setTimeEnabled={setTimeEnabled}
                    onToggleSetTime={() => setSetTimeEnabled((v) => !v)}
                    studyMinutes={studyMinutes}
                    onChangeStudyMinutes={handleChangeStudyMinutes}
                    timerState={timerState}
                    remainingMs={remainingMs}
                    totalMs={totalMs}
                    onStartOrResumeTimer={handleStartOrResumeTimer}
                    onRequestPauseTimer={handleRequestPauseTimer}
                    onResetTimer={handleResetTimer}
                    eventSwitches={eventSwitches}
                    onToggleEventSwitch={handleToggleEventSwitch}
                    todoOpen={todoOpen}
                    onToggleTodo={() => setTodoOpen((v) => !v)}
                  />
                )}

                {activeTab === 'emotion' && (
                  <EmotionTab emotion={emotion} onSelectEmotion={setEmotion} />
                )}

                {activeTab === 'metronome' && (
                  <MetronomeTab
                    mode={audioMode}
                    running={musicRunning}
                    busy={audioBusy}
                    bpm={telemetry.bpm}
                    level={telemetry.level}
                    bass={telemetry.bass}
                    beats={telemetry.beats}
                    locked={telemetry.locked}
                    sensitivity={sensitivity}
                    fileName={fileName}
                    error={audioError}
                    silenceHint={silenceText}
                    platformTip={platformTip}
                    autoCheckBpm={autoCheckBpm}
                    onToggleAutoCheckBpm={() => setAutoCheckBpm((v) => !v)}
                    autoFollow={autoFollow}
                    onToggleAutoFollow={() => setAutoFollow((v) => !v)}
                    onSelectMode={(mode) => void startAudio(mode)}
                    onSelectFile={(file) => void startAudio('file', file)}
                    onSensitivityChange={setSensitivity}
                    onStop={stopAudio}
                  />
                )}
              </div>

              {/* Shortcut Footer Hint */}
              <div className="flex shrink-0 items-center justify-between border-t border-[#EFE1EC] bg-[#FFF9FC] px-3.5 py-1.5 text-[10.5px] font-semibold text-[#8A6B89]">
                <span>Space 开始/停止 · Q 急停 · H 收起</span>
                <span>拖拽人物可移动位置</span>
              </div>
            </div>

            {/* Connected Todo Drawer Hanging Below Main Window */}
            {activeTab === 'status' && (
              <TodoDrawer
                open={todoOpen}
                listName={todoListName}
                onListNameChange={setTodoListName}
                items={todoItems}
                onToggleItem={handleToggleTodoItem}
                onUpdateItemText={handleUpdateTodoText}
                onAddItem={handleAddTodoItem}
                onRemoveItem={handleRemoveTodoItem}
                onRequestSave={() => setModal('confirm-save-todo')}
                savedNotice={todoSavedNotice}
              />
            )}
          </aside>
        )}
      </div>

      {/* Confirmation & Reminder Modals */}
      <Modals
        modal={modal}
        onConfirmPause={handleConfirmPause}
        onRejectPause={handleRejectPause}
        onDismissGoBack={handleDismissGoBack}
        onConfirmSaveTodo={handleConfirmSaveTodo}
        onCancelSaveTodo={() => setModal('none')}
      />
    </div>
  );
}
