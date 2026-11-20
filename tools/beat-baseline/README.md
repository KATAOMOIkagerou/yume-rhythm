# beat-baseline：节拍引擎基准轨迹

在 Node 里原样运行冻结的 JS 基准引擎 `frozen-audio-core.js`（sha256 `e5b7372e…2011c`，**不要修改**），
生成逐帧轨迹，供 `crates/beat/tests/` 的回归测试对比 Rust 实现（`yume-beat`）。

没有 npm 依赖，Node 18+ 即可。

## 重新生成基准

```sh
node tools/beat-baseline/generate.mjs                    # 全部合成信号 + nod_cases → crates/beat/tests/fixtures/
node tools/beat-baseline/generate.mjs --only silence_gap # 只生成一条
cargo test -p yume-beat                                  # 然后跑回归
```

生成是确定性的，重复运行得到逐字节相同的文件。基准引擎是冻结的，**只有改了本目录的
信号或驱动方式才需要重新生成**；改了之后，Rust 侧对应的转写（见下表）必须同步修改。

## 驱动约定（与 Rust 测试完全相同）

- 基准宿主 `E1` 的分析器设置：fftSize 2048、smoothingTimeConstant 0.08、-90..-3 dB。
- 固定 60 Hz 时钟：`now_k = k·1000/60` 毫秒，`k = 0, 1, …, floor(采样数·60/采样率) - 1`；
  `skip` 区间 `[start, end)` 内的帧不处理（模拟 rAF 停顿）。
- 每帧的分析窗口是以采样 `floor(now_k·sr/1000)` 结尾（不含）的 2048 个采样，越界补零。
- 每帧调用一次 `x1.process(now_k)`；它内部调用假 AnalyserNode 的 `getByteFrequencyData`。
- 假 AnalyserNode（`lib/analyser.mjs`）按 Web Audio 规范实现，与 `yume_beat::Analyser`
  逐行相同。x1 只用到它的 `frequencyBinCount`、`context.sampleRate`、`getByteFrequencyData`。

## 文件与 Rust 侧的对应

| 本目录 | 作用 | Rust 侧（必须同步） |
|---|---|---|
| `frozen-audio-core.js` | 基准引擎（只读） | `crates/beat/src/` |
| `lib/dsp.mjs` | 确定性数学：`dexp`、`sinCycles`、xorshift32、FNV-1a、单位根表、FFT | `src/fft.rs`、`tests/common/synth.rs`、`tests/common/fixture.rs` |
| `lib/analyser.mjs` | 假 AnalyserNode | `src/analyser.rs` |
| `lib/signals.mjs` | 合成信号目录与合成器 | `tests/common/synth.rs` |
| `lib/wav.mjs` | WAV 读取与下混 | `tests/common/wav.rs` |
| `lib/trace.mjs` | 60 Hz 驱动与记录 | `tests/common/mod.rs`（`run_end_to_end`） |

合成器、单位根和 FFT 只用 IEEE 基本运算（`+ - * / sqrt floor`），不调 `Math.sin/cos/exp`，
所以 Node 与 Rust 在任何平台上都逐位相同（每条轨迹记录了采样哈希 `samplesHash`，Rust
测试会先核对它）。分析器里唯一的 libm 调用是 `log10`。

## 合成信号

48 kHz 单声道，24–26 秒，16 步鼓组（底鼓 = 衰减正弦扫频，军鼓 / 镲 = xorshift 噪声猝发，
低音 = 低通锯齿波）。

| 名称 | 内容 | 基准结果 |
|---|---|---|
| `bpm094_hiphop` | 94 BPM 嘻哈 | 锁 95 |
| `bpm120_four_floor` | 120 BPM 四四拍 | 锁 120 |
| `bpm140_rock` | 140 BPM 摇滚 | 锁 140 |
| `bpm175_dnb` | 175 BPM 鼓打贝斯 | 先锁 176，3 秒后改锁半速 88 |
| `tempo_change_100_128` | 100 → 128 BPM（12 秒处） | 100，约 16 秒改锁半速 64 |
| `silence_gap` | 120 BPM，1.5 秒静音（解锁），再 108 BPM | 120 → 解锁 → 109；**额外录制了每帧频谱字节** |
| `quiet_demo112` | 基准演示鼓型 112 BPM，增益 0.004（约 -48 dB） | 锁 111 |
| `stall_128` | 128 BPM，12 秒处时钟停顿 350 ms（触发 >200 ms 间隔分支） | 64 → 停顿解锁 → 64 → 128 |

`nod_cases.json`：基准 `m1`（点头曲线）的 2019 组输入输出（含边界），以及空闲帧 `jh`。

## 轨迹格式（`format: 1`）

每个文件一个 JSON 对象，`columns` 按列存储（每帧一个元素）：

- `beat` / `onset` / `locked`：`"0"`/`"1"` 字符串；
- `strength`、`level`、`bass`、`bpm`、`beats`：数值；`nextBeatAt`、`periodMs`：数值或 `null`；
- `nod`：`m1(now + 8, nextBeatAt, periodMs, 1.2)`，数值或 `null`；
- `byteHash`：每帧 8 位十六进制，基准引擎拿到的 1024 个频谱字节的 FNV-1a；
- `bytes`（仅 `silence_gap`）：每帧前 `bins` 个频谱字节（覆盖检测器读取的全部频点），base64。

数值是 JS 的最短往返表示，Rust 侧用 serde_json 的 `float_roundtrip` 逐位解析。

## 真实歌曲对比

歌曲和它的轨迹**永远不要提交**（本目录的 `.gitignore` 已忽略 `*.wav`、`*.trace.json`；
放在仓库外更好）。

```sh
# 1. 用基准引擎生成轨迹（默认写到 <wav>.trace.json，可用 --out 指定）
node tools/beat-baseline/generate.mjs --wav /path/to/song.wav
# 2. 用 Rust 实现跑同一首歌并对比（release 更快）
YUME_BEAT_AUDIO=/path/to/song.wav cargo test -p yume-beat --release --test parity -- --ignored --nocapture
#    轨迹不在默认位置时再加 YUME_BEAT_TRACE=/path/to/trace.json
```

支持 16/24/32 位 PCM、32/64 位浮点、WAVE_FORMAT_EXTENSIBLE；多声道取平均下混为单声道，
采样率按文件原样使用。其他格式先用 ffmpeg 转：`ffmpeg -i song.flac -c:a pcm_s16le song.wav`。
