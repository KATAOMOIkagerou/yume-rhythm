//! Pure-Rust local audio file decoder & looping player (`symphonia` + WASAPI render).
//!
//! Decodes MP3/WAV/FLAC/OGG/Vorbis/AAC/M4A files, resamples cleanly to 48 kHz stereo
//! for WASAPI playback, and feeds downmixed mono PCM into the lock-free beat-detector ring.

use crate::wasapi_io::{ComGuard, ProducerTask, ENGINE_SAMPLE_RATE};
use ringbuf::traits::Producer;
use ringbuf::HeapProd;
use std::collections::VecDeque;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;
use wasapi::{Direction, SampleType, StreamMode, WaveFormat};

struct DecodedStream {
    path: PathBuf,
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    src_rate: u32,
    channels: usize,
    /// Interleaved stereo `[L, R, L, R, ...]` at `ENGINE_SAMPLE_RATE`.
    out_stereo: VecDeque<f32>,
    /// Linear resampler fractional cursor & previous frame.
    frac_pos: f64,
    prev_l: f32,
    prev_r: f32,
    has_prev: bool,
}

impl DecodedStream {
    fn open(path: &Path) -> Result<Self, String> {
        let (format, decoder, track_id, src_rate, channels) = Self::open_reader(path)?;
        Ok(Self {
            path: path.to_path_buf(),
            format,
            decoder,
            track_id,
            src_rate,
            channels,
            out_stereo: VecDeque::with_capacity(16_384),
            frac_pos: 0.0,
            prev_l: 0.0,
            prev_r: 0.0,
            has_prev: false,
        })
    }

    fn open_reader(
        path: &Path,
    ) -> Result<(Box<dyn FormatReader>, Box<dyn Decoder>, u32, u32, usize), String> {
        let file = File::open(path).map_err(|e| format!("无法打开音频文件: {e}"))?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        let probed = symphonia::default::get_probe()
            .format(
                &hint,
                mss,
                &FormatOptions::default(),
                &MetadataOptions::default(),
            )
            .map_err(|e| format!("不支持或无法解析该音频格式: {e}"))?;

        let format = probed.format;
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or_else(|| "文件中没有可播放的音频轨道".to_string())?;

        let track_id = track.id;
        let src_rate = track.codec_params.sample_rate.unwrap_or(ENGINE_SAMPLE_RATE);
        let channels = track
            .codec_params
            .channels
            .map(|c| c.count())
            .unwrap_or(2)
            .max(1);

        let decoder = symphonia::default::get_codecs()
            .make(&track.codec_params, &DecoderOptions::default())
            .map_err(|e| format!("初始化音频解码器失败: {e}"))?;

        Ok((format, decoder, track_id, src_rate, channels))
    }

    fn rewind(&mut self) -> Result<(), String> {
        let seek_res = self.format.seek(
            SeekMode::Coarse,
            SeekTo::Time {
                time: Time::new(0, 0.0),
                track_id: Some(self.track_id),
            },
        );
        if seek_res.is_ok() {
            self.decoder.reset();
            self.frac_pos = 0.0;
            self.has_prev = false;
            return Ok(());
        }
        // Fallback: reopen stream cleanly for formats without seek tables.
        let (format, decoder, track_id, src_rate, channels) = Self::open_reader(&self.path)?;
        self.format = format;
        self.decoder = decoder;
        self.track_id = track_id;
        self.src_rate = src_rate;
        self.channels = channels;
        self.frac_pos = 0.0;
        self.has_prev = false;
        Ok(())
    }

    fn push_resampled_packet(&mut self, samples: &[f32], channels: usize, src_rate: u32) {
        let ch = channels.max(1);
        let frames = samples.len() / ch;
        if frames == 0 {
            return;
        }

        if src_rate == ENGINE_SAMPLE_RATE {
            for f in 0..frames {
                let base = f * ch;
                let (l, r) = if ch == 1 {
                    (samples[base], samples[base])
                } else {
                    (samples[base], samples[base + 1])
                };
                self.out_stereo.push_back(l);
                self.out_stereo.push_back(r);
            }
            return;
        }

        let step = (src_rate as f64) / (ENGINE_SAMPLE_RATE as f64);
        let frame_lr = |idx: usize| -> (f32, f32) {
            let base = idx * ch;
            if ch == 1 {
                (samples[base], samples[base])
            } else {
                (samples[base], samples[base + 1])
            }
        };

        let mut pos = self.frac_pos;
        if !self.has_prev {
            let (l0, r0) = frame_lr(0);
            self.prev_l = l0;
            self.prev_r = r0;
            self.has_prev = true;
        }

        let frames_f = frames as f64;
        while pos < frames_f {
            let idx = pos.floor() as isize;
            let frac = (pos - idx as f64) as f32;
            let (l0, r0) = if idx < 0 {
                (self.prev_l, self.prev_r)
            } else {
                frame_lr(idx as usize)
            };
            let next_idx = idx + 1;
            let (l1, r1) = if next_idx < 0 {
                (self.prev_l, self.prev_r)
            } else if (next_idx as usize) < frames {
                frame_lr(next_idx as usize)
            } else {
                frame_lr(frames - 1)
            };
            let l = l0 + (l1 - l0) * frac;
            let r = r0 + (r1 - r0) * frac;
            self.out_stereo.push_back(l);
            self.out_stereo.push_back(r);
            pos += step;
        }

        let (last_l, last_r) = frame_lr(frames - 1);
        self.prev_l = last_l;
        self.prev_r = last_r;
        self.frac_pos = pos - frames_f;
    }

