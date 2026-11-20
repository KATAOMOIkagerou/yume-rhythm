//! L0 频谱分析与节拍检测，与 JS 基准引擎 `tools/beat-baseline/frozen-audio-core.js` 逐帧一致。
//! 注释里的"基准 `x1`"等指该文件中的标识符，便于对照。
//!
//! - [`Analyser`]：Web Audio `AnalyserNode.getByteFrequencyData` 的等价实现。
//! - [`BeatDetector`]：基准 `x1`，每帧输入频谱字节，输出 [`MotionFrame`]。
//! - [`locked_beat_nod`]：基准 `m1`，锁拍时的点头曲线。
//! - [`TempoShift`]：手动纠正半速 / 倍速锁拍（基准引擎没有的功能，默认不生效），见
//!   [`BeatDetector::shift_tempo`]。
//!
//! 典型用法（节拍线程，固定 60 Hz）：
//!
//! ```
//! use yume_beat::{Analyser, BeatDetector};
//!
//! let mut analyser = Analyser::default();
//! let mut detector = BeatDetector::new(48_000.0, analyser.frequency_bin_count());
//! let mut bytes = vec![0_u8; analyser.frequency_bin_count()];
//! let recent_samples = vec![0.0_f32; analyser.fft_size()];
//! for k in 0..3 {
//!     let now_ms = k as f64 * 1000.0 / 60.0;
//!     analyser.get_byte_frequency_data(&recent_samples, &mut bytes);
//!     let frame = detector.process(now_ms, &bytes);
//!     assert!(!frame.locked);
//! }
//! ```
//!
//! 与基准引擎的回归对比见 `tests/` 和 `tools/beat-baseline/README.md`。
#![forbid(unsafe_code)]

mod analyser;
mod detector;
mod fft;
mod frame;
mod jsmath;
mod nod;
mod octave;

pub use analyser::{Analyser, AnalyserConfig, AnalyserError};
pub use detector::BeatDetector;
pub use frame::MotionFrame;
pub use nod::locked_beat_nod;
pub use octave::{TempoScale, TempoShift};
