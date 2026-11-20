//! 音频文件流式解码（symphonia），到结尾自动从头循环。输出交错立体声 f32。

use std::fs::File;
use std::path::{Path, PathBuf};
use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("不支持的音频格式")]
    Unsupported,
    #[error("无法读取文件：{0}")]
    Io(String),
    #[error("解码失败：{0}")]
    Decode(String),
}

pub struct FileDecoder {
    path: PathBuf,
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    sample_rate: u32,
    scratch: Vec<f32>,
}

impl FileDecoder {
    /// # Errors
    /// 文件打不开、格式不支持或没有音轨。
    pub fn open(path: &Path) -> Result<Self, DecodeError> {
        let file = File::open(path).map_err(|e| DecodeError::Io(e.to_string()))?;
        let stream = MediaSourceStream::new(Box::new(file), MediaSourceStreamOptions::default());
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }
        let format = symphonia::default::get_probe()
            .probe(
                &hint,
                stream,
                FormatOptions::default(),
                MetadataOptions::default(),
            )
            .map_err(map_open_error)?;
        let track = format
            .default_track(TrackType::Audio)
            .ok_or(DecodeError::Unsupported)?;
        let params = track
            .codec_params
            .as_ref()
            .and_then(|p| p.audio())
            .ok_or(DecodeError::Unsupported)?;
        let sample_rate = params.sample_rate.ok_or(DecodeError::Unsupported)?;
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(params, &AudioDecoderOptions::default())
            .map_err(map_open_error)?;
        let track_id = track.id;
        Ok(Self {
            path: path.to_path_buf(),
            format,
            decoder,
            track_id,
            sample_rate,
            scratch: Vec::new(),
        })
    }

    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// 解码下一段并追加到 `out`（交错立体声）。到文件尾时重新打开文件循环播放。
    ///
    /// # Errors
    /// 文件在播放中变得不可读，或整个文件解不出任何声音。
    pub fn next_stereo(&mut self, out: &mut Vec<f32>) -> Result<(), DecodeError> {
        let mut empty_loops = 0;
        loop {
            let packet = match self.format.next_packet() {
                Ok(Some(packet)) => packet,
                Ok(None) => {
                    empty_loops += 1;
                    if empty_loops > 1 {
                        return Err(DecodeError::Decode("文件中没有可播放的声音".into()));
                    }
                    *self = Self::open(&self.path)?;
                    continue;
                }
                Err(SymError::ResetRequired) => {
                    self.decoder.reset();
                    continue;
                }
                Err(e) => return Err(DecodeError::Io(e.to_string())),
            };
            if packet.track_id != self.track_id {
                continue;
            }
            match self.decoder.decode(&packet) {
                Ok(buffer) => {
                    let channels = buffer.spec().channels().count().max(1);
                    self.scratch.clear();
                    buffer.copy_to_vec_interleaved(&mut self.scratch);
                    to_stereo(&self.scratch, channels, out);
                    return Ok(());
                }
                // 个别损坏的帧直接跳过。
                Err(SymError::DecodeError(_)) => {}
                Err(e) => return Err(DecodeError::Decode(e.to_string())),
            }
        }
    }
}

fn map_open_error(e: SymError) -> DecodeError {
    match e {
        SymError::Unsupported(_) => DecodeError::Unsupported,
        SymError::IoError(e) => DecodeError::Io(e.to_string()),
        other => DecodeError::Decode(other.to_string()),
    }
}

/// 单声道复制到两个声道；多声道取前两个。
fn to_stereo(input: &[f32], channels: usize, out: &mut Vec<f32>) {
    match channels {
        1 => out.extend(input.iter().flat_map(|&s| [s, s])),
        2 => out.extend_from_slice(input),
        n => out.extend(input.chunks_exact(n).flat_map(|f| [f[0], f[1]])),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wav(path: &Path, rate: u32, samples: &[i16]) {
        let data_len = u32::try_from(samples.len() * 2).unwrap();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&rate.to_le_bytes());
        bytes.extend_from_slice(&(rate * 2).to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&data_len.to_le_bytes());
        bytes.extend(samples.iter().flat_map(|s| s.to_le_bytes()));
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn decodes_mono_wav_to_stereo_and_loops() {
        let path = std::env::temp_dir().join(format!("yume-decode-{}.wav", std::process::id()));
        wav(&path, 22_050, &[0, 16_384, -16_384, 0]);
        let mut d = FileDecoder::open(&path).unwrap();
        assert_eq!(d.sample_rate(), 22_050);
        let mut out = Vec::new();
        for _ in 0..3 {
            d.next_stereo(&mut out).unwrap();
        }
        assert!(out.len() >= 8 * 2, "循环后应持续有输出");
        assert!((out[2] - 0.5).abs() < 1e-3 && (out[3] - 0.5).abs() < 1e-3);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn rejects_non_audio() {
        let path = std::env::temp_dir().join(format!("yume-decode-{}.mp3", std::process::id()));
        std::fs::write(&path, b"definitely not audio").unwrap();
        assert!(FileDecoder::open(&path).is_err());
        std::fs::remove_file(path).unwrap();
    }
}
