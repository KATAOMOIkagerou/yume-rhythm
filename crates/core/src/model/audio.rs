//! 音频同步的业务状态（功能清单第 9 节）。采样与节拍分析不在这里，见 docs/架构.md 第 4 节。

use crate::clock::WallMs;

/// 拖入文件时接受的扩展名。
pub const AUDIO_EXTENSIONS: &[&str] = &["mp3", "wav", "ogg", "flac", "m4a", "aac"];
/// 开始监听后超过这么久仍然几乎无声，提示用户检查。
pub const SILENCE_HINT_AFTER_MS: i64 = 3_500;
pub const SILENCE_LEVEL: f32 = 0.012;

/// 音源。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AudioSource {
    /// 回环采集系统输出（设备取自设置）。
    System,
    /// 循环播放本地音频文件。
    File { path: String },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AudioStatus {
    #[default]
    Idle,
    Starting,
    Running,
}

/// 音频错误的种类。中文文案由界面层翻译。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioError {
    /// 找不到输出设备，或设备被拔出。
    DeviceUnavailable,
    /// 文件格式不支持。
    UnsupportedFile,
    /// 文件无法读取或解码失败。
    FileUnreadable,
    /// 其他系统错误。
    Other,
}

/// 节拍线程每 120 ms 汇报一次的遥测数据，仅用于显示。
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Telemetry {
    pub bpm: f32,
    pub level: f32,
    pub bass: f32,
    pub beats: u32,
    /// 是否已锁定节拍周期（注意：不是桌宠的锁定）。
    pub beat_locked: bool,
}

/// 手动纠正倍速误锁。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TempoShift {
    Double,
    Half,
}

/// 输出设备。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioState {
    pub status: AudioStatus,
    /// 当前（或正在启动的）音源。
    pub source: Option<AudioSource>,
    /// 上一次使用的音源。空格键重放它，默认系统声音。
    pub last_source: Option<AudioSource>,
    pub error: Option<AudioError>,
    pub telemetry: Telemetry,
    pub started_at: Option<WallMs>,
    /// 每次启动递增。适配层回报时带上它，过期的回报会被忽略。
    pub request: u64,
    pub devices: Vec<AudioDevice>,
}

impl AudioState {
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.status == AudioStatus::Running
    }

    /// 有音源：正在启动或已在运行。节拍分析、耳机、"停止听歌"都以它为准。
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.status != AudioStatus::Idle
    }

    #[must_use]
    pub fn is_busy(&self) -> bool {
        self.status == AudioStatus::Starting
    }

    /// 已运行一段时间却几乎无声。
    #[must_use]
    pub fn silence_hint(&self, now: WallMs) -> bool {
        self.is_running()
            && self.error.is_none()
            && self
                .started_at
                .is_some_and(|t| now.since(t) > SILENCE_HINT_AFTER_MS)
            && self.telemetry.level < SILENCE_LEVEL
    }

    /// 空格键要重放的音源。
    #[must_use]
    pub fn replay_source(&self) -> AudioSource {
        self.last_source.clone().unwrap_or(AudioSource::System)
    }
}

/// 从拖入的文件中挑出第一个音频文件。
#[must_use]
pub fn first_audio_file(paths: &[String]) -> Option<&String> {
    paths.iter().find(|p| is_audio_file(p))
}

#[must_use]
pub fn is_audio_file(path: &str) -> bool {
    path.rsplit_once('.')
        .is_some_and(|(_, ext)| AUDIO_EXTENSIONS.iter().any(|e| ext.eq_ignore_ascii_case(e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_audio_files_case_insensitively() {
        assert!(is_audio_file(r"C:\歌\a.MP3"));
        assert!(!is_audio_file("cover.png"));
        assert!(!is_audio_file("noext"));
        let paths = vec!["a.txt".to_string(), "b.flac".to_string()];
        assert_eq!(first_audio_file(&paths).map(String::as_str), Some("b.flac"));
    }

    #[test]
    fn silence_hint_needs_time_and_quiet() {
        let mut a = AudioState {
            status: AudioStatus::Running,
            started_at: Some(WallMs(0)),
            ..AudioState::default()
        };
        assert!(!a.silence_hint(WallMs(3_000)));
        assert!(a.silence_hint(WallMs(4_000)));
        a.telemetry.level = 0.5;
        assert!(!a.silence_hint(WallMs(4_000)));
    }
}
