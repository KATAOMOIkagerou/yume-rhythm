//! Windows output-device loopback. The bounded handoff never records audio to disk.
use crate::UserEvent;
use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    thread::{self, JoinHandle},
};
use tao::event_loop::EventLoopProxy;
use wasapi::{Direction, SampleType, StreamMode, WaveFormat};

pub struct Capture {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Capture {
    pub fn start(generation: u64, request: u64, proxy: EventLoopProxy<UserEvent>) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let thread = thread::spawn(move || {
            let mut started = false;
            let result = (|| -> Result<(), Box<dyn std::error::Error>> {
                wasapi::initialize_mta().ok()?;
                // Drop this last, after every COM audio object.
                struct ComGuard;
                impl Drop for ComGuard {
                    fn drop(&mut self) {
                        wasapi::deinitialize();
                    }
                }
                let _com = ComGuard;
                let device = wasapi::get_default_device(&Direction::Render)?;
                let mut client = device.get_iaudioclient()?;
                let format = WaveFormat::new(32, 32, &SampleType::Float, 48_000, 2, None);
                let (_, minimum) = client.get_device_period()?;
                client.initialize_client(
                    &format,
                    &Direction::Capture,
                    &StreamMode::EventsShared {
                        autoconvert: true,
                        buffer_duration_hns: minimum,
                    },
                )?;
                let event = client.set_get_eventhandle()?;
                let capture = client.get_audiocaptureclient()?;
                let mut queue = VecDeque::with_capacity(32_768);
                let pending = Arc::new(AtomicUsize::new(0));
                client.start_stream()?;
                started = true;
                let _ = proxy.send_event(UserEvent::AudioStarted {
                    generation,
                    request,
                });
                while !worker_stop.load(Ordering::Acquire) {
                    capture.read_from_device_to_deque(&mut queue)?;
                    while queue.len() >= 1024 * 8 {
                        let mut mono = Vec::with_capacity(1024 * 4);
                        for _ in 0..1024 {
                            let mut bytes = [0u8; 8];
                            for byte in &mut bytes {
                                *byte = queue.pop_front().unwrap();
                            }
                            let left = f32::from_le_bytes(bytes[..4].try_into().unwrap());
                            let right = f32::from_le_bytes(bytes[4..].try_into().unwrap());
                            mono.extend_from_slice(&((left + right) * 0.5).to_le_bytes());
                        }
                        // At most three chunks (64 ms) can wait for the UI thread.
                        let queued = pending.load(Ordering::Acquire);
                        if queued < 3
                            && pending
                                .compare_exchange(
                                    queued,
                                    queued + 1,
                                    Ordering::AcqRel,
                                    Ordering::Acquire,
                                )
                                .is_ok()
                        {
                            if proxy
                                .send_event(UserEvent::Pcm {
                                    generation,
                                    bytes: mono,
                                    pending: pending.clone(),
                                })
                                .is_err()
                            {
                                break;
                            }
                        }
                    }
                    // Silence commonly times out; it is not a device failure.
                    let _ = event.wait_for_event(50);
                }
                client.stop_stream()?;
                Ok(())
            })();
            if let Err(error) = result {
                let _ = proxy.send_event(UserEvent::AudioFailed {
                    generation,
                    request: if started { None } else { Some(request) },
                    message: format!("无法监听系统输出，请检查默认播放设备：{error}"),
                });
            }
        });
        Self {
            stop,
            thread: Some(thread),
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
