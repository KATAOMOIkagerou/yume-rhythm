use crate::metrics::ProcessSample;
use crate::shell::{init_dpi_awareness, pump_win32_messages, wait_for_messages_or_timeout};
use pet_audio::AudioEngineHandle;
use pet_core::{
    load_character_assets, load_persisted_state, monotonic_ms, wall_ms, AppAction, AppState,
    CharacterAnimator, MainTab,
};
use pet_render::{PetCompositor, PetRenderInput, PetWindow, PET_LOGICAL_HEIGHT, PET_LOGICAL_WIDTH};
use pet_ui::{
    AnchorRect, ConsoleRuntimeMetrics, ConsoleWindow, HeadlessConsole, CONSOLE_LOGICAL_HEIGHT,
    CONSOLE_LOGICAL_WIDTH,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

pub struct BenchOptions {
    pub duration_secs: f64,
    pub with_console: bool,
    pub console_tab: Option<MainTab>,
    pub screenshot: Option<PathBuf>,
    pub console_screenshot: Option<PathBuf>,
    pub out_file: Option<PathBuf>,
}

pub fn run_self_test(data_dir: &Path, out_file: Option<&Path>) -> Result<(), String> {
    let assets = load_character_assets()?;
    let mut state = AppState::new(load_persisted_state(data_dir));
    let mut animator = CharacterAnimator::default();
    let mut compositor = PetCompositor::new(PET_LOGICAL_WIDTH, PET_LOGICAL_HEIGHT);

    // 1. Render 100 frames and verify premultiplied alpha invariant (zero white halo) + timing
    let mut frame_times_ms = Vec::with_capacity(100);
    let mut premul_violations = 0usize;
    let mut opaque_pixels_last = 0usize;
    let start_ms = monotonic_ms();

    for i in 0..100u64 {
        let now_ms = start_ms + (i as f64) * (1000.0 / 24.0);
        let layers = state.resolve_face_layers(&assets.manifest, &assets.expressions, i % 20 == 0);
        let pose = animator.step(
            now_ms,
            &state.audio_telemetry,
            false,
            true,
            layers.is_dozing,
            layers.suppress_blink,
            assets.manifest.nod_amplitude_scale,
            assets.manifest.nod_tilt_scale,
        );
        let t0 = Instant::now();
        let input = PetRenderInput {
            assets: &assets,
            pose: &pose,
            layers: &layers,
            activity_mode: state.activity_mode,
            music_running: false,
            timer_active: false,
            pet_locked: false,
            elapsed_ms: i * 42,
        };
        let bgra = compositor.render_frame(&input);
        frame_times_ms.push(t0.elapsed().as_secs_f64() * 1000.0);

        if i == 99 {
            for px in bgra.chunks_exact(4) {
                let (b, g, r, a) = (px[0], px[1], px[2], px[3]);
                if b > a || g > a || r > a {
                    premul_violations += 1;
                }
                if a > 0 {
                    opaque_pixels_last += 1;
                }
            }
        }
    }

    // 2. Render all 4 console tabs headlessly via egui + pure-Rust CPU software rasterizer
    let mut headless = HeadlessConsole::new(CONSOLE_LOGICAL_WIDTH, CONSOLE_LOGICAL_HEIGHT, 1.0);
    headless.metrics = ConsoleRuntimeMetrics {
        sprite_count: assets.sprites.len(),
        asset_decoded_bytes: assets.total_decoded_bytes,
        asset_full_canvas_bytes: assets.full_canvas_equivalent_bytes,
        pet_frame_ms: 0.0,
        ui_frame_ms: 0.0,
        private_memory_mib: 0.0,
        loaded_font_name: headless.metrics.loaded_font_name.clone(),
    };
    let mut tab_checks = 0usize;
    let mut ui_times_ms = Vec::with_capacity(4);
    let mut ui_min_non_bg = usize::MAX;
    for tab in MainTab::ALL {
        let _ = state.dispatch(
            AppAction::SelectTab(tab),
            &assets.expressions,
            monotonic_ms(),
            wall_ms(),
        );
        let (_actions, elapsed_ms) =
            headless.step(&mut state, &assets.expressions, monotonic_ms(), Vec::new());
        ui_times_ms.push(elapsed_ms);
        let non_empty = headless
            .framebuffer_bgra()
            .chunks_exact(4)
            .filter(|px| px[3] == 255)
            .count();
        ui_min_non_bg = ui_min_non_bg.min(non_empty);
        tab_checks += 1;
    }
    let chinese_font_loaded = headless.font_info.path.is_some();
    let (ui_avg_ms, _, _, ui_max_ms) = summarize_ms(&mut ui_times_ms);

    // 3. Offline 120 BPM beat lock verification
    let detected_bpm = AudioEngineHandle::self_test_120bpm()?;
    let bpm_ok = (118..=122).contains(&detected_bpm);

    // 4. Process memory measurement
    let mem = ProcessSample::capture(monotonic_ms());
    let (avg_ms, p50_ms, p95_ms, max_ms) = summarize_ms(&mut frame_times_ms);
    let compression_ratio = if assets.full_canvas_equivalent_bytes > 0 {
        1.0 - (assets.total_decoded_bytes as f64 / assets.full_canvas_equivalent_bytes as f64)
    } else {
        0.0
    };

    let ok = premul_violations == 0
        && opaque_pixels_last > 10_000
        && bpm_ok
        && tab_checks == 4
        && chinese_font_loaded;
    let report = serde_json::json!({
        "ok": ok,
        "packName": assets.manifest.pack_name,
        "sourceHint": assets.source_hint,
        "expressionsCount": assets.expressions.len(),
        "spritesCount": assets.sprites.len(),
        "decodedKiB": assets.total_decoded_bytes / 1024,
        "fullCanvasKiB": assets.full_canvas_equivalent_bytes / 1024,
        "tightBboxSavedPct": (compression_ratio * 1000.0).round() / 10.0,
        "compositor": {
            "frames": 100,
            "avgMs": round3(avg_ms),
            "p50Ms": round3(p50_ms),
            "p95Ms": round3(p95_ms),
            "maxMs": round3(max_ms),
            "opaquePixels": opaque_pixels_last,
            "premulViolations": premul_violations,
        },
        "consoleUi": {
            "tabsChecked": tab_checks,
            "chineseFontLoaded": chinese_font_loaded,
            "fontName": headless.metrics.loaded_font_name,
            "avgRasterMs": round3(ui_avg_ms),
            "maxRasterMs": round3(ui_max_ms),
            "opaquePixelsMin": ui_min_non_bg,
            "atlasKiB": headless.texture_bytes() / 1024,
        },
        "audioSelfTest": {
            "targetBpm": 120,
            "detectedBpm": detected_bpm,
            "withinTolerance": bpm_ok,
        },
        "memory": {
            "privateMiB": round2(mem.private_mib()),
            "workingSetMiB": round2(mem.working_set_mib()),
        }
    });

    let serialized = serde_json::to_string_pretty(&report).unwrap_or_else(|_| report.to_string());
    if let Some(path) = out_file {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|e| format!("创建报告目录失败：{e}"))?;
            }
        }
        fs::write(path, &serialized).map_err(|e| format!("写入自检报告失败：{e}"))?;
    }
    println!("{serialized}");

    if !ok {
        return Err(format!(
            "自检未通过：premulViolations={premul_violations}, detectedBpm={detected_bpm}"
        ));
    }
    Ok(())
}