    /// Fills `stereo_out` (length = `frames * 2`) with decoded 48 kHz stereo PCM, looping on EOF.
    fn fill_stereo(&mut self, stereo_out: &mut [f32]) -> Result<(), String> {
        let mut eof_retries = 0usize;
        while self.out_stereo.len() < stereo_out.len() {
            match self.format.next_packet() {
                Ok(packet) => {
                    if packet.track_id() != self.track_id {
                        continue;
                    }
                    match self.decoder.decode(&packet) {
                        Ok(decoded) => {
                            let spec = *decoded.spec();
                            let ch = spec.channels.count().max(1);
                            let rate = spec.rate.max(8_000);
                            let mut sbuf = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
                            sbuf.copy_interleaved_ref(decoded);
                            self.push_resampled_packet(sbuf.samples(), ch, rate);
                            eof_retries = 0;
                        }
                        Err(SymphoniaError::DecodeError(_)) => continue,
                        Err(SymphoniaError::IoError(err))
                            if err.kind() == std::io::ErrorKind::UnexpectedEof =>
                        {
                            eof_retries += 1;
                            if eof_retries > 2 {
                                return Err("音频文件为空或无法继续解码".to_string());
                            }
                            self.rewind()?;
                        }
                        Err(e) => return Err(format!("音频解码失败: {e}")),
                    }
                }
                Err(SymphoniaError::IoError(err))
                    if err.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    eof_retries += 1;
                    if eof_retries > 2 {
                        return Err("音频文件为空或不含有效音频帧".to_string());
                    }
                    self.rewind()?;
                }
                Err(SymphoniaError::ResetRequired) => {
                    self.decoder.reset();
                }
                Err(e) => return Err(format!("读取音频包失败: {e}")),
            }
        }

        for sample in stereo_out.iter_mut() {
            *sample = self.out_stereo.pop_front().unwrap_or(0.0);
        }
        Ok(())
    }
}

