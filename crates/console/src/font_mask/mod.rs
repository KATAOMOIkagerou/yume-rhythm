//! OpenType / TrueType `cmap` 文种掩码工具。
//!
//! 当用户为「英文字体」选了同样包含汉字的字体，或为「中文字体」选了同样包含拉丁字母的字体时，
//! 仅靠改变家族回落顺序仍会导致单一字体通吃中英混排。本模块通过重写单面 SFNT 的 `cmap`
//! （Format 12），把英文字体裁剪为仅映射非中日韩码位、把中文字体裁剪为仅映射中日韩码位，
//! 让两种文种在字形查找层面彻底隔离，互不串用。

mod cmap;
#[cfg(test)]
mod tests;

use cmap::{CmapGroup, append_run, encode_cmap_format12, extract_cmap_groups, read_u16, read_u32};
use std::cmp::{max, min};

/// 文种掩码模式。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScriptMask {
    /// 仅保留非中日韩码位（拉丁字母、数字、西文标点、符号等）。
    NonCjk,
    /// 仅保留中日韩码位（汉字、假名、谚文、全角/CJK 标点与扩展区）。
    Cjk,
}

/// 掩码生成结果。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MaskOutcome {
    /// 原字体已严格满足掩码，无需复制与重写字节。
    Unchanged,
    /// 已生成裁剪后的独立单面 SFNT 字体字节。
    Masked(Vec<u8>),
}

/// 闭区间 `[start, end]` 形式的中日韩及相关全角/假名/谚文码位表（已按升序互不重叠排列）。
const CJK_RANGES: &[(u32, u32)] = &[
    (0x1100, 0x11FF),   // Hangul Jamo
    (0x2E80, 0x2EFF),   // CJK Radicals Supplement
    (0x2F00, 0x2FDF),   // Kangxi Radicals
    (0x2FF0, 0x2FFF),   // Ideographic Description Characters
    (0x3000, 0x303F),   // CJK Symbols and Punctuation
    (0x3040, 0x30FF),   // Hiragana & Katakana
    (0x3100, 0x318F),   // Bopomofo & Hangul Compatibility Jamo
    (0x3190, 0x31FF),   // Kanbun / Bopomofo Ext / CJK Strokes / Katakana Ext
    (0x3200, 0x33FF),   // Enclosed CJK & CJK Compatibility
    (0x3400, 0x4DBF),   // CJK Unified Ideographs Extension A
    (0x4E00, 0x9FFF),   // CJK Unified Ideographs
    (0xA960, 0xA97F),   // Hangul Jamo Extended-A
    (0xAC00, 0xD7FF),   // Hangul Syllables & Extended-B
    (0xF900, 0xFAFF),   // CJK Compatibility Ideographs
    (0xFE10, 0xFE1F),   // Vertical Forms
    (0xFE30, 0xFE4F),   // CJK Compatibility Forms
    (0xFF00, 0xFFEF),   // Halfwidth and Fullwidth Forms
    (0x1B000, 0x1B2FF), // Kana Supplement / Extended / Nushu
    (0x20000, 0x323AF), // CJK Unified Ideographs Extensions B..I
];

/// 判断 Unicode 码位是否属于中日韩（汉字、假名、谚文及全角/CJK 标点）。
#[must_use]
pub fn is_cjk_codepoint(cp: u32) -> bool {
    CJK_RANGES
        .iter()
        .any(|&(start, end)| cp >= start && cp <= end)
}

/// 从字体文件或 `.ttc` 字体合集提取指定面，并按 `mask` 裁剪 `cmap` 映射表。
#[must_use]
pub fn mask_sfnt_face(data: &[u8], face_index: u32, mask: ScriptMask) -> Option<MaskOutcome> {
    let sfnt_offset = resolve_sfnt_offset(data, face_index)?;
    let (sfnt_version, tables) = parse_sfnt_tables(data, sfnt_offset)?;
    let cmap_bytes = tables
        .iter()
        .find(|(tag, _)| tag == b"cmap")
        .map(|(_, slice)| *slice)?;

    let groups = extract_cmap_groups(cmap_bytes)?;
    match mask {
        ScriptMask::NonCjk if !any_overlaps_cjk(&groups) => return Some(MaskOutcome::Unchanged),
        ScriptMask::Cjk if !any_outside_cjk(&groups) => return Some(MaskOutcome::Unchanged),
        _ => {}
    }

    let masked_groups = filter_groups(&groups, mask);
    let new_cmap = encode_cmap_format12(&masked_groups);
    let rebuilt = rebuild_sfnt(sfnt_version, &tables, &new_cmap)?;
    Some(MaskOutcome::Masked(rebuilt))
}

