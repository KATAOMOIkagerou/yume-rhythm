//! 节拍检测器每帧的输出。

/// 一帧节拍与音量信息（基准 `AudioMotionFrame`）。
///
/// 时间都在调用方传入的单调时钟（毫秒）上。`Default` 即基准的空闲帧 `jh`。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MotionFrame {
    /// 本帧触发了一次锁拍节拍（点头）。
    pub beat: bool,
    /// 本帧检测到起音（不要求锁拍）。
    pub onset: bool,
    /// 节拍力度，`beat` 为真时在 0.5..=1.7，否则为 0。
    pub strength: f64,
    /// 整体音量 0..=1。
    pub level: f64,
    /// 低频能量 0..=1。
    pub bass: f64,
    /// 显示用 BPM（整数值，中位数去抖后），未锁拍时为 0。
    pub bpm: f64,
    /// 自上次 `reset` 以来触发的节拍总数。
    pub beats: u32,
    /// 是否锁定节拍。
    pub locked: bool,
    /// 下一拍的预测时间；仅锁拍时有值（基准 `nextBeatAt`）。
    pub next_beat_at: Option<f64>,
    /// 当前锁定的节拍周期（毫秒）；仅锁拍时有值（基准 `periodMs`）。
    pub period_ms: Option<f64>,
}

impl MotionFrame {
    /// 空闲帧，与 `MotionFrame::default()` 相同。基准：`jh`。
    pub const IDLE: Self = Self {
        beat: false,
        onset: false,
        strength: 0.0,
        level: 0.0,
        bass: 0.0,
        bpm: 0.0,
        beats: 0,
        locked: false,
        next_beat_at: None,
        period_ms: None,
    };
}
