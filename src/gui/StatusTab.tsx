import React, { useState } from 'react';
import type {
  ActivityMode,
  EventContextKey,
  EventSwitchMatrix,
  RandomEventType,
} from '../types/contracts';
import type { TimerMode } from '../state/timer';
import { CheckIcon, ClockIcon, PauseIcon, PlayIcon, StripedProgressBar } from './Icons';

export type TimerState = 'idle' | 'running' | 'paused' | 'completed';

export interface StatusTabProps {
  timerMode: TimerMode;
  onChangeTimerMode: (mode: TimerMode) => void;
  activityMode: ActivityMode;
  onSelectActivityMode: (mode: ActivityMode) => void;
  studyMinutes: number;
  onChangeStudyMinutes: (minutes: number) => void;
  timerState: TimerState;
  remainingMs: number;
  totalMs: number;
  onStartOrResumeTimer: () => void;
  onRequestPauseTimer: () => void;
  onResetTimer: () => void;
  eventSwitches: EventSwitchMatrix;
  onToggleEventSwitch: (ctx: EventContextKey, ev: RandomEventType) => void;
  todoOpen: boolean;
  onToggleTodo: () => void;
}

function formatMs(ms: number): string {
  const totalSec = Math.max(0, Math.ceil(ms / 1000));
  const mins = Math.floor(totalSec / 60);
  const secs = totalSec % 60;
  return `${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}`;
}

const EVENT_ROWS: { key: RandomEventType; label: string }[] = [
  { key: 'doze', label: '打瞌睡' },
  { key: 'think', label: '思考' },
  { key: 'shock', label: '震惊' },
  { key: 'love', label: '喜欢' },
];

const CONTEXT_COLS: { key: EventContextKey; label: string }[] = [
  { key: 'chill', label: '休闲' },
  { key: 'study', label: '专注' },
  { key: 'music', label: '音乐' },
];

