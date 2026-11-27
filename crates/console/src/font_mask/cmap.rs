//! OpenType / TrueType `cmap` 解析与 Format 12 序列化。

use std::cmp::{max, min};

/// Format 12 连续码位映射段：`cp in start_cp..=end_cp` 映射到 `start_gid + (cp - start_cp)`。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CmapGroup {
    pub start_cp: u32,
    pub end_cp: u32,
    pub start_gid: u32,
}

pub(super) fn extract_cmap_groups(cmap: &[u8]) -> Option<Vec<CmapGroup>> {
    let num_tables = usize::from(read_u16(cmap, 2)?);
    let recs_len = 4usize.checked_add(num_tables.checked_mul(8)?)?;
    if cmap.len() < recs_len {
        return None;
    }

    // 与 read-fonts / fontique 一致的 Unicode subtable 优先级。
    const PRIORITY: &[(u16, u16)] = &[
        (3, 10),
        (0, 4),
        (0, 3),
        (3, 1),
        (0, 6),
        (0, 2),
        (0, 1),
        (0, 0),
        (1, 0),
    ];

    for &(plat, enc) in PRIORITY {
        for i in 0..num_tables {
            let off = 4 + i * 8;
            let p = read_u16(cmap, off)?;
            let e = read_u16(cmap, off + 2)?;
            if p != plat || e != enc {
                continue;
            }
            let sub_off = usize::try_from(read_u32(cmap, off + 4)?).ok()?;
            let Some(sub) = cmap.get(sub_off..) else {
                continue;
            };
            if let Some(groups) = decode_subtable(sub) {
                return Some(groups);
            }
        }
    }
    None
}

fn decode_subtable(sub: &[u8]) -> Option<Vec<CmapGroup>> {
    let fmt = read_u16(sub, 0)?;
    let mut groups = Vec::new();
    match fmt {
        0 => {
            let map = sub.get(6..6 + 256)?;
            for (cp, &gid) in map.iter().enumerate() {
                if gid != 0 {
                    push_group(&mut groups, u32::try_from(cp).ok()?, u32::from(gid));
                }
            }
        }
        4 => decode_format4(sub, &mut groups)?,
        6 => {
            let first = read_u16(sub, 6)?;
            let count = read_u16(sub, 8)?;
            for i in 0..count {
                let gid = read_u16(sub, 10 + usize::from(i) * 2)?;
                if gid != 0 {
                    push_group(&mut groups, u32::from(first) + u32::from(i), u32::from(gid));
                }
            }
        }
        10 => {
            let start = read_u32(sub, 12)?;
            let count = read_u32(sub, 16)?;
            for i in 0..count {
                let off = 20usize.checked_add(usize::try_from(i).ok()?.checked_mul(2)?)?;
                let gid = read_u16(sub, off)?;
                if gid != 0
                    && let Some(cp) = start.checked_add(i)
                    && cp <= 0x10_FFFF
                {
                    push_group(&mut groups, cp, u32::from(gid));
                }
            }
        }
        12 => {
            let count = read_u32(sub, 12)?;
            let count_usize = usize::try_from(count).ok()?;
            let needed = 16usize.checked_add(count_usize.checked_mul(12)?)?;
            if sub.len() < needed {
                return None;
            }
            groups.reserve(count_usize);
            for i in 0..count_usize {
                let off = 16 + i * 12;
                let start_cp = read_u32(sub, off)?;
                let end_cp = min(read_u32(sub, off + 4)?, 0x10_FFFF);
                let start_gid = read_u32(sub, off + 8)?;
                if start_cp <= end_cp {
                    append_run(
                        &mut groups,
                        CmapGroup {
                            start_cp,
                            end_cp,
                            start_gid,
                        },
                    );
                }
            }
        }
        _ => return None,
    }
    Some(normalize_groups(groups))
}

