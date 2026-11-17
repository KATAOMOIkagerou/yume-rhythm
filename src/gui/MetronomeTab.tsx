import React, { useEffect, useRef } from 'react';
import type { AudioSourceMode } from '../types/contracts';
import { CheckIcon, LockIcon, MetronomeIcon } from './Icons';

const LEVEL_SEGMENTS = 24;

const SOURCE_OPTIONS: { key: Exclude<AudioSourceMode, 'idle'>; label: string; icon: string; hint: string }[] = [
  { key: 'system', label: '电脑音频', icon: '🖥️', hint: '共享屏幕 / 标签页音频，直接听到系统里正在播放的音乐' },
  { key: 'file', label: '本地音乐', icon: '🎵', hint: '选择本地音频文件（也支持直接拖入窗口），节拍精准同步' },
  { key: 'mic', label: '麦克风', icon: '🎤', hint: '把音乐外放，用麦克风收听（安静环境效果最好）' },
  { key: 'demo', label: '演示鼓点', icon: '🥁', hint: '内置合成鼓机，无需任何音源即可预览跟拍效果' },
];

export interface MetronomeTabProps {
  mode: AudioSourceMode;
  running: boolean;
  busy: boolean;
  bpm: number;
  level: number;
  bass: number;
  beats: number;
  locked: boolean;
  sensitivity: number;
  fileName: string;
  error: string | null;
  silenceHint: string | null;
  platformTip: string | null;
  autoCheckBpm: boolean;
  onToggleAutoCheckBpm: () => void;
  autoFollow: boolean;
  onToggleAutoFollow: () => void;
  onSelectMode: (mode: Exclude<AudioSourceMode, 'idle' | 'file'>) => void;
  onSelectFile: (file: File) => void;
  onSensitivityChange: (value: number) => void;
  onStop: () => void;
}