pub fn run_bench(data_dir: &Path, opts: &BenchOptions) -> Result<(), String> {
    init_dpi_awareness();
    let assets = load_character_assets()?;
    let mut state = AppState::new(load_persisted_state(data_dir));
    let mut animator = CharacterAnimator::default();
    let mut pet = PetWindow::create()?;
    let mut console = ConsoleWindow::new();
    state.asset_status.loaded = true;
    state.asset_status.pack_name = assets.manifest.pack_name.clone();
    state.asset_status.source_hint = assets.source_hint.clone();
    if let Some(tab) = opts.console_tab {
        state.active_tab = tab;
    }

    if opts.with_console {
        let r = pet.window_rect();
        console.set_runtime_metrics(ConsoleRuntimeMetrics {
            sprite_count: assets.sprites.len(),
            asset_decoded_bytes: assets.total_decoded_bytes,
            asset_full_canvas_bytes: assets.full_canvas_equivalent_bytes,
            pet_frame_ms: pet.last_render_ms() as f32,
            ui_frame_ms: 0.0,
            private_memory_mib: ProcessSample::capture(monotonic_ms()).private_mib() as f32,
            loaded_font_name: None,
        });
        console.show_near(AnchorRect {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
            scale_factor: r.scale_factor,
        })?;
    }

    let duration_ms = (opts.duration_secs.clamp(0.5, 120.0)) * 1000.0;
    let start_ms = monotonic_ms();
    let end_ms = start_ms + duration_ms;
    let frame_step_ms = 1000.0 / 24.0;
    let mut next_frame_ms = start_ms;
    let mut next_sample_ms = start_ms + 250.0;

    let first_sample = ProcessSample::capture(start_ms);
    let mut prev_sample = first_sample;
    let mut samples = Vec::new();
    let mut render_times_ms = Vec::new();
    let mut present_times_ms = Vec::new();
    let mut cpu_peak_pct = 0.0f64;
    let mut private_peak_mib = first_sample.private_mib();

    loop {
        if pump_win32_messages() {
            break;
        }
        let now_ms = monotonic_ms();
        if now_ms >= end_ms {
            break;
        }

        if console.is_visible() {
            let actions = console.update(&mut state, &assets.expressions, now_ms);
            for action in actions {
                let _ = state.dispatch(action, &assets.expressions, now_ms, wall_ms());
            }
        }

        if now_ms >= next_frame_ms {
            let layers = state.resolve_face_layers(&assets.manifest, &assets.expressions, false);
            let pose = animator.step(
                now_ms,
                &state.audio_telemetry,
                false,
                true,
                layers.is_dozing,
                layers.suppress_blink,
                assets.manifest.nod_amplitude_scale,
                assets.manifest.nod_tilt_scale,
            );
            let input = PetRenderInput {
                assets: &assets,
                pose: &pose,
                layers: &layers,
                activity_mode: state.activity_mode,
                music_running: false,
                timer_active: false,
                pet_locked: state.pet_locked,
                elapsed_ms: (now_ms - start_ms) as u64,
            };
            let t0 = Instant::now();
            pet.present(&input)?;
            present_times_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
            render_times_ms.push(pet.last_render_ms());
            next_frame_ms = (next_frame_ms + frame_step_ms).max(now_ms + 1.0);
        }

        if now_ms >= next_sample_ms {
            let cur = ProcessSample::capture(now_ms);
            let cpu = cur.cpu_percent_since(&prev_sample);
            if cpu > cpu_peak_pct {
                cpu_peak_pct = cpu;
            }
            if cur.private_mib() > private_peak_mib {
                private_peak_mib = cur.private_mib();
            }
            samples.push(cur);
            prev_sample = cur;
            next_sample_ms = now_ms + 250.0;
        }

        let wait_ms = (next_frame_ms - monotonic_ms()).clamp(1.0, 42.0) as u32;
        wait_for_messages_or_timeout(wait_ms);
    }

    if let Some(screenshot_path) = &opts.screenshot {
        let (bgra, w, h) = pet.last_frame_bgra();
        if !bgra.is_empty() && w > 0 && h > 0 {
            save_premul_bgra_png(screenshot_path, w, h, bgra)?;
        }
    }

    let derived_console_path = opts.console_screenshot.clone().or_else(|| {
        if opts.with_console {
            opts.screenshot.as_ref().map(|p| {
                let stem = p
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("bench");
                let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("png");
                p.with_file_name(format!("{stem}_console.{ext}"))
            })
        } else {
            None
        }
    });

    let final_sample = ProcessSample::capture(monotonic_ms());
    let cpu_avg_pct = final_sample.cpu_percent_since(&first_sample);
    let avg_private_mib = if samples.is_empty() {
        final_sample.private_mib()
    } else {
        samples.iter().map(|s| s.private_mib()).sum::<f64>() / samples.len() as f64
    };
    let (render_avg_ms, render_p50_ms, render_p95_ms, render_max_ms) =
        summarize_ms(&mut render_times_ms);
    let (present_avg_ms, present_p50_ms, present_p95_ms, present_max_ms) =
        summarize_ms(&mut present_times_ms);

    if let Some(console_path) = &derived_console_path {
        let scale = pet.window_rect().scale_factor.clamp(1.0, 2.0);
        let cw = (CONSOLE_LOGICAL_WIDTH as f32 * scale).round() as u32;
        let ch = (CONSOLE_LOGICAL_HEIGHT as f32 * scale).round() as u32;
        let mut headless = HeadlessConsole::new(cw, ch, scale);
        headless.metrics = ConsoleRuntimeMetrics {
            sprite_count: assets.sprites.len(),
            asset_decoded_bytes: assets.total_decoded_bytes,
            asset_full_canvas_bytes: assets.full_canvas_equivalent_bytes,
            pet_frame_ms: render_avg_ms as f32,
            ui_frame_ms: 0.0,
            private_memory_mib: final_sample.private_mib() as f32,
            loaded_font_name: headless.metrics.loaded_font_name.clone(),
        };
        let _ = headless.step(&mut state, &assets.expressions, monotonic_ms(), Vec::new());
        let _ = headless.step(&mut state, &assets.expressions, monotonic_ms() + 16.0, Vec::new());
        save_premul_bgra_png(
            console_path,
            headless.width() as u32,
            headless.height() as u32,
            headless.framebuffer_bgra(),
        )?;
    }

    console.hide_and_release();

    let report = serde_json::json!({
        "durationSecs": round2((final_sample.timestamp_ms - first_sample.timestamp_ms) / 1000.0),
        "withConsole": opts.with_console,
        "framesRendered": present_times_ms.len(),
        "renderMs": {
            "avg": round3(render_avg_ms),
            "p50": round3(render_p50_ms),
            "p95": round3(render_p95_ms),
            "max": round3(render_max_ms),
        },
        "presentMs": {
            "avg": round3(present_avg_ms),
            "p50": round3(present_p50_ms),
            "p95": round3(present_p95_ms),
            "max": round3(present_max_ms),
        },
        "frameMs": {
            "avg": round3(present_avg_ms),
            "p50": round3(present_p50_ms),
            "p95": round3(present_p95_ms),
            "max": round3(present_max_ms),
        },
        "cpu": {
            "avgPct": round2(cpu_avg_pct),
            "peakPct": round2(cpu_peak_pct),
        },
        "memory": {
            "privateAvgMiB": round2(avg_private_mib),
            "privatePeakMiB": round2(private_peak_mib),
            "workingSetFinalMiB": round2(final_sample.working_set_mib()),
            "decodedAssetsKiB": assets.total_decoded_bytes / 1024,
        },
        "screenshot": opts.screenshot.as_ref().map(|p| p.display().to_string()),
        "consoleScreenshot": derived_console_path.as_ref().map(|p| p.display().to_string()),
    });

    let serialized = serde_json::to_string_pretty(&report).unwrap_or_else(|_| report.to_string());
    if let Some(path) = &opts.out_file {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|e| format!("创建基准目录失败：{e}"))?;
            }
        }
        fs::write(path, &serialized).map_err(|e| format!("写入基准报告失败：{e}"))?;
    }
    println!("{serialized}");
    Ok(())
}

