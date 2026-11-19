import { useSettings } from '../state/SettingsContext';

export function SettingsTab({ locked, onToggleLock }: { locked: boolean; onToggleLock: () => void }) {
  const { settings, updateSettings } = useSettings();
  return <div className="space-y-3 text-sm">
    <div className="anime-card-cream rounded-2xl p-4">
      <label className="flex items-center gap-2">
        <input data-testid="setting-end-sound" type="checkbox" checked={settings.endSoundEnabled}
          onChange={(e) => updateSettings({ endSoundEnabled: e.target.checked })} />
        倒计时结束音效
      </label>
      <p className="mt-2 text-xs">开启后，自习倒计时结束时播放提示音。</p>
    </div>
    <div className="anime-card-lavender rounded-2xl p-4 space-y-2">
      <button className="action-button" data-testid="btn-settings-lock" onClick={onToggleLock}>
        {locked ? '解除桌宠锁定' : '锁定桌宠并开启点击穿透'}
      </button>
      <p className="text-xs">锁定后可在本页、托盘菜单或按 Ctrl＋Alt＋L 解除锁定。</p>
      <p className="text-xs">Ctrl＋Alt＋C：打开控制台。关闭控制台后计时和音频同步继续运行。</p>
    </div>
    {window.desktopPetBridge && <button className="action-button" onClick={() => window.desktopPetBridge?.quit()}>退出桌宠节奏师</button>}
  </div>;
}