export function MetronomeTab({
  mode,
  running,
  busy,
  bpm,
  level,
  bass,
  beats,
  locked,
  sensitivity,
  fileName,
  error,
  silenceHint,
  platformTip,
  autoCheckBpm,
  onToggleAutoCheckBpm,
  autoFollow,
  onToggleAutoFollow,
  onSelectMode,
  onSelectFile,
  onSensitivityChange,
  onStop,
}: MetronomeTabProps) {
  const fileInputRef = useRef<HTMLInputElement>(null);
  const beatDotRef = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    const dot = beatDotRef.current;
    if (!dot || beats === 0) return;
    dot.classList.remove('beating');
    void dot.offsetWidth;
    dot.classList.add('beating');
  }, [beats]);

  const activeSource = SOURCE_OPTIONS.find((item) => item.key === mode);
  const statusText = busy
    ? '正在连接…'
    : running
      ? `跟随中 · ${activeSource?.label ?? ''}`
      : '未开始';

  const displayedBpm = autoCheckBpm && bpm > 0 ? String(bpm) : '--';

  const handleSourceClick = (key: Exclude<AudioSourceMode, 'idle'>) => {
    if (key === 'file') {
      fileInputRef.current?.click();
      return;
    }
    onSelectMode(key);
  };

  return (
    <div className="space-y-2.5">
      {/* Live Telemetry Summary */}
      <div className="anime-card-lavender rounded-2xl px-3.5 py-2.5">
        <div className="flex items-center gap-2.5">
          <span
            ref={beatDotRef}
            className={`beat-dot h-3 w-3 shrink-0 rounded-full border border-[#6E4F75] ${
              running ? 'bg-[#EC4899]' : 'bg-[#D8CBE0]'
            }`}
          />
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-1.5">
              <span
                data-testid="music-status"
                className="truncate text-[13px] font-bold text-[#4A3246]"
              >
                {statusText}
              </span>
              {mode === 'file' && running && fileName && (
                <span
                  data-testid="current-file-name"
                  className="truncate rounded-full bg-white/80 px-2 py-0.5 text-[10.5px] font-semibold text-[#7C3AED]"
                >
                  {fileName}
                </span>
              )}
            </div>
          </div>

          <div className="flex items-baseline gap-1">
            <span className="text-[11px] font-bold text-[#755873]">BPM</span>
            <span
              data-testid="bpm-value"
              className={`min-w-[44px] text-right font-mono text-2xl font-extrabold tabular-nums ${
                autoCheckBpm && bpm > 0 ? 'text-[#7C3AED]' : 'text-[#B8A5C0]'
              }`}
            >
              {displayedBpm}
            </span>
          </div>
        </div>

        <div className="mt-1.5 flex items-center justify-between gap-2">
          {running ? (
            <span
              data-testid="lock-badge"
              className={`rounded-full border px-2 py-0.5 text-[11px] font-bold ${
                locked
                  ? 'border-[#86EFAC] bg-[#DCFCE7] text-[#166534]'
                  : 'border-[#FCD34D] bg-[#FEF3C7] text-[#92400E]'
              }`}
            >
              {locked ? '🔒 节拍已锁定' : '🔍 搜索节拍…'}
            </span>
          ) : (
            <span className="text-[11px] font-medium text-[#8C708D]">
              选择下方音源即可开始听歌跟拍
            </span>
          )}

          <div className="flex items-center gap-2.5 font-mono text-[11px] font-semibold tabular-nums text-[#6E4F75]">
            <span data-testid="beat-count">♪ {beats} 拍</span>
            <span data-testid="bass-pct">低频 {Math.round(bass * 100)}%</span>
            <span data-testid="level-pct">音量 {Math.round(level * 100)}%</span>
          </div>
        </div>

        {/* 24-segment Level Meter */}
        <div
          data-testid="level-meter"
          className="mt-2 flex items-end gap-[2.5px] rounded-xl border border-[#DFD2F6] bg-white/80 px-2 py-1.5"
        >
          {Array.from({ length: LEVEL_SEGMENTS }).map((_, i) => {
            const mid = (LEVEL_SEGMENTS - 1) / 2;
            const weight = 1 - Math.abs(i - mid) / mid;
            const active = level * weight * 1.25 > (i / LEVEL_SEGMENTS) * 1.05;
            return (
              <div
                key={i}
                className={`level-bar h-2 flex-1 rounded-full ${
                  active
                    ? i > LEVEL_SEGMENTS * 0.72
                      ? 'bg-[#EC4899]'
                      : 'bg-[#8B5CF6]'
                    : 'bg-[#EDE6F7]'
                }`}
              />
            );
          })}
        </div>
      </div>

      {/* Auto Check BPM & Auto Follow + Press "Q" to stop */}
      <div className="anime-card-sakura rounded-2xl px-3.5 py-2">
        <div className="flex flex-wrap items-center justify-between gap-2">
          <div className="flex flex-wrap items-center gap-3">
            <button
              type="button"
              data-testid="chk-auto-check-bpm"
              onClick={onToggleAutoCheckBpm}
              className="flex cursor-pointer items-center gap-1.5 text-[12px] font-bold text-[#4A3246]"
            >
              <span
                className={`flex h-4.5 w-4.5 items-center justify-center rounded border-2 ${
                  autoCheckBpm
                    ? 'border-[#6E4F75] bg-[#F472B6] text-white'
                    : 'border-[#8D6E92] bg-white text-transparent'
                }`}
              >
                <CheckIcon size={11} />
              </span>
              <span>Auto check BPM (自动测 BPM)</span>
            </button>

            <button
              type="button"
              data-testid="chk-auto-follow"
              onClick={onToggleAutoFollow}
              className="flex cursor-pointer items-center gap-1.5 text-[12px] font-bold text-[#4A3246]"
            >
              <span
                className={`flex h-4.5 w-4.5 items-center justify-center rounded border-2 ${
                  autoFollow
                    ? 'border-[#6E4F75] bg-[#8B5CF6] text-white'
                    : 'border-[#8D6E92] bg-white text-transparent'
                }`}
              >
                <CheckIcon size={11} />
              </span>
              <span>Auto follow (自动跟拍)</span>
            </button>
          </div>

          <span
            data-testid="hint-press-q"
            className="rounded-lg border border-[#F2BFD7] bg-white px-2 py-0.5 text-[11px] font-bold text-[#BE185D]"
          >
            Press &quot;Q&quot; to stop / 按 Q 停止
          </span>
        </div>
      </div>

      {/* 4 Audio Sources + Sensitivity + Stop */}
      <div className="anime-card-cream rounded-2xl px-3.5 py-2.5">
        <div className="grid grid-cols-2 gap-2">
          {SOURCE_OPTIONS.map((item) => {
            const active = mode === item.key && running;
            return (
              <button
                key={item.key}
                type="button"
                data-testid={`btn-${item.key}`}
                disabled={busy}
                title={item.hint}
                onClick={() => handleSourceClick(item.key)}
                className={`flex items-center justify-center gap-1.5 rounded-xl border-2 px-2.5 py-1.5 text-[12.5px] font-bold transition ${
                  active
                    ? 'border-[#6E4F75] bg-[#EDE9FE] text-[#5B21B6] shadow-[0_2px_0_#C7B8F5]'
                    : 'border-[#E6C7DA] bg-white text-[#4A3246] hover:border-[#6E4F75] hover:bg-[#FFF5FA]'
                } ${busy ? 'cursor-not-allowed opacity-50' : 'cursor-pointer active:translate-y-0.5'}`}
              >
                <span>{item.icon}</span>
                <span>{item.label}</span>
              </button>
            );
          })}
        </div>

        <div className="mt-2.5 flex items-center gap-2.5">
          <label className="flex flex-1 items-center gap-2 text-[12px] font-bold text-[#4A3246]">
            <span className="shrink-0">灵敏度</span>
            <input
              type="range"
              data-testid="slider-sensitivity"
              min={1.05}
              max={2}
              step={0.01}
              value={sensitivity}
              onChange={(e) => onSensitivityChange(Number(e.target.value))}
              className="h-1.5 flex-1 cursor-pointer accent-[#EC4899]"
            />
            <span className="w-9 text-right font-mono text-[11.5px] font-bold text-[#7C3AED]">
              {sensitivity.toFixed(2)}
            </span>
          </label>

          <button
            type="button"
            data-testid="btn-stop"
            disabled={!running}
            onClick={onStop}
            className={`rounded-xl border-2 border-[#6E4F75] px-3 py-1 text-[12px] font-bold transition ${
              running
                ? 'cursor-pointer bg-[#FFE4E6] text-[#BE123C] shadow-[0_2px_0_#F6B2BE] hover:bg-[#FECDD3] active:translate-y-0.5'
                : 'cursor-not-allowed border-[#DECFE0] bg-white/60 text-[#B4A0B6]'
            }`}
          >
            ⏹ 停止
          </button>
        </div>

        <input
          ref={fileInputRef}
          type="file"
          accept="audio/*"
          data-testid="input-audio-file"
          className="hidden"
          onChange={(e) => {
            const file = e.target.files?.[0];
            if (file) onSelectFile(file);
            e.target.value = '';
          }}
        />
      </div>

      {/* Error / Hint / Tip */}
      {error && (
        <div
          data-testid="audio-error"
          className="rounded-xl border border-[#FDA4AF] bg-[#FFF1F2] px-3 py-1.5 text-[11.5px] font-semibold text-[#BE123C]"
        >
          ⚠ {error}
        </div>
      )}
      {!error && silenceHint && (
        <div
          data-testid="audio-silence-hint"
          className="rounded-xl border border-[#FCD34D] bg-[#FFFBEB] px-3 py-1.5 text-[11.5px] font-semibold text-[#92400E]"
        >
          💡 {silenceHint}
        </div>
      )}
      {!error && platformTip && (
        <div
          data-testid="audio-platform-tip"
          className="rounded-xl border border-[#BAE6FD] bg-[#F0F9FF] px-3 py-1.5 text-[11.5px] font-medium text-[#0369A1]"
        >
          ℹ️ {platformTip}
        </div>
      )}

      {/* Unimplemented Sketch Placeholders (Strictly Disabled) */}
      <div className="rounded-2xl border-2 border-dashed border-[#D8C5DC] bg-white/75 px-3 py-2">
        <div className="mb-1.5 flex items-center justify-between">
          <span className="flex items-center gap-1 text-[11.5px] font-bold text-[#6E4F75]">
            <MetronomeIcon size={14} />
            <span>草图规划扩展位</span>
          </span>
          <span className="inline-flex items-center gap-1 rounded-full bg-[#F3E8FF] px-2 py-0.5 text-[10px] font-bold text-[#7C3AED]">
            <LockIcon size={11} />
            待后续独立模块接入（音频冻结期未启用）
          </span>
        </div>

        <div className="space-y-1.5 text-[11.5px] text-[#8E7492]">
          <div
            data-testid="placeholder-set-bpm"
            className="flex items-center justify-between rounded-xl border border-[#E7DCE9] bg-[#FAF7FB] px-2.5 py-1 opacity-75"
          >
            <label className="flex items-center gap-1.5">
              <input type="checkbox" disabled checked={false} readOnly />
              <span>Set BPM for</span>
              <input
                type="text"
                disabled
                placeholder="___"
                className="w-11 rounded border border-[#D8C7DC] bg-white px-1 py-0.5 text-center font-mono text-[11px]"
              />
            </label>
            <button
              type="button"
              disabled
              className="cursor-not-allowed rounded-lg border border-[#D8C7DC] bg-white px-2 py-0.5 text-[10.5px] font-bold"
            >
              ▶
            </button>
          </div>

          <div className="grid grid-cols-2 gap-1.5">
            <label
              data-testid="placeholder-tap-follow"
              className="flex cursor-not-allowed items-center gap-1.5 rounded-xl border border-[#E7DCE9] bg-[#FAF7FB] px-2.5 py-1 opacity-75"
            >
              <input type="checkbox" disabled checked={false} readOnly />
              <span>Tap to follow (手敲跟拍)</span>
            </label>

            <label
              data-testid="placeholder-tap-check"
              className="flex cursor-not-allowed items-center gap-1.5 rounded-xl border border-[#E7DCE9] bg-[#FAF7FB] px-2.5 py-1 opacity-75"
            >
              <input type="checkbox" disabled checked={false} readOnly />
              <span>Tap to check BPM (手敲测速)</span>
            </label>
          </div>
        </div>
      </div>
    </div>
  );
}