fn resolve_sfnt_offset(data: &[u8], face_index: u32) -> Option<usize> {
    if data.get(..4) == Some(b"ttcf") {
        let num_fonts = read_u32(data, 8)?;
        if face_index >= num_fonts {
            return None;
        }
        let table_idx = 12usize.checked_add(usize::try_from(face_index).ok()?.checked_mul(4)?)?;
        let off = read_u32(data, table_idx)?;
        usize::try_from(off).ok()
    } else if face_index == 0 {
        Some(0)
    } else {
        None
    }
}

type TableSlice<'a> = ([u8; 4], &'a [u8]);

fn parse_sfnt_tables(data: &[u8], offset: usize) -> Option<([u8; 4], Vec<TableSlice<'_>>)> {
    let hdr = data.get(offset..offset.checked_add(12)?)?;
    let sfnt_version: [u8; 4] = hdr.get(..4)?.try_into().ok()?;
    let valid_magic = matches!(
        &sfnt_version,
        b"\x00\x01\x00\x00" | b"OTTO" | b"true" | b"typ1"
    );
    if !valid_magic {
        return None;
    }
    let num_tables = usize::from(read_u16(hdr, 4)?);
    let dir_start = offset.checked_add(12)?;
    let dir_len = num_tables.checked_mul(16)?;
    let dir = data.get(dir_start..dir_start.checked_add(dir_len)?)?;

    let mut tables = Vec::with_capacity(num_tables);
    for i in 0..num_tables {
        let rec_off = i * 16;
        let tag: [u8; 4] = dir.get(rec_off..rec_off + 4)?.try_into().ok()?;
        let t_off = usize::try_from(read_u32(dir, rec_off + 8)?).ok()?;
        let t_len = usize::try_from(read_u32(dir, rec_off + 12)?).ok()?;
        let slice = data.get(t_off..t_off.checked_add(t_len)?)?;
        tables.push((tag, slice));
    }
    Some((sfnt_version, tables))
}

fn any_overlaps_cjk(groups: &[CmapGroup]) -> bool {
    let mut r_idx = 0;
    for g in groups {
        while r_idx < CJK_RANGES.len() && CJK_RANGES[r_idx].1 < g.start_cp {
            r_idx += 1;
        }
        if r_idx < CJK_RANGES.len() && CJK_RANGES[r_idx].0 <= g.end_cp {
            return true;
        }
    }
    false
}

fn any_outside_cjk(groups: &[CmapGroup]) -> bool {
    let mut r_idx = 0;
    for g in groups {
        let mut cursor = g.start_cp;
        while r_idx < CJK_RANGES.len() && CJK_RANGES[r_idx].1 < cursor {
            r_idx += 1;
        }
        while cursor <= g.end_cp && r_idx < CJK_RANGES.len() {
            let (rs, re) = CJK_RANGES[r_idx];
            if cursor < rs {
                return true;
            }
            if g.end_cp <= re {
                cursor = g.end_cp.saturating_add(1);
                break;
            }
            cursor = re + 1;
            r_idx += 1;
        }
        if cursor <= g.end_cp {
            return true;
        }
    }
    false
}

fn filter_groups(groups: &[CmapGroup], mask: ScriptMask) -> Vec<CmapGroup> {
    let mut out = Vec::new();
    match mask {
        ScriptMask::Cjk => {
            for &g in groups {
                for &(rs, re) in CJK_RANGES {
                    if re < g.start_cp {
                        continue;
                    }
                    if rs > g.end_cp {
                        break;
                    }
                    let s = max(g.start_cp, rs);
                    let e = min(g.end_cp, re);
                    append_run(
                        &mut out,
                        CmapGroup {
                            start_cp: s,
                            end_cp: e,
                            start_gid: g.start_gid + (s - g.start_cp),
                        },
                    );
                }
            }
        }
        ScriptMask::NonCjk => {
            for &g in groups {
                let mut cursor = g.start_cp;
                for &(rs, re) in CJK_RANGES {
                    if re < cursor {
                        continue;
                    }
                    if rs > g.end_cp {
                        break;
                    }
                    if cursor < rs {
                        let e = rs - 1;
                        append_run(
                            &mut out,
                            CmapGroup {
                                start_cp: cursor,
                                end_cp: e,
                                start_gid: g.start_gid + (cursor - g.start_cp),
                            },
                        );
                    }
                    let Some(next) = re.checked_add(1) else {
                        cursor = u32::MAX;
                        break;
                    };
                    cursor = max(cursor, next);
                    if cursor > g.end_cp {
                        break;
                    }
                }
                if cursor <= g.end_cp {
                    append_run(
                        &mut out,
                        CmapGroup {
                            start_cp: cursor,
                            end_cp: g.end_cp,
                            start_gid: g.start_gid + (cursor - g.start_cp),
                        },
                    );
                }
            }
        }
    }
    out
}

