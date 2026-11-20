//! 音频的启动、停止与回报（功能清单第 9 节）。

use crate::clock::WallMs;
use crate::command::AudioCommand;
use crate::effect::Effect;
use crate::model::audio::{AudioError, AudioSource, AudioStatus, Telemetry, first_audio_file};
use crate::state::AppState;

pub(super) fn handle(state: &mut AppState, cmd: AudioCommand, now: WallMs, fx: &mut Vec<Effect>) {
    match cmd {
        AudioCommand::Toggle => {
            if state.audio.status == AudioStatus::Idle {
                let source = state.audio.replay_source();
                start(state, source, fx);
            } else {
                stop(state, fx);
            }
        }
        AudioCommand::StartSystem => start(state, AudioSource::System, fx),
        AudioCommand::Stop => stop(state, fx),
        AudioCommand::FilesDropped(paths) => match first_audio_file(&paths) {
            Some(path) => start(state, AudioSource::File { path: path.clone() }, fx),
            None => state.audio.error = Some(AudioError::UnsupportedFile),
        },
        AudioCommand::SelectDevice(device) => {
            state.settings.audio_device = device;
            // 正在监听系统声音时，切换设备立即生效。
            if state.audio.status != AudioStatus::Idle
                && state.audio.source == Some(AudioSource::System)
            {
                start(state, AudioSource::System, fx);
            }
        }
        AudioCommand::RefreshDevices => fx.push(Effect::ListAudioDevices),
        AudioCommand::ShiftTempo(shift) => {
            if state.audio.is_running() {
                fx.push(Effect::ShiftTempo(shift));
            }
        }
        AudioCommand::Started { request } => {
            if request == state.audio.request && state.audio.status == AudioStatus::Starting {
                state.audio.status = AudioStatus::Running;
                state.audio.started_at = Some(now);
                state.audio.last_source.clone_from(&state.audio.source);
            }
        }
        AudioCommand::Failed { request, error } => {
            if request == state.audio.request && state.audio.status != AudioStatus::Idle {
                go_idle(state);
                state.audio.error = Some(error);
            }
        }
        AudioCommand::Ended { request } => {
            if request == state.audio.request {
                go_idle(state);
            }
        }
        AudioCommand::Telemetry(telemetry) => {
            if state.audio.is_running() {
                state.audio.telemetry = telemetry;
            }
        }
        AudioCommand::DevicesListed(devices) => state.audio.devices = devices,
    }
}

fn start(state: &mut AppState, source: AudioSource, fx: &mut Vec<Effect>) {
    let audio = &mut state.audio;
    audio.request += 1;
    audio.status = AudioStatus::Starting;
    audio.source = Some(source.clone());
    audio.error = None;
    audio.telemetry = Telemetry::default();
    audio.started_at = None;
    let device = state.settings.audio_device.clone();
    fx.push(Effect::StartAudio {
        request: audio.request,
        source,
        device,
    });
}

fn stop(state: &mut AppState, fx: &mut Vec<Effect>) {
    if state.audio.status != AudioStatus::Idle {
        fx.push(Effect::StopAudio);
    }
    // 让还在路上的"已启动"回报失效。
    state.audio.request += 1;
    go_idle(state);
}

fn go_idle(state: &mut AppState) {
    let audio = &mut state.audio;
    audio.status = AudioStatus::Idle;
    audio.source = None;
    audio.started_at = None;
    audio.telemetry = Telemetry::default();
}
