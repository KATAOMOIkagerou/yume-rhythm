import { useEffect, useState } from 'react';
import { useSettings } from '../state/SettingsContext';
import type { ExpressionDefinition } from '../types/contracts';

export function EmotionTab({ expressions, selectedId, musicRunning, onSelectExpression }: {
  expressions: ExpressionDefinition[];
  selectedId: string;
  musicRunning: boolean;
  onSelectExpression: (id: string) => void;
}) {
  const { settings, updateSettings } = useSettings();
  const [intervalDraft, setIntervalDraft] = useState(String(settings.randomIntervalSeconds));
  useEffect(() => setIntervalDraft(String(settings.randomIntervalSeconds)), [settings.randomIntervalSeconds]);
  return <div className="space-y-3">
    <div className="anime-card-sakura rounded-2xl p-3.5 space-y-3">
      <p className="font-bold">当前表情：{expressions.find((entry) => entry.id === selectedId)?.name ?? '自然'}</p>
      {(['base', 'accessory'] as const).map((type) => <section key={type}>
        <h2 className="text-sm font-bold mb-2">{type === 'base' ? '整体待机表情' : '配件与局部表情'}</h2>
        <div className="grid grid-cols-2 gap-2">
          {expressions.filter((entry) => entry.type === type).map((entry) =>
            <button type="button" key={entry.id} data-testid={`emotion-${entry.id}`}
              disabled={musicRunning && entry.event === 'doze'}
              title={musicRunning && entry.event === 'doze' ? '音乐同步期间禁止打瞌睡' : entry.name}
              aria-pressed={selectedId === entry.id} onClick={() => onSelectExpression(entry.id)}
              className={`action-button ${selectedId === entry.id ? 'selected-expression' : ''}`}>
              {entry.name}
            </button>)}
        </div>
      </section>)}
      {!expressions.length && <p>正在加载表情配置…</p>}
      <p className="text-xs">再次点击选中的表情即可恢复自然表情。配件表情保留角色主体，只更换局部图层。</p>
    </div>
    <div className="anime-card-lavender rounded-2xl p-3.5 space-y-3 text-sm">
      <label className="flex gap-2 items-center">
        <input data-testid="random-enabled" type="checkbox" checked={settings.randomEnabled}
          onChange={(e) => updateSettings({ randomEnabled: e.target.checked })} />定时随机表情
      </label>
      <label className="flex gap-2 items-center">随机间隔
        <input data-testid="random-interval" aria-label="随机表情间隔秒数" type="number" min={5} max={86400}
          value={intervalDraft} className="w-24 rounded border bg-white px-2 py-1"
          onChange={(e) => {
            setIntervalDraft(e.target.value);
            const value = Number(e.target.value);
            if (Number.isFinite(value) && value >= 5 && value <= 86400) updateSettings({ randomIntervalSeconds: value });
          }} onBlur={() => {
            const raw = Number(intervalDraft);
            const value = Number.isFinite(raw) && intervalDraft.trim() ? Math.max(5, Math.min(86400, Math.trunc(raw))) : settings.randomIntervalSeconds;
            setIntervalDraft(String(value));
            updateSettings({ randomIntervalSeconds: value });
          }} />秒
      </label>
      <p className="text-xs">随机播放结束后恢复所选表情；音乐同步期间禁用打瞌睡。</p>
    </div>
  </div>;
}