fn rebuild_sfnt(
    sfnt_version: [u8; 4],
    orig: &[TableSlice<'_>],
    new_cmap: &[u8],
) -> Option<Vec<u8>> {
    let mut owned_head: Option<Vec<u8>> = None;
    let mut entries: Vec<([u8; 4], &[u8])> = Vec::with_capacity(orig.len());
    for &(tag, slice) in orig {
        if &tag == b"cmap" {
            entries.push((tag, new_cmap));
        } else if &tag == b"head" {
            let mut h = slice.to_vec();
            if let Some(adj) = h.get_mut(8..12) {
                adj.copy_from_slice(&0u32.to_be_bytes());
            }
            owned_head = Some(h);
            entries.push((tag, &[]));
        } else {
            entries.push((tag, slice));
        }
    }
    if let Some(head_bytes) = owned_head.as_deref()
        && let Some((_, slot)) = entries.iter_mut().find(|(tag, _)| tag == b"head")
    {
        *slot = head_bytes;
    }
    entries.sort_unstable_by_key(|(tag, _)| *tag);

    let num_tables = u16::try_from(entries.len()).ok()?;
    if num_tables == 0 {
        return None;
    }
    let max_pow2 = 1u16 << (15 - num_tables.leading_zeros());
    let search_range = max_pow2 * 16;
    let entry_selector = u16::try_from(max_pow2.trailing_zeros()).ok()?;
    let range_shift = num_tables * 16 - search_range;

    let header_len = 12 + entries.len() * 16;
    let total_payload: usize = entries.iter().map(|(_, s)| (s.len() + 3) & !3).sum();
    let mut out = Vec::with_capacity(header_len + total_payload);
    out.extend_from_slice(&sfnt_version);
    out.extend_from_slice(&num_tables.to_be_bytes());
    out.extend_from_slice(&search_range.to_be_bytes());
    out.extend_from_slice(&entry_selector.to_be_bytes());
    out.extend_from_slice(&range_shift.to_be_bytes());
    out.resize(header_len, 0);

    let mut head_file_offset: Option<usize> = None;
    for (idx, &(tag, bytes)) in entries.iter().enumerate() {
        let off = out.len();
        if &tag == b"head" {
            head_file_offset = Some(off);
        }
        out.extend_from_slice(bytes);
        while out.len() % 4 != 0 {
            out.push(0);
        }
        let csum = calc_table_checksum(bytes);
        let rec = 12 + idx * 16;
        out[rec..rec + 4].copy_from_slice(&tag);
        out[rec + 4..rec + 8].copy_from_slice(&csum.to_be_bytes());
        out[rec + 8..rec + 12].copy_from_slice(&u32::try_from(off).ok()?.to_be_bytes());
        out[rec + 12..rec + 16].copy_from_slice(&u32::try_from(bytes.len()).ok()?.to_be_bytes());
    }

    if let Some(hoff) = head_file_offset
        && hoff + 12 <= out.len()
    {
        let whole = calc_table_checksum(&out);
        let adjustment = 0xB1B0_AFBAu32.wrapping_sub(whole);
        out[hoff + 8..hoff + 12].copy_from_slice(&adjustment.to_be_bytes());
    }
    Some(out)
}

fn calc_table_checksum(bytes: &[u8]) -> u32 {
    let mut sum = 0u32;
    let (chunks, rem) = bytes.as_chunks::<4>();
    for c in chunks {
        let word = u32::from_be_bytes(*c);
        sum = sum.wrapping_add(word);
    }
    if !rem.is_empty() {
        let mut last = [0u8; 4];
        last[..rem.len()].copy_from_slice(rem);
        sum = sum.wrapping_add(u32::from_be_bytes(last));
    }
    sum
}
