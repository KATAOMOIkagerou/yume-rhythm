import { useEffect, useState } from 'react';
import type { AudioSourceMode } from '../types/contracts';

export interface MetronomeTabProps {
  mode: AudioSourceMode; running: boolean; busy: boolean;
  bpm: number; level: number; bass: number; beats: number; locked: boolean;
  sensitivity: number; fileName: string; error: string | null; silenceHint: string | null;
  autoCheckBpm: boolean; onToggleAutoCheckBpm: () => void;
  autoFollow: boolean; onToggleAutoFollow: () => void;
  onSelectMode: (mode: 'system') => void;
  onSensitivityChange: (value: number) => void; onStop: () => void;
}

export function MetronomeTab(props: MetronomeTabProps) {
  const [devices, setDevices] = useState<MediaDeviceInfo[]>([]);
  const [deviceHint, setDeviceHint] = useState('');
  useEffect(() => {
    let disposed = false;
    const refresh = async () => {
      try {
        const all = await navigator.mediaDevices.enumerateDevices();
        if (!disposed) { setDevices(all.filter((item) => item.kind === 'audiooutput')); setDeviceHint(''); }
      } catch { if (!disposed) setDeviceHint('无法读取设备名称，将监听系统默认输出。'); }
    };
    void refresh();
    navigator.mediaDevices?.addEventListener('devicechange', refresh);
    return () => { disposed = true; navigator.mediaDevices?.removeEventListener('devicechange', refresh); };
  }, []);
  const defaultDevice = devices.find((item) => item.deviceId === 'default');
  const defaultName = defaultDevice?.label.replace(/^Default\s*-?\s*/i, '') || '跟随系统默认输出';
  return <div className="space-y-3 text-sm">
    <div className="anime-card-lavender rounded-2xl p-3.5 space-y-2">
      <p data-testid="music-status" className="font-bold">{props.busy ? '正在连接…' : props.running ? '正在音频同步' : '未开始'}</p>
      <p>速度：<strong data-testid="bpm-value">{props.autoCheckBpm && props.bpm > 0 ? props.bpm : '--'}</strong> 拍／分钟</p>
      <p data-testid="lock-badge">{props.running ? props.locked ? '节拍已锁定' : '正在搜索节拍…' : '等待音频'}</p>
      <p><span data-testid="beat-count">已跟随 {props.beats} 拍</span> · 音量 {Math.round(props.level * 100)}% · 低频 {Math.round(props.bass * 100)}%</p>
      <progress aria-label="音量" value={props.level} max={1} className="w-full" />
      {props.mode === 'file' && props.fileName && <p data-testid="current-file-name">{props.fileName}</p>}
    </div>
    <div className="anime-card-cream rounded-2xl p-3.5 space-y-3">
      <label className="block font-bold" htmlFor="listen-device">监听设备（输出设备）</label>
      <select id="listen-device" data-testid="output-device" value="default" onChange={() => {}}
        className="w-full rounded-lg border bg-white p-2" disabled={props.running || props.busy}>
        <option value="default">系统默认输出：{defaultName}</option>
        {devices.filter((item) => item.deviceId !== 'default').map((item, index) =>
          <option key={item.deviceId || index} value={item.deviceId} disabled>
            {item.label || `输出设备 ${index + 1}`}（请先设为系统默认）
          </option>)}
      </select>
      <p className="text-xs">当前版本监听系统默认输出。若要使用其他设备，请先在系统声音设置中将其设为默认输出，再重新开始监听。</p>
      {deviceHint && <p>{deviceHint}</p>}
      {window.desktopPetBridge && <button className="action-button" onClick={() => window.desktopPetBridge?.openSoundSettings()}>打开系统声音设置</button>}
      <div className="flex gap-2">
        <button className="action-button" data-testid="btn-system" disabled={props.busy || props.running}
          onClick={() => props.onSelectMode('system')}>开始监听电脑音频</button>
        <button className="action-button" data-testid="btn-stop" disabled={!props.running || props.busy} onClick={props.onStop}>停止</button>
      </div>
    </div>
    <div className="anime-card-sakura rounded-2xl p-3.5 space-y-2">
      <label className="flex items-center gap-2"><input type="checkbox" data-testid="chk-auto-check-bpm"
        checked={props.autoCheckBpm} onChange={props.onToggleAutoCheckBpm} />自动测速</label>
      <label className="flex items-center gap-2"><input type="checkbox" data-testid="chk-auto-follow"
        checked={props.autoFollow} onChange={props.onToggleAutoFollow} />自动跟拍</label>
      <p data-testid="hint-press-q" className="text-xs">按 Q 键停止</p>
    </div>
    <details className="anime-card-lavender rounded-2xl p-3.5">
      <summary className="cursor-pointer font-bold">高级设置</summary>
      <label className="mt-3 flex items-center gap-2">灵敏度
        <input data-testid="slider-sensitivity" type="range" min={1.05} max={2} step={0.01}
          value={props.sensitivity} onChange={(e) => props.onSensitivityChange(Number(e.target.value))} />
        {props.sensitivity.toFixed(2)}
      </label>
    </details>
    {props.error && <p role="alert" data-testid="audio-error" className="text-rose-700">{props.error}</p>}
    {!props.error && props.silenceHint && <p>{props.silenceHint}</p>}
  </div>;
}