fn summarize_ms(values: &mut [f64]) -> (f64, f64, f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let avg = values.iter().sum::<f64>() / values.len() as f64;
    values.sort_by(|a, b| a.total_cmp(b));
    let p50 = values[values.len() / 2];
    let p95 = values[((values.len() * 95) / 100).min(values.len() - 1)];
    let max = *values.last().unwrap_or(&0.0);
    (avg, p50, p95, max)
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// Self-contained uncompressed PNG encoder (RFC 2083 + RFC 1950 stored DEFLATE blocks)
/// converting premultiplied BGRA8 into straight RGBA8 PNG with zero extra crate dependencies.
fn save_premul_bgra_png(path: &Path, width: u32, height: u32, bgra: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| format!("创建截图目录失败：{e}"))?;
        }
    }
    let stride = (width as usize) * 4;
    let mut raw_scanlines = Vec::with_capacity((height as usize) * (stride + 1));
    for y in 0..(height as usize) {
        raw_scanlines.push(0u8); // filter type 0 (None)
        let row = &bgra[y * stride..(y + 1) * stride];
        for px in row.chunks_exact(4) {
            let (b, g, r, a) = (px[0], px[1], px[2], px[3]);
            if a == 0 {
                raw_scanlines.extend_from_slice(&[0, 0, 0, 0]);
            } else if a == 255 {
                raw_scanlines.extend_from_slice(&[r, g, b, 255]);
            } else {
                let unpremul = |c: u8| -> u8 { ((c as u32 * 255 + (a as u32 / 2)) / a as u32).min(255) as u8 };
                raw_scanlines.extend_from_slice(&[unpremul(r), unpremul(g), unpremul(b), a]);
            }
        }
    }

    let mut zlib = Vec::with_capacity(raw_scanlines.len() + (raw_scanlines.len() / 65535 + 1) * 5 + 6);
    zlib.push(0x78);
    zlib.push(0x01);
    let mut offset = 0usize;
    while offset < raw_scanlines.len() {
        let len = (raw_scanlines.len() - offset).min(65535);
        let is_final = offset + len >= raw_scanlines.len();
        zlib.push(if is_final { 1 } else { 0 });
        let len16 = len as u16;
        let nlen16 = !len16;
        zlib.extend_from_slice(&len16.to_le_bytes());
        zlib.extend_from_slice(&nlen16.to_le_bytes());
        zlib.extend_from_slice(&raw_scanlines[offset..offset + len]);
        offset += len;
    }
    let adler = adler32(&raw_scanlines);
    zlib.extend_from_slice(&adler.to_be_bytes());

    let mut png = Vec::with_capacity(zlib.len() + 64);
    png.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    let mut ihdr = [0u8; 13];
    ihdr[0..4].copy_from_slice(&width.to_be_bytes());
    ihdr[4..8].copy_from_slice(&height.to_be_bytes());
    ihdr[8] = 8; // bit depth
    ihdr[9] = 6; // RGBA
    ihdr[10] = 0;
    ihdr[11] = 0;
    ihdr[12] = 0;
    write_png_chunk(&mut png, b"IHDR", &ihdr);
    write_png_chunk(&mut png, b"IDAT", &zlib);
    write_png_chunk(&mut png, b"IEND", &[]);

    fs::write(path, png).map_err(|e| format!("保存截图失败：{e}"))
}

