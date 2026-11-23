//! 内置角色「樱音」的加载、渲染快照与性能。
//!
//! 有意修改画面后，用 `UPDATE_SNAPSHOTS=1 cargo test -p yume-character` 更新基准图。
//! 每次运行都会把实际渲染结果写到 `target/snapshots/`，方便人工查看。
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    reason = "测试辅助函数，出错即测试失败"
)]

use std::path::{Path, PathBuf};
use yume_beat::MotionFrame;
use yume_character::{CharacterRenderer, Frame, LayeredCharacter, Pack, PackSource};
use yume_core::derive::PetScene;
use yume_core::model::events::{ActivityMode, RandomEvent};

struct Dir(PathBuf);

impl PackSource for Dir {
    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, String> {
        match std::fs::read(self.0.join(path)) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load() -> Pack {
    Pack::load(&Dir(root().join("characters/sakura")), "sakura", None).expect("内置角色包应能加载")
}

fn scene() -> PetScene {
    PetScene {
        activity: ActivityMode::Chill,
        music: false,
        follow_beat: false,
        timer_running: false,
        base_expression: "neutral".into(),
        accessory_expression: None,
        event: None,
        dozing: false,
    }
}

/// 从 t=0 起按 60 fps 推进到 `until_ms`，然后渲染。
fn render(scene: &PetScene, beats: &[(f64, MotionFrame)], until_ms: f64, scale: f32) -> Frame {
    let mut ch = LayeredCharacter::new(load(), 7);
    let mut frame = Frame::default();
    let mut t = 0.0;
    let mut motion = MotionFrame::default();
    while t <= until_ms {
        for (at, m) in beats {
            if (*at - t).abs() < 8.4 {
                motion = *m;
            }
        }
        ch.advance(t, scene, &motion);
        t += 1000.0 / 60.0;
    }
    ch.render(&mut frame, scale);
    frame
}

fn snapshot(name: &str, frame: &Frame) {
    let png = frame.to_png().expect("编码 PNG");
    let out = root().join("target/snapshots");
    std::fs::create_dir_all(&out).unwrap();
    std::fs::write(out.join(format!("{name}.png")), &png).unwrap();

    let golden = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots")
        .join(format!("{name}.png"));
    if std::env::var_os("UPDATE_SNAPSHOTS").is_some() || !golden.exists() {
        std::fs::create_dir_all(golden.parent().unwrap()).unwrap();
        std::fs::write(&golden, &png).unwrap();
        return;
    }
    let expected = yume_character::sprite::decode_png(&std::fs::read(&golden).unwrap()).unwrap();
    let actual = yume_character::sprite::decode_png(&png).unwrap();
    assert_eq!(
        (actual.width, actual.height),
        (expected.width, expected.height),
        "{name} 尺寸变化"
    );
    let differing = actual
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .zip(expected.rgba.as_chunks::<4>().0)
        .filter(|(a, b)| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > 3))
        .count();
    let ratio = differing as f64 / f64::from(actual.width * actual.height);
    assert!(
        ratio < 0.002,
        "{name} 与基准图差异 {:.3}%，见 target/snapshots/{name}.png",
        ratio * 100.0
    );
}

#[test]
fn pack_loads_with_all_expressions() {
    let pack = load();
    let ids: Vec<_> = pack.expression_meta().into_iter().map(|m| m.id).collect();
    assert_eq!(
        ids,
        ["neutral", "happy", "sad", "think", "doze", "shock", "love"]
    );
}

#[test]
fn chill_idle_snapshot_and_hit_test() {
    let frame = render(&scene(), &[], 1000.0, 1.0);
    assert_eq!((frame.width(), frame.height()), (500, 503));
    // 四角透明（点击穿透），脸部不透明（可以拖动）。
    assert_eq!(frame.alpha_at(2, 2), 0);
    assert_eq!(frame.alpha_at(497, 2), 0);
    assert_eq!(frame.alpha_at(250, 240), 255);
    snapshot("chill_idle", &frame);
}

#[test]
fn study_with_timer_snapshot() {
    let s = PetScene {
        activity: ActivityMode::Study,
        timer_running: true,
        ..scene()
    };
    snapshot("study_timer", &render(&s, &[], 1200.0, 1.0));
}

#[test]
fn music_beat_snapshot() {
    let s = PetScene {
        music: true,
        follow_beat: true,
        ..scene()
    };
    let beat = |n: u32| MotionFrame {
        beats: n,
        strength: 1.2,
        level: 0.4,
        ..MotionFrame::default()
    };
    let beats: Vec<_> = (1..=4).map(|n| (f64::from(n) * 500.0, beat(n))).collect();
    snapshot("music_beat", &render(&s, &beats, 2150.0, 1.0));
}

#[test]
fn doze_and_happy_snapshots() {
    let doze = PetScene {
        accessory_expression: Some("doze".into()),
        event: Some(RandomEvent::Doze),
        dozing: true,
        ..scene()
    };
    snapshot("doze", &render(&doze, &[], 3000.0, 1.0));
    let happy = PetScene {
        base_expression: "happy".into(),
        ..scene()
    };
    snapshot("happy", &render(&happy, &[], 800.0, 1.0));
}

#[test]
fn idle_frames_are_skipped_when_nothing_moves_visibly() {
    let mut ch = LayeredCharacter::new(load(), 3);
    let mut frame = Frame::default();
    let motion = MotionFrame::default();
    let frames = 300;
    let mut changed = 0;
    for i in 0..frames {
        ch.advance(f64::from(i) * 1000.0 / 30.0, &scene(), &motion);
        if ch.render(&mut frame, 1.0) {
            changed += 1;
        }
    }
    // 第一帧必画；之后只有眨眼和明显的呼吸位移才重画。
    assert!(changed >= 1);
    assert!(
        changed * 2 < frames,
        "空闲时 {frames} 帧里重画了 {changed} 帧"
    );
}

#[test]
#[allow(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::print_stderr,
    reason = "性能测试需要读时钟并输出耗时；只在测试里用"
)]
fn hidpi_render_cost() {
    let s = PetScene {
        music: true,
        follow_beat: true,
        ..scene()
    };
    let mut ch = LayeredCharacter::new(load(), 1);
    let mut frame = Frame::default();
    let frames = 120;
    let start = std::time::Instant::now();
    for i in 0..frames {
        let t = f64::from(i) * 1000.0 / 60.0;
        let motion = MotionFrame {
            beats: i / 30,
            strength: 1.0,
            level: 0.3,
            ..MotionFrame::default()
        };
        ch.advance(t, &s, &motion);
        ch.render(&mut frame, 2.0);
    }
    let per_frame = start.elapsed().as_secs_f64() * 1000.0 / f64::from(frames);
    eprintln!(
        "200% 缩放下每帧渲染 {per_frame:.2} ms（{}×{}）",
        frame.width(),
        frame.height()
    );
    assert_eq!((frame.width(), frame.height()), (999, 1006));
}