fn decode_format4(sub: &[u8], groups: &mut Vec<CmapGroup>) -> Option<()> {
    let seg_count_x2 = usize::from(read_u16(sub, 6)?);
    if seg_count_x2 == 0 || seg_count_x2 % 2 != 0 {
        return None;
    }
    let seg_count = seg_count_x2 / 2;
    let end_off = 14usize;
    let start_off = end_off + seg_count_x2 + 2;
    let delta_off = start_off + seg_count_x2;
    let range_off = delta_off + seg_count_x2;

    for i in 0..seg_count {
        let start = read_u16(sub, start_off + i * 2)?;
        let end = read_u16(sub, end_off + i * 2)?;
        if start > end || (start == 0xFFFF && end == 0xFFFF) {
            continue;
        }
        let delta = read_u16(sub, delta_off + i * 2)?;
        let roff = read_u16(sub, range_off + i * 2)?;
        if roff == 0 {
            push_format4_linear(groups, start, end, delta);
        } else {
            for cp in start..=end {
                let idx_off = range_off + i * 2 + usize::from(roff) + usize::from(cp - start) * 2;
                let Some(raw_gid) = read_u16(sub, idx_off) else {
                    break;
                };
                if raw_gid != 0 {
                    let gid = u32::from(raw_gid.wrapping_add(delta));
                    if gid != 0 {
                        push_group(groups, u32::from(cp), gid);
                    }
                }
            }
        }
    }
    Some(())
}

fn push_format4_linear(groups: &mut Vec<CmapGroup>, start: u16, end: u16, delta: u16) {
    let zero_cp = 0u16.wrapping_sub(delta);
    if start < zero_cp {
        let left_end = min(end, zero_cp - 1);
        append_run(
            groups,
            CmapGroup {
                start_cp: u32::from(start),
                end_cp: u32::from(left_end),
                start_gid: u32::from(start.wrapping_add(delta)),
            },
        );
    }
    if zero_cp < u16::MAX && end > zero_cp {
        let right_start = max(start, zero_cp + 1);
        append_run(
            groups,
            CmapGroup {
                start_cp: u32::from(right_start),
                end_cp: u32::from(end),
                start_gid: u32::from(right_start.wrapping_add(delta)),
            },
        );
    }
}

fn push_group(groups: &mut Vec<CmapGroup>, cp: u32, gid: u32) {
    append_run(
        groups,
        CmapGroup {
            start_cp: cp,
            end_cp: cp,
            start_gid: gid,
        },
    );
}

pub(super) fn append_run(groups: &mut Vec<CmapGroup>, next: CmapGroup) {
    if let Some(last) = groups.last_mut()
        && last.end_cp.checked_add(1) == Some(next.start_cp)
    {
        let expected_gid = last.start_gid + (last.end_cp - last.start_cp) + 1;
        if expected_gid == next.start_gid {
            last.end_cp = next.end_cp;
            return;
        }
    }
    groups.push(next);
}

fn normalize_groups(mut groups: Vec<CmapGroup>) -> Vec<CmapGroup> {
    groups.sort_unstable_by_key(|g| (g.start_cp, g.end_cp));
    let mut out: Vec<CmapGroup> = Vec::with_capacity(groups.len());
    for mut g in groups {
        if let Some(prev) = out.last()
            && g.start_cp <= prev.end_cp
        {
            let Some(next_start) = prev.end_cp.checked_add(1) else {
                continue;
            };
            if next_start > g.end_cp {
                continue;
            }
            g.start_gid += next_start - g.start_cp;
            g.start_cp = next_start;
        }
        append_run(&mut out, g);
    }
    out
}

pub(super) fn encode_cmap_format12(groups: &[CmapGroup]) -> Vec<u8> {
    let num_groups = u32::try_from(groups.len()).unwrap_or(u32::MAX);
    let sub_len = 16u32.saturating_add(num_groups.saturating_mul(12));
    let mut out = Vec::with_capacity(20 + groups.len() * 12);

    // cmap header: version=0, numTables=2 ((0,4) + (3,10) 共享同一 Format 12 子表)
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&2u16.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&4u16.to_be_bytes());
    out.extend_from_slice(&20u32.to_be_bytes());
    out.extend_from_slice(&3u16.to_be_bytes());
    out.extend_from_slice(&10u16.to_be_bytes());
    out.extend_from_slice(&20u32.to_be_bytes());

    out.extend_from_slice(&12u16.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&sub_len.to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(&num_groups.to_be_bytes());
    for g in groups {
        out.extend_from_slice(&g.start_cp.to_be_bytes());
        out.extend_from_slice(&g.end_cp.to_be_bytes());
        out.extend_from_slice(&g.start_gid.to_be_bytes());
    }
    out
}

pub(super) fn read_u16(data: &[u8], offset: usize) -> Option<u16> {
    let b = data.get(offset..offset.checked_add(2)?)?;
    Some(u16::from_be_bytes([b[0], b[1]]))
}

pub(super) fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    let b = data.get(offset..offset.checked_add(4)?)?;
    Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}