fn write_png_chunk(out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(tag);
    out.extend_from_slice(data);
    let mut crc = 0xFFFF_FFFFu32;
    for &b in tag.iter().chain(data.iter()) {
        crc = crc32_step(crc, b);
    }
    out.extend_from_slice(&(!crc).to_be_bytes());
}

fn crc32_step(mut crc: u32, byte: u8) -> u32 {
    crc ^= byte as u32;
    for _ in 0..8 {
        if crc & 1 != 0 {
            crc = (crc >> 1) ^ 0xEDB8_8320;
        } else {
            crc >>= 1;
        }
    }
    crc
}

fn adler32(data: &[u8]) -> u32 {
    const MOD: u32 = 65521;
    let mut a = 1u32;
    let mut b = 0u32;
    for &byte in data {
        a = (a + byte as u32) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn png_encoder_writes_valid_signature_and_chunks() {
        let tmp = std::env::temp_dir().join("yume_rhythm_png_test.png");
        let bgra = vec![
            100, 150, 200, 255, // opaque pixel
            0, 0, 0, 0, // transparent pixel
            50, 64, 100, 128, // half-alpha premul pixel
            255, 255, 255, 255, // white pixel
        ];
        save_premul_bgra_png(&tmp, 2, 2, &bgra).expect("write png");
        let bytes = fs::read(&tmp).expect("read png");
        let _ = fs::remove_file(&tmp);
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"));
        assert!(bytes.windows(4).any(|w| w == b"IHDR"));
        assert!(bytes.windows(4).any(|w| w == b"IDAT"));
        assert!(bytes.windows(4).any(|w| w == b"IEND"));
    }
}
