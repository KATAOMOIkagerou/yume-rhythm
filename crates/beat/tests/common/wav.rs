//! 极简 WAV 读取，与 `tools/beat-baseline/lib/wav.mjs` 相同的下混规则：
//! `mono[i] = f32((ch0 + ch1 + ...) / channels)`，整数 PCM 按 `1 / 2^(bits-1)` 缩放。

use std::path::Path;

pub struct Wav {
    pub sample_rate: f64,
    pub channels: u32,
    pub samples: Vec<f32>,
}

fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

pub fn read_wav(path: &Path) -> Wav {
    let buf = std::fs::read(path).unwrap_or_else(|e| panic!("读取 {} 失败：{e}", path.display()));
    assert!(
        buf.len() >= 12 && &buf[0..4] == b"RIFF" && &buf[8..12] == b"WAVE",
        "{} 不是 RIFF/WAVE 文件",
        path.display()
    );
    let mut fmt = None;
    let mut data = None;
    let mut o = 12;
    while o + 8 <= buf.len() {
        let id = &buf[o..o + 4];
        let size = u32_at(&buf, o + 4) as usize;
        let body = o + 8;
        if id == b"fmt " {
            let mut format = u16_at(&buf, body);
            let channels = u16_at(&buf, body + 2);
            let sample_rate = u32_at(&buf, body + 4);
            let bits = u16_at(&buf, body + 14);
            if format == 0xfffe && size >= 40 {
                format = u16_at(&buf, body + 24);
            }
            fmt = Some((format, channels, sample_rate, bits));
        } else if id == b"data" {
            data = Some((body, size.min(buf.len() - body)));
        }
        o = body + size + (size & 1);
    }
    let (format, channels, sample_rate, bits) = fmt.expect("缺少 fmt 块");
    let (offset, size) = data.expect("缺少 data 块");
    let bytes_per = usize::from(bits / 8);
    let ch = usize::from(channels);
    let frames = size / (bytes_per * ch);
    let read = |p: usize| -> f64 {
        let b = &buf[p..p + bytes_per];
        match (format, bits) {
            (1, 16) => f64::from(i16::from_le_bytes([b[0], b[1]])) / 32768.0,
            (1, 24) => {
                let v = i32::from(b[0]) | (i32::from(b[1]) << 8) | (i32::from(b[2] as i8) << 16);
                f64::from(v) / 8388608.0
            }
            (1, 32) => f64::from(i32::from_le_bytes([b[0], b[1], b[2], b[3]])) / 2147483648.0,
            (3, 32) => f64::from(f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
            (3, 64) => f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]),
            _ => panic!("不支持的 WAV 格式 {format} / {bits} bit"),
        }
    };
    let samples = (0..frames)
        .map(|i| {
            let mut sum = 0.0;
            for c in 0..ch {
                sum += read(offset + (i * ch + c) * bytes_per);
            }
            (sum / f64::from(channels)) as f32
        })
        .collect();
    Wav {
        sample_rate: f64::from(sample_rate),
        channels: u32::from(channels),
        samples,
    }
}
