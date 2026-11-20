import { E1 as OriginalEngine, jh } from './frozen-audio-core.js';

// Reuse the original detector, FFT/smoothing and beat semantics. Rust supplies PCM only.
// No display-capture permission, desktop video track, or recorded audio file is involved.
interface EngineInternals {
  ensureGraph(): AudioContext;
  analyser: AnalyserNode;
  teardownSource(): void;
  activate(): void;
  running: boolean;
  listeners: { ended?: () => void };
}

export class E1 extends OriginalEngine {
  private nativeNode: AudioWorkletNode | null = null;
  private nativeGeneration: number | null = null;
  private workletContext: AudioContext | null = null;
  private workletReady: Promise<void> | null = null;
  private attempt = 0;
  private pcmListener: ((event: Event) => void) | null = null;

  override async startSystemAudio() {
    const bridge = window.desktopPetBridge;
    if (!bridge?.isRust || !bridge.startLoopback) return super.startSystemAudio();
    this.stop();
    const attempt = this.attempt;
    const checkCurrent = () => {
      if (attempt !== this.attempt) throw new DOMException('音频监听已取消', 'AbortError');
    };
    const engine = this as unknown as EngineInternals;
    const ctx = engine.ensureGraph();
    try {
      await ctx.resume();
      checkCurrent();
      if (this.workletContext !== ctx) {
        this.workletContext = ctx;
        this.workletReady = ctx.audioWorklet.addModule(new URL('/native-pcm-worklet.js', document.baseURI))
          .catch(error => { this.workletContext = null; throw error; });
      }
      await this.workletReady;
      checkCurrent();
      const node = new AudioWorkletNode(ctx, 'pet-pcm', { numberOfOutputs:1, outputChannelCount:[1] });
      node.connect(engine.analyser);
      this.nativeNode = node;
      this.pcmListener = event => {
        const detail = (event as CustomEvent<{ generation:number; pcm?:string; error?:string }>).detail;
        if (detail.generation !== this.nativeGeneration) return;
        if (detail.error) { this.stop(); engine.listeners.ended?.(); return; }
        if (detail.pcm) {
          const bytes = Uint8Array.from(atob(detail.pcm), c => c.charCodeAt(0));
          node.port.postMessage(bytes.buffer, [bytes.buffer]);
        }
      };
      window.addEventListener('native-pcm', this.pcmListener);
      const info = await bridge.startLoopback();
      checkCurrent();
      this.nativeGeneration = info.generation;
      node.port.postMessage({ sourceRate: info.sampleRate, outputRate: ctx.sampleRate });
      engine.activate();
    } catch (error) { if (attempt === this.attempt) this.stop(); throw error; }
  }

  override stop() {
    this.attempt++;
    if (this.nativeNode || this.nativeGeneration !== null) {
      this.nativeGeneration = null;
      if (this.pcmListener) window.removeEventListener('native-pcm', this.pcmListener);
      this.pcmListener = null;
      this.nativeNode?.port.postMessage({ stop:true });
      this.nativeNode?.disconnect(); this.nativeNode?.port.close(); this.nativeNode = null;
      void window.desktopPetBridge?.stopLoopback?.();
    }
    super.stop();
  }

  override startFile(file: File) { this.stop(); return super.startFile(file); }
  override startMic() { this.stop(); return super.startMic(); }
  override startDemo() { this.stop(); super.startDemo(); }
}
export { jh };
