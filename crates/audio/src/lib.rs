//! L1 音频适配：WASAPI 回环采集、文件解码与循环播放、提示音。只输出采样，不做节拍分析。
//!
//! - `SampleRing`：音频线程写、节拍线程读的单声道采样缓冲。
//! - `AudioService`（仅 Windows）：持有 cpal 流的服务线程。

pub mod chime;
pub mod decode;
pub mod queue;
pub mod resample;
mod ring;
#[cfg(windows)]
mod service;

pub use ring::SampleRing;
#[cfg(windows)]
pub use service::{AudioEvent, AudioService, ErrorKind, OutputDevice, Source};
