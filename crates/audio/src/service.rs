//! 音频服务线程：持有全部 cpal 流，按命令启动回环采集、文件播放、提示音。
//!
//! cpal 的流对象不保证可跨线程移动，所以都留在这一个线程里。

use crate::chime;
use crate::decode::{DecodeError, FileDecoder};
use crate::queue::PcmQueue;
use crate::resample::Resampler;
use crate::ring::SampleRing;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, DeviceId, SampleFormat, Stream, StreamConfig};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// 回环采集输出设备。`None` 为系统默认输出。
    Loopback { device: Option<String> },
    /// 循环播放文件（默认输出设备）。
    File { path: PathBuf },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    DeviceUnavailable,
    UnsupportedFile,
    FileUnreadable,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputDevice {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AudioEvent {
    Started {
        request: u64,
    },
    Failed {
        request: u64,
        kind: ErrorKind,
        detail: String,
    },
    Devices(Vec<OutputDevice>),
}

enum Cmd {
    Start { request: u64, source: Source },
    Stop,
    Chime,
    ListDevices,
    StreamError { request: u64, detail: String },
    Shutdown,
}

pub struct AudioService {
    tx: Sender<Cmd>,
    thread: Option<JoinHandle<()>>,
}

impl AudioService {
    /// 启动服务线程。采样写入 `ring`，事件通过 `on_event` 回报（在服务线程上调用）。
    pub fn spawn(ring: Arc<SampleRing>, on_event: impl Fn(AudioEvent) + Send + 'static) -> Self {
        let (tx, rx) = channel();
        let self_tx = tx.clone();
        let thread = std::thread::Builder::new()
            .name("audio-service".into())
            .spawn(move || {
                Worker {
                    ring,
                    on_event: Box::new(on_event),
                    tx: self_tx,
                    active: None,
                    chime: None,
                }
                .run(&rx)
            })
            .ok();
        Self { tx, thread }
    }

    pub fn start(&self, request: u64, source: Source) {
        let _ = self.tx.send(Cmd::Start { request, source });
    }

    pub fn stop(&self) {
        let _ = self.tx.send(Cmd::Stop);
    }

    pub fn play_chime(&self) {
        let _ = self.tx.send(Cmd::Chime);
    }

    pub fn list_devices(&self) {
        let _ = self.tx.send(Cmd::ListDevices);
    }
}

impl Drop for AudioService {
    fn drop(&mut self) {
        let _ = self.tx.send(Cmd::Shutdown);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

struct Active {
    request: u64,
    _stream: Stream,
    feeder: Option<(Arc<AtomicBool>, JoinHandle<()>)>,
}

impl Drop for Active {
    fn drop(&mut self) {
        if let Some((stop, handle)) = self.feeder.take() {
            stop.store(true, Ordering::Release);
            let _ = handle.join();
        }
    }
}

struct Worker {
    ring: Arc<SampleRing>,
    on_event: Box<dyn Fn(AudioEvent) + Send>,
    tx: Sender<Cmd>,
    active: Option<Active>,
    chime: Option<(Stream, Instant)>,
}

impl Worker {
    fn run(mut self, rx: &Receiver<Cmd>) {
        loop {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(Cmd::Start { request, source }) => {
                    self.active = None;
                    match self.open(request, &source) {
                        Ok(active) => {
                            self.active = Some(active);
                            (self.on_event)(AudioEvent::Started { request });
                        }
                        Err((kind, detail)) => {
                            log::warn!("音频启动失败：{detail}");
                            (self.on_event)(AudioEvent::Failed {
                                request,
                                kind,
                                detail,
                            });
                        }
                    }
                }
                Ok(Cmd::Stop) => self.active = None,
                Ok(Cmd::Chime) => self.chime = play_chime().map(|s| (s, Instant::now())).ok(),
                Ok(Cmd::ListDevices) => (self.on_event)(AudioEvent::Devices(list_devices())),
                Ok(Cmd::StreamError { request, detail }) => {
                    if self.active.as_ref().is_some_and(|a| a.request == request) {
                        self.active = None;
                        let kind = ErrorKind::DeviceUnavailable;
                        (self.on_event)(AudioEvent::Failed {
                            request,
                            kind,
                            detail,
                        });
                    }
                }
                Ok(Cmd::Shutdown) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            }
            if self
                .chime
                .as_ref()
                .is_some_and(|(_, at)| at.elapsed() > Duration::from_secs(2))
            {
                self.chime = None;
            }
        }
    }

    fn open(&self, request: u64, source: &Source) -> Result<Active, (ErrorKind, String)> {
        let tx = self.tx.clone();
        let on_error = move |e: cpal::Error| {
            // 偶发的欠载 / 过载、默认设备切换后自动改道，流仍在运行，不算失败。
            if matches!(
                e.kind(),
                cpal::ErrorKind::Xrun | cpal::ErrorKind::DeviceChanged
            ) {
                log::debug!("音频流提示：{e}");
                return;
            }
            let _ = tx.send(Cmd::StreamError {
                request,
                detail: e.to_string(),
            });
        };
        match source {
            Source::Loopback { device } => {
                let device = find_output(device.as_deref())?;
                let (config, channels) = f32_config(&device)?;
                log::info!(
                    "回环采集：{}，{} Hz，{channels} 声道",
                    device_name(&device),
                    config.sample_rate
                );
                self.ring.begin(config.sample_rate);
                let ring = self.ring.clone();
                let stream = device
                    .build_input_stream::<f32, _, _>(
                        config,
                        move |data, _| ring.push_interleaved(data, channels),
                        on_error,
                        None,
                    )
                    .map_err(device_error)?;
                stream.play().map_err(device_error)?;
                Ok(Active {
                    request,
                    _stream: stream,
                    feeder: None,
                })
            }
            Source::File { path } => {
                let decoder = FileDecoder::open(path).map_err(file_error)?;
                let device = find_output(None)?;
                let (config, channels) = f32_config(&device)?;
                self.ring.begin(config.sample_rate);
                let queue = Arc::new(PcmQueue::new(config.sample_rate as usize));
                let stop = Arc::new(AtomicBool::new(false));
                let feeder = spawn_feeder(
                    decoder,
                    config.sample_rate,
                    queue.clone(),
                    stop.clone(),
                    self.tx.clone(),
                    request,
                );
                let ring = self.ring.clone();
                let stream = device
                    .build_output_stream::<f32, _, _>(
                        config,
                        move |out, _| play(out, channels, &queue, &ring),
                        on_error,
                        None,
                    )
                    .map_err(device_error)?;
                stream.play().map_err(device_error)?;
                Ok(Active {
                    request,
                    _stream: stream,
                    feeder: Some((stop, feeder)),
                })
            }
        }
    }
}

/// 播放回调：从队列取立体声，写到设备声道；同时把播放的内容送去分析。
fn play(out: &mut [f32], channels: usize, queue: &PcmQueue, ring: &SampleRing) {
    let mut stereo = [0.0f32; 1024];
    for chunk in out.chunks_mut(channels * 512) {
        let frames = chunk.len() / channels;
        let got = queue.pop(&mut stereo[..frames * 2]);
        stereo[got..frames * 2].fill(0.0);
        for (f, frame) in chunk.chunks_exact_mut(channels).enumerate() {
            let (l, r) = (stereo[f * 2], stereo[f * 2 + 1]);
            match frame {
                [mono] => *mono = (l + r) * 0.5,
                [a, b, rest @ ..] => {
                    *a = l;
                    *b = r;
                    rest.fill(0.0);
                }
                [] => {}
            }
        }
        ring.push_interleaved(&stereo[..frames * 2], 2);
    }
}

fn spawn_feeder(
    mut decoder: FileDecoder,
    device_rate: u32,
    queue: Arc<PcmQueue>,
    stop: Arc<AtomicBool>,
    tx: Sender<Cmd>,
    request: u64,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let mut resampler = Resampler::new(decoder.sample_rate(), device_rate);
        let (mut decoded, mut pending) = (Vec::new(), Vec::<f32>::new());
        while !stop.load(Ordering::Acquire) {
            if pending.is_empty() {
                decoded.clear();
                if let Err(e) = decoder.next_stereo(&mut decoded) {
                    let _ = tx.send(Cmd::StreamError {
                        request,
                        detail: e.to_string(),
                    });
                    return;
                }
                resampler.process(&decoded, &mut pending);
            }
            let written = queue.push(&pending);
            pending.drain(..written);
            if !pending.is_empty() {
                std::thread::sleep(Duration::from_millis(10));
            }
        }
    })
}