/// Starts looping playback of a local audio file via WASAPI shared-mode render while
/// simultaneously feeding downmixed mono 48 kHz PCM into `prod`.
pub fn start_file_player(
    path: PathBuf,
    mut prod: HeapProd<f32>,
    on_error: Arc<dyn Fn(String) + Send + Sync>,
) -> Result<ProducerTask, String> {
    // Probe and validate the file synchronously so invalid files report errors immediately.
    let mut stream = DecodedStream::open(&path)?;

    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);

    let thread = thread::Builder::new()
        .name("pet-file-player".into())
        .spawn(move || {
            let run = (|| -> Result<(), String> {
                let _com = ComGuard::init_mta()?;
                let device = wasapi::get_default_device(&Direction::Render)
                    .map_err(|e| format!("无法获取默认播放设备: {e}"))?;
                let mut client = device
                    .get_iaudioclient()
                    .map_err(|e| format!("创建播放客户端失败: {e}"))?;
                let format =
                    WaveFormat::new(32, 32, &SampleType::Float, ENGINE_SAMPLE_RATE as usize, 2, None);
                let (default_period, _) = client
                    .get_device_period()
                    .map_err(|e| format!("获取播放设备周期失败: {e}"))?;
                let buf_hns = (default_period * 2).max(200_000);
                client
                    .initialize_client(
                        &format,
                        &Direction::Render,
                        &StreamMode::EventsShared {
                            autoconvert: true,
                            buffer_duration_hns: buf_hns,
                        },
                    )
                    .map_err(|e| format!("初始化音频播放失败: {e}"))?;

                let event = client
                    .set_get_eventhandle()
                    .map_err(|e| format!("创建播放事件句柄失败: {e}"))?;
                let render = client
                    .get_audiorenderclient()
                    .map_err(|e| format!("获取渲染客户端失败: {e}"))?;
                client
                    .start_stream()
                    .map_err(|e| format!("启动音频播放失败: {e}"))?;

                let _ = ready_tx.send(Ok(()));

                let mut stereo_buf = Vec::with_capacity(4096 * 2);
                let mut stereo_bytes = Vec::with_capacity(4096 * 8);
                let mut mono_buf = Vec::with_capacity(4096);

                while !worker_stop.load(Ordering::Acquire) {
                    let avail = client
                        .get_available_space_in_frames()
                        .map_err(|e| format!("查询播放缓冲区失败: {e}"))?
                        as usize;
                    if avail > 0 {
                        stereo_buf.resize(avail * 2, 0.0);
                        stream.fill_stereo(&mut stereo_buf)?;

                        stereo_bytes.clear();
                        stereo_bytes.reserve(avail * 8);
                        mono_buf.clear();
                        mono_buf.reserve(avail);

                        for pair in stereo_buf.chunks_exact(2) {
                            let l = pair[0].clamp(-1.0, 1.0);
                            let r = pair[1].clamp(-1.0, 1.0);
                            stereo_bytes.extend_from_slice(&l.to_le_bytes());
                            stereo_bytes.extend_from_slice(&r.to_le_bytes());
                            mono_buf.push((l + r) * 0.5);
                        }

                        render
                            .write_to_device(avail, &stereo_bytes, None)
                            .map_err(|e| format!("写入播放缓冲区失败: {e}"))?;
                        let _ = prod.push_slice(&mono_buf);
                    }
                    let _ = event.wait_for_event(50);
                }

                let _ = client.stop_stream();
                Ok(())
            })();

            if let Err(err) = run {
                let msg = format!("本地音频播放失败：{err}");
                if ready_tx.send(Err(msg.clone())).is_err() {
                    on_error(msg);
                }
            }
        })
        .map_err(|e| format!("无法创建音频播放线程: {e}"))?;

    match ready_rx.recv() {
        Ok(Ok(())) => Ok(ProducerTask::from_parts(stop, thread)),
        Ok(Err(err)) => {
            let _ = thread.join();
            Err(err)
        }
        Err(_) => {
            let _ = thread.join();
            Err("本地音频播放线程意外退出".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;
    use std::io::Write;

    fn write_test_wav_mono_16bit(path: &Path, sample_rate: u32, num_samples: usize) {
        let data_bytes = (num_samples * 2) as u32;
        let riff_size = 36 + data_bytes;
        let mut f = File::create(path).expect("create temp wav");
        f.write_all(b"RIFF").unwrap();
        f.write_all(&riff_size.to_le_bytes()).unwrap();
        f.write_all(b"WAVE").unwrap();
        f.write_all(b"fmt ").unwrap();
        f.write_all(&16u32.to_le_bytes()).unwrap(); // PCM chunk size
        f.write_all(&1u16.to_le_bytes()).unwrap(); // PCM format
        f.write_all(&1u16.to_le_bytes()).unwrap(); // 1 channel
        f.write_all(&sample_rate.to_le_bytes()).unwrap();
        f.write_all(&(sample_rate * 2).to_le_bytes()).unwrap(); // byte rate
        f.write_all(&2u16.to_le_bytes()).unwrap(); // block align
        f.write_all(&16u16.to_le_bytes()).unwrap(); // bits per sample
        f.write_all(b"data").unwrap();
        f.write_all(&data_bytes.to_le_bytes()).unwrap();
        for i in 0..num_samples {
            let s = (TAU * 440.0 * (i as f32) / (sample_rate as f32)).sin() * 0.5;
            let i16_val = (s * 32767.0).round() as i16;
            f.write_all(&i16_val.to_le_bytes()).unwrap();
        }
    }

    #[test]
    fn symphonia_decodes_resamples_and_loops_wav() {
        let tmp = std::env::temp_dir().join(format!(
            "pet_audio_test_{}.wav",
            std::process::id()
        ));
        // 0.1s of 44.1 kHz mono audio -> 4410 samples
        write_test_wav_mono_16bit(&tmp, 44_100, 4410);

        let mut stream = DecodedStream::open(&tmp).expect("open temp wav");
        // Request 0.25s of 48 kHz stereo (12000 frames = 24000 floats) -> forces at least 2 loops!
        let mut stereo = vec![0.0_f32; 12_000 * 2];
        stream
            .fill_stereo(&mut stereo)
            .expect("fill_stereo must decode, resample, and loop cleanly");

        let _ = std::fs::remove_file(&tmp);

        let peak = stereo.iter().map(|v| v.abs()).fold(0.0_f32, f32::max);
        assert!(peak > 0.35 && peak <= 0.6, "unexpected peak amplitude: {peak}");
        // Verify stereo duplication from mono source
        for pair in stereo.chunks_exact(2).take(100) {
            assert!((pair[0] - pair[1]).abs() < 1e-6);
        }
    }
}
