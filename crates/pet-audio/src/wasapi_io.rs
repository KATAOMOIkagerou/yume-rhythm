//! Windows WASAPI loopback capture, shared-mode audio rendering, and endpoint enumeration.

use crate::synth::{generate_completion_chime, DemoSynth};
use pet_core::AudioDeviceInfo;
use ringbuf::traits::Producer;
use ringbuf::HeapProd;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use wasapi::{Device, DeviceCollection, Direction, SampleType, StreamMode, WaveFormat};

pub const ENGINE_SAMPLE_RATE: u32 = 48_000;

/// RAII COM multi-threaded apartment initializer.
/// Must be declared BEFORE any COM/WASAPI objects so it drops LAST.
pub struct ComGuard {
    should_uninit: bool,
}

impl ComGuard {
    pub fn init_mta() -> Result<Self, String> {
        let hr = wasapi::initialize_mta();
        if hr.is_ok() {
            Ok(Self {
                should_uninit: true,
            })
        } else {
            // RPC_E_CHANGED_MODE (0x80010106): COM already initialized on this thread as STA.
            // On a dedicated worker thread we expect S_OK/S_FALSE, but allow STA fallback cleanly.
            if hr.0 as u32 == 0x8001_0106 {
                Ok(Self {
                    should_uninit: false,
                })
            } else {
                Err(format!("COM 初始化失败: {hr:?}"))
            }
        }
    }
}

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.should_uninit {
            wasapi::deinitialize();
        }
    }
}

/// Enumerates all active Windows audio output (render) endpoints, marking the system default first.
pub fn enumerate_output_devices() -> Result<Vec<AudioDeviceInfo>, String> {
    let _com = ComGuard::init_mta()?;
    let default_id = wasapi::get_default_device(&Direction::Render)
        .ok()
        .and_then(|dev| dev.get_id().ok());

    let collection = DeviceCollection::new(&Direction::Render)
        .map_err(|e| format!("枚举输出设备失败: {e}"))?;
    let count = collection
        .get_nbr_devices()
        .map_err(|e| format!("获取设备数量失败: {e}"))?;

    let mut devices = Vec::with_capacity(count as usize);
    for i in 0..count {
        let Ok(dev) = collection.get_device_at_index(i) else {
            continue;
        };
        let Ok(id) = dev.get_id() else {
            continue;
        };
        let name = dev
            .get_friendlyname()
            .unwrap_or_else(|_| format!("输出设备 {}", i + 1));
        let is_default = default_id.as_deref() == Some(id.as_str());
        devices.push(AudioDeviceInfo {
            id,
            name,
            is_default,
        });
    }

    // Place default device first while preserving relative order of others.
    devices.sort_by_key(|d| !d.is_default);
    Ok(devices)
}

pub(crate) fn resolve_render_device(device_id: Option<&str>) -> Result<Device, String> {
    match device_id {
        None | Some("") | Some("default") => wasapi::get_default_device(&Direction::Render)
            .map_err(|e| format!("无法获取系统默认输出设备: {e}")),
        Some(target_id) => {
            let collection = DeviceCollection::new(&Direction::Render)
                .map_err(|e| format!("枚举输出设备失败: {e}"))?;
            let count = collection
                .get_nbr_devices()
                .map_err(|e| format!("获取设备数量失败: {e}"))?;
            for i in 0..count {
                if let Ok(dev) = collection.get_device_at_index(i) {
                    if dev.get_id().ok().as_deref() == Some(target_id) {
                        return Ok(dev);
                    }
                }
            }
            Err("未找到指定的音频输出设备，请刷新设备列表或切回系统默认输出。".to_string())
        }
    }
}

/// Active background audio producer (loopback capture, local file playback, or demo synth).
pub struct ProducerTask {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl ProducerTask {
    pub fn from_parts(stop: Arc<AtomicBool>, thread: JoinHandle<()>) -> Self {
        Self {
            stop,
            thread: Some(thread),
        }
    }