fn play_chime() -> Result<Stream, (ErrorKind, String)> {
    let device = find_output(None)?;
    let (config, channels) = f32_config(&device)?;
    let samples = chime::render(config.sample_rate);
    let mut pos = 0;
    let stream = device
        .build_output_stream::<f32, _, _>(
            config,
            move |out, _| {
                for frame in out.chunks_exact_mut(channels) {
                    frame.fill(samples.get(pos).copied().unwrap_or(0.0));
                    pos += 1;
                }
            },
            |e| log::warn!("提示音播放出错：{e}"),
            None,
        )
        .map_err(device_error)?;
    stream.play().map_err(device_error)?;
    Ok(stream)
}

fn find_output(id: Option<&str>) -> Result<Device, (ErrorKind, String)> {
    let host = cpal::default_host();
    let device = match id {
        Some(id) => id
            .parse::<DeviceId>()
            .ok()
            .and_then(|id| host.device_by_id(&id)),
        None => host.default_output_device(),
    };
    device.ok_or((ErrorKind::DeviceUnavailable, "找不到输出设备".into()))
}

fn f32_config(device: &Device) -> Result<(StreamConfig, usize), (ErrorKind, String)> {
    let supported = device.default_output_config().map_err(device_error)?;
    if supported.sample_format() != SampleFormat::F32 {
        return Err((
            ErrorKind::Other,
            format!("不支持的采样格式 {:?}", supported.sample_format()),
        ));
    }
    let config = supported.config();
    let channels = usize::from(config.channels);
    Ok((config, channels))
}

fn list_devices() -> Vec<OutputDevice> {
    let host = cpal::default_host();
    let default_id = host
        .default_output_device()
        .and_then(|d| d.id().ok())
        .map(|id| id.to_string());
    let Ok(devices) = host.output_devices() else {
        return Vec::new();
    };
    devices
        .filter_map(|d| {
            let id = d.id().ok()?.to_string();
            let name = d
                .description()
                .ok()
                .map_or_else(|| id.clone(), |desc| desc.name().to_string());
            let is_default = default_id.as_deref() == Some(id.as_str());
            Some(OutputDevice {
                id,
                name,
                is_default,
            })
        })
        .collect()
}

fn device_name(device: &Device) -> String {
    device
        .description()
        .map_or_else(|_| "未知设备".to_owned(), |d| d.name().to_string())
}

fn device_error(e: cpal::Error) -> (ErrorKind, String) {
    (ErrorKind::DeviceUnavailable, e.to_string())
}

fn file_error(e: DecodeError) -> (ErrorKind, String) {
    let kind = match e {
        DecodeError::Unsupported => ErrorKind::UnsupportedFile,
        DecodeError::Io(_) | DecodeError::Decode(_) => ErrorKind::FileUnreadable,
    };
    (kind, e.to_string())
}