export function StatusTab({
  timerMode,
  onChangeTimerMode,
  activityMode,
  onSelectActivityMode,
  studyMinutes,
  onChangeStudyMinutes,
  timerState,
  remainingMs,
  totalMs,
  onStartOrResumeTimer,
  onRequestPauseTimer,
  onResetTimer,
  eventSwitches,
  onToggleEventSwitch,
  todoOpen,
  onToggleTodo,
}: StatusTabProps) {
  const [switchesExpanded, setSwitchesExpanded] = useState(false);
  const [draftMinutes, setDraftMinutes] = useState(String(studyMinutes));

  React.useEffect(() => {
    setDraftMinutes(String(studyMinutes));
  }, [studyMinutes]);

  const isRunning = timerState === 'running';
  const isPaused = timerState === 'paused';
  const isCompleted = timerState === 'completed';

  const progressRatio =
    totalMs > 0 ? Math.max(0, Math.min(100, (remainingMs / totalMs) * 100)) : 100;

  return (
    <div className="space-y-2.5">
      {/* Study / Chill Activity Mode */}
      <div className="anime-card-sakura rounded-2xl px-3.5 py-2.5">
        <div className="mb-2 flex items-center justify-between">
          <span className="text-[12px] font-bold text-[#6E4F75]">当前陪伴模式</span>
          <span className="rounded-full bg-white/90 px-2 py-0.5 text-[11px] font-semibold text-[#8A5A8E]">
            {activityMode === 'study' ? '💻 认真自习中' : '🍵 轻松休闲中'}
          </span>
        </div>

        <div className="grid grid-cols-2 gap-2.5">
          <button
            type="button"
            data-testid="mode-study"
            onClick={() => onSelectActivityMode('study')}
            className={`flex cursor-pointer items-center gap-2 rounded-xl border-2 px-3 py-2 text-left transition ${
              activityMode === 'study'
                ? 'border-[#6E4F75] bg-[#FFF5D6] shadow-[0_2px_0_#E6CE80]'
                : 'border-[#E6C7DA] bg-white/90 hover:border-[#6E4F75]'
            }`}
          >
            <span
              className={`flex h-4.5 w-4.5 shrink-0 items-center justify-center rounded border-2 ${
                activityMode === 'study'
                  ? 'border-[#6E4F75] bg-[#F472B6] text-white'
                  : 'border-[#8D6E92] bg-white text-transparent'
              }`}
            >
              <CheckIcon size={11} />
            </span>
            <span className="text-[12.5px] font-bold text-[#4A3246]">自习</span>
          </button>

          <button
            type="button"
            data-testid="mode-chill"
            onClick={() => onSelectActivityMode('chill')}
            className={`flex cursor-pointer items-center gap-2 rounded-xl border-2 px-3 py-2 text-left transition ${
              activityMode === 'chill'
                ? 'border-[#6E4F75] bg-[#EDE9FE] shadow-[0_2px_0_#C7B8F5]'
                : 'border-[#E6C7DA] bg-white/90 hover:border-[#6E4F75]'
            }`}
          >
            <span
              className={`flex h-4.5 w-4.5 shrink-0 items-center justify-center rounded border-2 ${
                activityMode === 'chill'
                  ? 'border-[#6E4F75] bg-[#8B5CF6] text-white'
                  : 'border-[#8D6E92] bg-white text-transparent'
              }`}
            >
              <CheckIcon size={11} />
            </span>
            <span className="text-[12.5px] font-bold text-[#4A3246]">休闲</span>
          </button>
        </div>
      </div>

      <label className="flex items-center justify-between gap-2 text-sm font-bold">计时模式
        <select data-testid="timer-mode" value={timerMode} disabled={isRunning || isPaused}
          onChange={(e) => onChangeTimerMode(e.target.value as TimerMode)} className="rounded border bg-white p-2">
          <option value="countdown">倒计时</option><option value="stopwatch">正计时（秒表）</option>
        </select>
      </label>
      {/* Timer Control Card */}
      <div className="anime-card-cream rounded-2xl px-3.5 py-2.5">
        <div className="flex items-center justify-between gap-2">
          <div className="flex items-center gap-1.5" hidden={timerMode === 'stopwatch'}>
            <label htmlFor="study-minutes">设置时间</label>

            <input
              id="study-minutes"
              type="number"
              min={1}
              max={999}
              data-testid="input-study-minutes"
              disabled={isRunning || isPaused}
              value={draftMinutes}
              onChange={(e) => {
                const val = e.target.value;
                setDraftMinutes(val);
                if (val === '') return;
                const raw = parseInt(val, 10);
                if (Number.isFinite(raw)) {
                  onChangeStudyMinutes(Math.max(1, Math.min(999, raw)));
                }
              }}
              onBlur={() => {
                if (draftMinutes === '') {
                  setDraftMinutes(String(studyMinutes));
                } else {
                  const raw = parseInt(draftMinutes, 10);
                  const clamped = Number.isFinite(raw) ? Math.max(1, Math.min(999, raw)) : studyMinutes;
                  onChangeStudyMinutes(clamped);
                  setDraftMinutes(String(clamped));
                }
              }}
              className="w-14 rounded-lg border-2 border-[#D4BC7E] bg-white px-1.5 py-0.5 text-center font-mono text-[13px] font-bold text-[#4A3246] outline-none focus:border-[#6E4F75] disabled:bg-[#FFF9E6] disabled:text-[#8E7668]"
            />
            <span className="text-[12px] font-bold text-[#6E4F75]">分钟</span>
          </div>

          <div className="flex items-center gap-1.5 text-[#6E4F75]">
            <ClockIcon size={14} />
            <span className="text-[11px] font-semibold">
              {isRunning ? '计时进行中' : isPaused ? '已暂停' : isCompleted ? '已完成' : '待启动'}
            </span>
          </div>
        </div>

        {/* Big Countdown + Play / Pause Buttons */}
        <div className="mt-2.5 flex items-center justify-between gap-3 rounded-xl border border-[#ECD89E] bg-white/85 px-3 py-2">
          <div
            data-testid="timer-display"
            className={`font-mono text-2xl font-extrabold tracking-wider tabular-nums ${
              isRunning
                ? 'text-[#DB2777]'
                : isPaused
                  ? 'text-[#7C3AED]'
                  : isCompleted
                    ? 'text-[#16A34A]'
                    : 'text-[#4A3246]'
            }`}
          >
            {formatMs(timerMode === 'stopwatch' ? Math.floor(remainingMs / 1000) * 1000 : remainingMs)}
          </div>

          <div className="flex items-center gap-2">
            <button
              type="button"
              data-testid="btn-timer-play"
              disabled={isRunning}
              onClick={onStartOrResumeTimer}
              title={isPaused ? '继续自习计时' : '开始自习计时'}
              className={`flex h-8 w-10 items-center justify-center rounded-xl border-2 border-[#6E4F75] transition ${
                isRunning
                  ? 'cursor-not-allowed bg-[#F3E8FF] text-[#B8A2C6] opacity-60'
                  : 'cursor-pointer bg-[#DCFCE7] text-[#15803D] shadow-[0_2px_0_#A7DFBA] hover:bg-[#BBF7D0] active:translate-y-0.5'
              }`}
            >
              <PlayIcon size={15} />
            </button>

            <button
              type="button"
              data-testid="btn-timer-pause"
              disabled={!isRunning}
              onClick={onRequestPauseTimer}
              title="暂停计时"
              className={`flex h-8 w-10 items-center justify-center rounded-xl border-2 border-[#6E4F75] transition ${
                !isRunning
                  ? 'cursor-not-allowed bg-[#F8F3FA] text-[#C5B0C7] opacity-60'
                  : 'cursor-pointer bg-[#FFE4EF] text-[#BE185D] shadow-[0_2px_0_#E6B9CE] hover:bg-[#FFD0E3] active:translate-y-0.5'
              }`}
            >
              <PauseIcon size={15} />
            </button>
          </div>
        </div>

        {/* Completion Message */}
        {isCompleted && (
          <div
            data-testid="timer-completed-banner"
            className="mt-2 flex items-center justify-between rounded-xl border border-[#86EFAC] bg-[#ECFDF5] px-2.5 py-1.5 text-[11.5px] font-bold text-[#166534]"
          >
            <span>🎉 太棒了！本次自习时间到啦！</span>
            <button
              type="button"
              onClick={onResetTimer}
              className="cursor-pointer rounded-lg bg-white px-2 py-0.5 text-[11px] font-bold text-[#15803D] shadow-xs hover:bg-[#DCFCE7]"
            >
              再来一轮
            </button>
          </div>
        )}

        {/* Pausing Bar with Striped Progress Bar */}
        {isPaused && (
          <div
            data-testid="pausing-bar"
            className="mt-2.5 rounded-xl border-2 border-dashed border-[#A78BFA] bg-[#F8F5FF] p-2.5"
          >
            <div className="mb-1.5 flex items-center justify-between text-[11.5px] font-bold text-[#5B21B6]">
              <span>已暂停，保留进度</span>
              <span data-testid="pausing-remaining" className="font-mono tabular-nums">
                {timerMode === 'stopwatch' ? '已计时 ' : '剩余 '}{formatMs(timerMode === 'stopwatch' ? Math.floor(remainingMs / 1000) * 1000 : remainingMs)}
              </span>
            </div>

            {timerMode === 'countdown' && <StripedProgressBar progress={progressRatio} animated />}

            <div className="mt-2 flex items-center justify-end gap-2">
              <button
                type="button"
                data-testid="btn-timer-reset"
                onClick={onResetTimer}
                className="cursor-pointer rounded-xl border border-[#C4B5FD] bg-white px-2.5 py-1 text-[11px] font-bold text-[#6D28D9] transition hover:bg-[#F3E8FF]"
              >
                ↺ 重置
              </button>
              <button
                type="button"
                data-testid="btn-timer-continue"
                onClick={onStartOrResumeTimer}
                className="cursor-pointer rounded-xl border-2 border-[#6E4F75] bg-[#FFF2B8] px-3 py-1 text-[11.5px] font-bold text-[#4A3246] shadow-[0_2px_0_#E6CE80] transition hover:bg-[#FFE991] active:translate-y-0.5"
              >
                继续
              </button>
            </div>
          </div>
        )}
      </div>

      {/* Random Action Event Matrix Card */}
      <div className="anime-card-lavender rounded-2xl px-3.5 py-2">
        <button
          type="button"
          data-testid="btn-toggle-event-switches"
          onClick={() => setSwitchesExpanded((v) => !v)}
          className="flex w-full cursor-pointer items-center justify-between text-left"
        >
          <span className="text-[12px] font-bold text-[#4A3246]">⚙ 随机动作开关</span>
          <span className="text-[11px] font-semibold text-[#7C3AED]">
            {switchesExpanded ? '收起 ▲' : '展开设置 ▼'}
          </span>
        </button>

        {switchesExpanded && (
          <div data-testid="event-switches-panel" className="mt-2.5 border-t border-[#D8C7F5] pt-2">
            <table className="w-full text-center text-[11.5px]">
              <thead>
                <tr className="text-[#6E4F75]">
                  <th className="py-1 text-left font-bold">动作</th>
                  {CONTEXT_COLS.map((col) => (
                    <th key={col.key} className="py-1 font-bold">
                      {col.label}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody className="divide-y divide-[#E7DCFA]">
                {EVENT_ROWS.map((row) => (
                  <tr key={row.key}>
                    <td className="py-1.5 text-left font-semibold text-[#4A3246]">{row.label}</td>
                    {CONTEXT_COLS.map((col) => {
                      const isLockedOff = col.key === 'music' && row.key === 'doze';
                      const checked = isLockedOff ? false : eventSwitches[col.key][row.key];
                      return (
                        <td key={col.key} className="py-1.5">
                          <input
                            type="checkbox"
                            data-testid={`switch-${col.key}-${row.key}`}
                            checked={checked}
                            disabled={isLockedOff}
                            title={
                              isLockedOff
                                ? '音乐跟拍期禁止打瞌睡'
                                : `${col.label}模式下允许${row.label}`
                            }
                            onChange={() => onToggleEventSwitch(col.key, row.key)}
                            className={`h-4 w-4 accent-[#EC4899] ${
                              isLockedOff ? 'cursor-not-allowed opacity-50' : 'cursor-pointer'
                            }`}
                          />
                        </td>
                      );
                    })}
                  </tr>
                ))}
              </tbody>
            </table>
            <p className="mt-1.5 text-[10.5px] font-medium text-[#7C3AED]">
              🔒 注：音乐跟拍期禁止打瞌睡（音乐 × 打瞌睡已强制锁定禁用）
            </p>
          </div>
        )}
      </div>

      {/* Bottom-right To Do List Trigger */}
      <div className="flex justify-end pt-0.5">
        <button
          type="button"
          data-testid="btn-toggle-todo"
          onClick={onToggleTodo}
          className={`cursor-pointer rounded-2xl border-2 border-[#6E4F75] px-3.5 py-1.5 text-[12px] font-bold text-[#4A3246] transition active:translate-y-0.5 ${
            todoOpen
              ? 'bg-[#FFE4EF] shadow-[0_2px_0_#E6B9CE]'
              : 'bg-[#FFF5D6] shadow-[0_2px_0_#E6CE80] hover:bg-[#FFECAC]'
          }`}
        >
          📝 待办清单 {todoOpen ? '▲' : '▼'}
        </button>
      </div>
    </div>
  );
}
