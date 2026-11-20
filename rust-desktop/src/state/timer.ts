export type TimerMode = 'countdown' | 'stopwatch';

export function timerValue(mode: TimerMode, anchor: number, now: number): number {
  return Math.max(0, mode === 'countdown' ? anchor - now : now - anchor);
}

// Prepare the context in the initiating user gesture, so completion can ring
// later even when the floating console has been hidden.
export function createCompletionSound() {
  let context: AudioContext | null = null;
  return {
    prepare() {
      context ??= new AudioContext();
      void context.resume().catch(() => {});
    },
    async play() {
      if (!context) return;
      await context.resume();
      const start = context.currentTime;
      [660, 880, 1046].forEach((frequency, index) => {
        const osc = context!.createOscillator();
        const gain = context!.createGain();
        const at = start + index * 0.22;
        osc.frequency.value = frequency;
        gain.gain.setValueAtTime(0, at);
        gain.gain.linearRampToValueAtTime(0.18, at + 0.02);
        gain.gain.exponentialRampToValueAtTime(0.001, at + 0.4);
        osc.connect(gain).connect(context!.destination);
        osc.start(at);
        osc.stop(at + 0.42);
      });
    },
    dispose() { void context?.close(); context = null; },
  };
}
