import type React from 'react';

export type AudioSourceMode = 'idle' | 'system' | 'mic' | 'file' | 'demo';

export interface MotionFrame {
  beat: boolean;
  onset: boolean;
  strength: number;
  level: number;
  bass: number;
  bpm: number;
  beats: number;
  locked: boolean;
  nextBeatAt?: number;
  periodMs?: number;
}

export type ActivityMode = 'chill' | 'study';

export type EmotionType = 'neutral' | 'happy' | 'sad';

export interface ExpressionDefinition {
  id: string;
  name: string;
  type: 'base' | 'accessory';
  resources: { animation?: string; eyes?: string; mouth?: string; overlay?: string };
  durationMs: number;
  random: boolean;
  event?: RandomEventType;
}

export interface GlobalSettings {
  endSoundEnabled: boolean;
  randomEnabled: boolean;
  randomIntervalSeconds: number;
}

export type RandomEventType = 'doze' | 'think' | 'shock' | 'love';

export type EventContextKey = 'chill' | 'study' | 'music';

export type EventSwitchMatrix = Record<EventContextKey, Record<RandomEventType, boolean>>;

export const DEFAULT_EVENT_SWITCHES: EventSwitchMatrix = {
  chill: { doze: true, think: true, shock: true, love: true },
  study: { doze: false, think: true, shock: true, love: true },
  music: { doze: false, think: true, shock: true, love: true },
};

export interface CharacterManifest {
  packName: string;
  version: string;
  canvasWidth: number;
  canvasHeight: number;
  headPivot: { x: number; y: number };
  nodAmplitudeScale: number;
  nodTiltScale: number;
  showSceneBg: boolean;
  showMusicHeadphones: boolean;
  layers: {
    sceneBg: string;
    chairBack: string;
    bodyBase: string;
    hairBack: string;
    headBase: string;
    blushBase: string;
    hairFront: string;
    headphones: string;
    deskSurface: string;
    armsChill: string;
    armsStudy: string;
    computerOff: string;
    computerOn: string;
    books: string;
    timerClock: string;
  };
  expressions: Record<EmotionType, { eyes: string; mouth: string }>;
  blinkEyes: string;
  randomEvents: Record<RandomEventType, { eyes: string; mouth: string; fx?: string }>;
}

export interface DeskCharacterSceneProps {
  motionRef: React.RefObject<MotionFrame>;
  musicRunning: boolean;
  autoFollow: boolean;
  activityMode: ActivityMode;
  timerActive: boolean;
  emotion: EmotionType;
  eventSwitches: EventSwitchMatrix;
  assetReloadToken: number;
  forcedEventPreview?: RandomEventType | null;
  expression?: ExpressionDefinition | null;
  baseExpression?: ExpressionDefinition | null;
  onExpressionsLoaded?: (expressions: ExpressionDefinition[]) => void;
  onAssetStatus?: (status: {
    loaded: boolean;
    packName?: string;
    assetDirHint?: string;
    error?: string;
  }) => void;
}

export interface DesktopPetBridge {
  isElectron: boolean;
  assetProtocolBase: string;
  externalAssetDir: string;
  setLocked: (value: boolean) => Promise<boolean>;
  getLocked: () => Promise<boolean>;
  onLocked: (callback: (value: boolean) => void) => () => void;
  showConsole: () => void;
  hideConsole: () => void;
  quit: () => void;
  openSoundSettings: () => void;
}

declare global {
  interface Window {
    desktopPetBridge?: DesktopPetBridge;
  }
}
