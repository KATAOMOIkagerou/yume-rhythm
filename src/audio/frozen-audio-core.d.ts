export interface AudioMotionFrame {
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

export interface AudioEngineListeners {
  frame?: (frame: AudioMotionFrame) => void;
  ended?: () => void;
}

export declare function m1(
  now: number,
  nextBeatAt: number | undefined,
  periodMs: number | undefined,
  strength: number
): number | null;

export declare class x1 {
  analyser: AnalyserNode;
  freq: Uint8Array;
  sensitivity: number;
  locked: boolean;
  lockedLagMs: number;
  beatCount: number;
  constructor(analyser: AnalyserNode);
  reset(): void;
  process(now: number): AudioMotionFrame;
}

export declare class A1 {
  ctx: AudioContext;
  readonly out: GainNode;
  bpm: number;
  constructor(ctx: AudioContext);
  start(): void;
  stop(): void;
  dispose(): void;
}

export declare class E1 {
  readonly isRunning: boolean;
  readonly contextState: AudioContextState | 'closed';
  on(listeners: AudioEngineListeners): void;
  setSensitivity(value: number): void;
  stop(): void;
  dispose(): void;
  startSystemAudio(): Promise<void>;
  startMic(): Promise<void>;
  startFile(file: File): Promise<void>;
  startDemo(): void;
}

export declare const jh: AudioMotionFrame;
