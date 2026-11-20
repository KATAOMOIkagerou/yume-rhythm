class PetPcm extends AudioWorkletProcessor {
  constructor() {
    super();
    this.ring = new Float32Array(16384); this.read = 0; this.write = 0;
    this.phase = 0; this.ratio = 48000 / sampleRate; this.last = 0;
    this.alive = true;
    this.port.onmessage = ({data}) => {
      if (data.stop) { this.alive = false; return; }
      if (data.sourceRate) { this.ratio = data.sourceRate / data.outputRate; return; }
      const pcm = new Float32Array(data);
      for (const value of pcm) {
        if (this.write - this.read >= this.ring.length) this.read = this.write - 4096;
        this.ring[this.write++ & 16383] = value;
      }
    };
  }
  process(_inputs, outputs) {
    if (!this.alive) return false;
    const out = outputs[0][0];
    for (let i=0; i<out.length; i++) {
      if (this.write - this.read > 1) {
        const at = this.read & 16383;
        out[i] = this.ring[at] * (1-this.phase) + this.ring[(at+1)&16383]*this.phase;
        this.phase += this.ratio;
        const advance = Math.floor(this.phase);
        this.read += advance; this.phase -= advance;
      } else out[i] = 0;
    }
    return true;
  }
}
registerProcessor('pet-pcm', PetPcm);