    pub fn stop_and_join(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for ProducerTask {
    fn drop(&mut self) {
        self.stop_and_join();
    }
}

/// Starts Windows WASAPI render loopback capture on either default or specific output endpoint.
///
/// Pushes downmixed mono `f32` PCM at `ENGINE_SAMPLE_RATE` (48 kHz) directly into `prod`.
pub fn start_loopback_capture(
    device_id: Option<String>,
    mut prod: HeapProd<f32>,
    on_error: Arc<dyn Fn(String) + Send + Sync>,
) -> Result<ProducerTask, String> {
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();

    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);

    let thread = thread::Builder::new()
        .name("pet-wasapi-loopback".into())
        .spawn(move || {
            let run = (|| -> Result<(), String> {
                let _com = ComGuard::init_mta()?;
                let device = resolve_render_device(device_id.as_deref())?;
                let mut client = device
                    .get_iaudioclient()
                    .map_err(|e| format!("创建音频客户端失败: {e}"))?;

                let format =
                    WaveFormat::new(32, 32, &SampleType::Float, ENGINE_SAMPLE_RATE as usize, 2, None);
                let (_, minimum) = client
                    .get_device_period()
                    .map_err(|e| format!("获取设备周期失败: {e}"))?;

                client
                    .initialize_client(
                        &format,
                        &Direction::Capture,
                        &StreamMode::EventsShared {
                            autoconvert: true,
                            buffer_duration_hns: minimum,
                        },
                    )
                    .map_err(|e| format!("初始化回环捕获失败: {e}"))?;

                let event = client
                    .set_get_eventhandle()
                    .map_err(|e| format!("创建音频事件句柄失败: {e}"))?;
                let capture = client
                    .get_audiocaptureclient()
                    .map_err(|e| format!("获取捕获客户端失败: {e}"))?;

                client
                    .start_stream()
                    .map_err(|e| format!("启动系统音频监听失败: {e}"))?;

                let _ = ready_tx.send(Ok(()));

                let mut raw_buf = vec![0u8; (ENGINE_SAMPLE_RATE as usize / 4) * 8];
                let mut mono_buf = Vec::with_capacity(4096);

                while !worker_stop.load(Ordering::Acquire) {
                    loop {
                        let next_frames = capture
                            .get_next_packet_size()
                            .map_err(|e| format!("读取音频包状态失败: {e}"))?
                            .unwrap_or(0);
                        if next_frames == 0 {
                            break;
                        }
                        let needed_bytes = (next_frames as usize) * 8;
                        if raw_buf.len() < needed_bytes {
                            raw_buf.resize(needed_bytes, 0);
                        }
                        let (frames, flags) = capture
                            .read_from_device(&mut raw_buf[..needed_bytes])
                            .map_err(|e| format!("读取回环音频流失败: {e}"))?;
                        if frames == 0 {
                            break;
                        }
                        let frame_count = frames as usize;
                        mono_buf.clear();
                        if flags.silent {
                            mono_buf.resize(frame_count, 0.0);
                        } else {
                            let slice = &raw_buf[..frame_count * 8];
                            for chunk in slice.chunks_exact(8) {
                                let l = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                                let r = f32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
                                mono_buf.push((l + r) * 0.5);
                            }
                        }
                        let _ = prod.push_slice(&mono_buf);
                    }

                    // Silence frequently times out; not a stream failure.
                    let _ = event.wait_for_event(25);
                }

                let _ = client.stop_stream();
                Ok(())
            })();

            if let Err(err) = run {
                let msg = format!("无法监听系统输出，请检查默认播放设备：{err}");
                // If startup handshake hasn't been sent yet, return via ready_tx; otherwise report runtime error.
                if ready_tx.send(Err(msg.clone())).is_err() {
                    on_error(msg);
                }
            }
        })
        .map_err(|e| format!("无法创建回环监听线程: {e}"))?;

    match ready_rx.recv() {
        Ok(Ok(())) => Ok(ProducerTask::from_parts(stop, thread)),
        Ok(Err(err)) => {
            let _ = thread.join();
            Err(err)
        }
        Err(_) => {
            let _ = thread.join();
            Err("系统音频监听线程意外退出".to_string())
        }
    }
}

/// Starts the built-in 16-step drum & bass demo synthesizer (`A1`), rendering via WASAPI
/// shared-mode speakers (falling back to headless real-time pacing if no speakers exist)
/// while feeding mono PCM into `prod`.
pub fn start_demo_player(bpm: f32, mut prod: HeapProd<f32>) -> Result<ProducerTask, String> {
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();

    let thread = thread::Builder::new()
        .name("pet-demo-synth".into())
        .spawn(move || {
            let mut synth = DemoSynth::new(ENGINE_SAMPLE_RATE, bpm);
            let mut mono_buf = vec![0.0_f32; 2048];
            let mut stereo_bytes = Vec::with_capacity(2048 * 8);

            // Try WASAPI playback first so user hears the demo beat.
            let wasapi_ok = (|| -> Result<(), String> {
                let _com = ComGuard::init_mta()?;
                let device = wasapi::get_default_device(&Direction::Render)
                    .map_err(|e| format!("{e}"))?;
                let mut client = device.get_iaudioclient().map_err(|e| format!("{e}"))?;
                let format =
                    WaveFormat::new(32, 32, &SampleType::Float, ENGINE_SAMPLE_RATE as usize, 2, None);
                let (default_period, _) =
                    client.get_device_period().map_err(|e| format!("{e}"))?;
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
                    .map_err(|e| format!("{e}"))?;
                let event = client.set_get_eventhandle().map_err(|e| format!("{e}"))?;
                let render = client
                    .get_audiorenderclient()
                    .map_err(|e| format!("{e}"))?;
                client.start_stream().map_err(|e| format!("{e}"))?;

                while !worker_stop.load(Ordering::Acquire) {
                    let avail = client
                        .get_available_space_in_frames()
                        .map_err(|e| format!("{e}"))? as usize;
                    if avail > 0 {
                        if mono_buf.len() < avail {
                            mono_buf.resize(avail, 0.0);
                        }
                        let slice = &mut mono_buf[..avail];
                        synth.render_mono(slice);
                        let _ = prod.push_slice(slice);

                        stereo_bytes.clear();
                        stereo_bytes.reserve(avail * 8);
                        for &s in slice.iter() {
                            let b = s.to_le_bytes();
                            stereo_bytes.extend_from_slice(&b);
                            stereo_bytes.extend_from_slice(&b);
                        }
                        render
                            .write_to_device(avail, &stereo_bytes, None)
                            .map_err(|e| format!("{e}"))?;
                    }
                    let _ = event.wait_for_event(50);
                }
                let _ = client.stop_stream();
                Ok(())
            })();

            // Headless real-time fallback if no Windows render endpoint is available.
            if wasapi_ok.is_err() && !worker_stop.load(Ordering::Acquire) {
                let chunk_frames = 480usize; // 10 ms at 48 kHz
                mono_buf.resize(chunk_frames, 0.0);
                let mut next_tick = Instant::now();
                while !worker_stop.load(Ordering::Acquire) {
                    synth.render_mono(&mut mono_buf[..chunk_frames]);
                    let _ = prod.push_slice(&mono_buf[..chunk_frames]);
                    next_tick += Duration::from_millis(10);
                    let now = Instant::now();
                    if next_tick > now {
                        thread::sleep(next_tick - now);
                    } else {
                        next_tick = now;
                    }
                }
            }
        })
        .map_err(|e| format!("无法创建演示鼓点线程: {e}"))?;

    Ok(ProducerTask::from_parts(stop, thread))
}

/// Plays the 3-tone countdown completion chime asynchronously over WASAPI default render.
pub fn play_completion_chime_async() {
    let _ = thread::Builder::new()
        .name("pet-chime".into())
        .spawn(|| {
            let _ = play_completion_chime_blocking();
        });
}

fn play_completion_chime_blocking() -> Result<(), String> {
    let _com = ComGuard::init_mta()?;
    let device =
        wasapi::get_default_device(&Direction::Render).map_err(|e| format!("{e}"))?;
    let mut client = device.get_iaudioclient().map_err(|e| format!("{e}"))?;
    let format = WaveFormat::new(32, 32, &SampleType::Float, ENGINE_SAMPLE_RATE as usize, 2, None);
    let (default_period, _) = client.get_device_period().map_err(|e| format!("{e}"))?;
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
        .map_err(|e| format!("{e}"))?;

    let event = client.set_get_eventhandle().map_err(|e| format!("{e}"))?;
    let render = client
        .get_audiorenderclient()
        .map_err(|e| format!("{e}"))?;

    let mut samples = generate_completion_chime(ENGINE_SAMPLE_RATE);
    // Append ~60 ms trailing silence so the hardware buffer drains completely before stop.
    samples.resize(samples.len() + (ENGINE_SAMPLE_RATE as usize * 60 / 1000), 0.0);

    client.start_stream().map_err(|e| format!("{e}"))?;

    let mut cursor = 0usize;
    let mut stereo_bytes = Vec::with_capacity(4096 * 8);
    while cursor < samples.len() {
        let avail = client
            .get_available_space_in_frames()
            .map_err(|e| format!("{e}"))? as usize;
        if avail > 0 {
            let count = avail.min(samples.len() - cursor);
            stereo_bytes.clear();
            stereo_bytes.reserve(count * 8);
            for &s in &samples[cursor..cursor + count] {
                let b = s.to_le_bytes();
                stereo_bytes.extend_from_slice(&b);
                stereo_bytes.extend_from_slice(&b);
            }
            render
                .write_to_device(count, &stereo_bytes, None)
                .map_err(|e| format!("{e}"))?;
            cursor += count;
        }
        if cursor < samples.len() {
            let _ = event.wait_for_event(50);
        }
    }

    // Brief drain wait so final packet plays out cleanly.
    thread::sleep(Duration::from_millis(50));
    let _ = client.stop_stream();
    Ok(())
}
