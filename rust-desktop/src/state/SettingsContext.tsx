import { createContext, useContext, useEffect, useState, type ReactNode } from 'react';
import type { GlobalSettings } from '../types/contracts';

const defaults: GlobalSettings = {
  endSoundEnabled: true, randomEnabled: true, randomIntervalSeconds: 120,
};
const KEY = 'desk_pet_settings_v1';
function loadSettings(): GlobalSettings {
  try {
    const raw = JSON.parse(localStorage.getItem(KEY) ?? '{}');
    return {
      endSoundEnabled: typeof raw.endSoundEnabled === 'boolean' ? raw.endSoundEnabled : true,
      randomEnabled: typeof raw.randomEnabled === 'boolean' ? raw.randomEnabled : true,
      randomIntervalSeconds: Number.isFinite(raw.randomIntervalSeconds)
        ? Math.max(5, Math.min(86400, raw.randomIntervalSeconds)) : 120,
    };
  } catch { return defaults; }
}
const SettingsContext = createContext<{
  settings: GlobalSettings;
  updateSettings: (patch: Partial<GlobalSettings>) => void;
} | null>(null);

export function SettingsProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState(loadSettings);
  useEffect(() => {
    try { localStorage.setItem(KEY, JSON.stringify(settings)); } catch { /* Storage may be full. */ }
  }, [settings]);
  return <SettingsContext.Provider value={{ settings, updateSettings: (patch) => {
    setSettings((current) => ({ ...current, ...patch }));
  } }}>{children}</SettingsContext.Provider>;
}
export function useSettings() {
  const context = useContext(SettingsContext);
  if (!context) throw new Error('全局设置尚未初始化');
  return context;
}
