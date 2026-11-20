//! 基准轨迹（`tools/beat-baseline` 生成的 JSON）的结构与读取。

use std::path::{Path, PathBuf};

use serde::Deserialize;

pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Fixture {
    pub format: u32,
    pub name: String,
    pub description: String,
    pub analyser: AnalyserSpec,
    pub sample_rate: f64,
    pub clock: Clock,
    pub skip: Vec<[f64; 2]>,
    pub source: Source,
    pub samples_hash: String,
    pub frames: usize,
    pub bands: Bands,
    pub columns: Columns,
    pub bytes: Option<Bytes>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyserSpec {
    pub fft_size: usize,
    pub smoothing: f64,
    pub min_db: f64,
    pub max_db: f64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clock {
    pub hz: f64,
    pub nod_offset_ms: f64,
    pub nod_strength: f64,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Source {
    Synth {
        spec: SynthSpec,
    },
    Wav {
        file: String,
        samples: usize,
        channels: u32,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SynthSpec {
    pub sample_rate: f64,
    pub duration: f64,
    pub gain: f64,
    pub seed: u32,
    pub sections: Vec<Section>,
}

#[derive(Debug, Deserialize)]
pub struct Section {
    pub start: f64,
    pub end: f64,
    pub bpm: f64,
    pub kick: Vec<f64>,
    pub snare: Vec<f64>,
    pub hat: Vec<f64>,
    pub bass: Vec<Option<f64>>,
}

#[derive(Debug, Deserialize)]
pub struct Bands {
    pub lo: Vec<i64>,
    pub hi: Vec<i64>,
    pub w: Vec<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Columns {
    pub beat: String,
    pub onset: String,
    pub locked: String,
    pub strength: Vec<f64>,
    pub level: Vec<f64>,
    pub bass: Vec<f64>,
    pub bpm: Vec<f64>,
    pub beats: Vec<u32>,
    pub next_beat_at: Vec<Option<f64>>,
    pub period_ms: Vec<Option<f64>>,
    pub nod: Vec<Option<f64>>,
    pub byte_hash: String,
}

#[derive(Debug, Deserialize)]
pub struct Bytes {
    pub bins: usize,
    pub base64: String,
}

/// JS 侧一帧，按列展开。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JsFrame {
    pub beat: bool,
    pub onset: bool,
    pub locked: bool,
    pub strength: f64,
    pub level: f64,
    pub bass: f64,
    pub bpm: f64,
    pub beats: u32,
    pub next_beat_at: Option<f64>,
    pub period_ms: Option<f64>,
    pub nod: Option<f64>,
    pub byte_hash: u32,
}

impl Fixture {
    pub fn load(path: &Path) -> Fixture {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("读取基准轨迹 {} 失败：{e}", path.display()));
        let fixture: Fixture = serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("解析基准轨迹 {} 失败：{e}", path.display()));
        assert_eq!(fixture.format, 1, "{}: 不支持的格式版本", path.display());
        fixture
    }

    pub fn load_named(name: &str) -> Fixture {
        Self::load(&fixtures_dir().join(format!("{name}.json")))
    }

    /// 所有合成信号的基准轨迹（按文件名排序）。
    pub fn all_synthetic() -> Vec<Fixture> {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(fixtures_dir())
            .expect("fixtures 目录")
            .map(|e| e.expect("目录项").path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .filter(|p| p.file_stem().is_some_and(|s| s != "nod_cases"))
            .collect();
        paths.sort();
        paths.iter().map(|p| Self::load(p)).collect()
    }

    pub fn js_frames(&self) -> Vec<JsFrame> {
        let c = &self.columns;
        let flags = |s: &str| -> Vec<bool> { s.bytes().map(|b| b == b'1').collect() };
        let (beat, onset, locked) = (flags(&c.beat), flags(&c.onset), flags(&c.locked));
        let n = self.frames;
        for (col, len) in [
            ("beat", beat.len()),
            ("onset", onset.len()),
            ("locked", locked.len()),
            ("strength", c.strength.len()),
            ("level", c.level.len()),
            ("bass", c.bass.len()),
            ("bpm", c.bpm.len()),
            ("beats", c.beats.len()),
            ("nextBeatAt", c.next_beat_at.len()),
            ("periodMs", c.period_ms.len()),
            ("nod", c.nod.len()),
            ("byteHash", c.byte_hash.len() / 8),
        ] {
            assert_eq!(len, n, "{}: 列 {col} 长度不对", self.name);
        }
        (0..n)
            .map(|i| JsFrame {
                beat: beat[i],
                onset: onset[i],
                locked: locked[i],
                strength: c.strength[i],
                level: c.level[i],
                bass: c.bass[i],
                bpm: c.bpm[i],
                beats: c.beats[i],
                next_beat_at: c.next_beat_at[i],
                period_ms: c.period_ms[i],
                nod: c.nod[i],
                byte_hash: u32::from_str_radix(&c.byte_hash[i * 8..i * 8 + 8], 16).expect("hex"),
            })
            .collect()
    }

    /// 录制的频谱字节，每帧 `bins` 个。
    pub fn recorded_bytes(&self) -> Option<(usize, Vec<u8>)> {
        let b = self.bytes.as_ref()?;
        Some((b.bins, base64_decode(&b.base64)))
    }
}

/// 帧时间：`now_k = k·1000/hz`，跳过 `[start, end)` 停顿区间。与 `lib/trace.mjs` 相同。
pub fn frame_times(sample_count: usize, sample_rate: f64, hz: f64, skip: &[[f64; 2]]) -> Vec<f64> {
    let count = ((sample_count as f64 * hz) / sample_rate).floor() as usize;
    (0..count)
        .map(|k| (k as f64 * 1000.0) / hz)
        .filter(|now| !skip.iter().any(|[a, b]| *now >= *a && *now < *b))
        .collect()
}

/// 32 位 FNV-1a，与 `lib/dsp.mjs` 的 `fnv1a` 相同。
pub fn fnv1a(bytes: impl IntoIterator<Item = u8>) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in bytes {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

pub fn samples_hash(samples: &[f32]) -> String {
    format!(
        "{:08x}",
        fnv1a(samples.iter().flat_map(|s| s.to_le_bytes()))
    )
}

fn base64_decode(text: &str) -> Vec<u8> {
    let value = |c: u8| -> u32 {
        match c {
            b'A'..=b'Z' => u32::from(c - b'A'),
            b'a'..=b'z' => u32::from(c - b'a') + 26,
            b'0'..=b'9' => u32::from(c - b'0') + 52,
            b'+' => 62,
            b'/' => 63,
            _ => panic!("非法 base64 字符 {c}"),
        }
    };
    let clean: Vec<u8> = text.bytes().filter(|&c| c != b'=').collect();
    let mut out = Vec::with_capacity(clean.len() * 3 / 4);
    for chunk in clean.chunks(4) {
        let mut acc = 0_u32;
        for (i, &c) in chunk.iter().enumerate() {
            acc |= value(c) << (18 - 6 * i);
        }
        let bytes = acc.to_be_bytes();
        out.extend_from_slice(&bytes[1..chunk.len()]);
    }
    out
}
