//! 字体枚举、关键词筛选、文种隔离掩码栈与运行时加载（功能清单第 2 节）。
//!
//! 直接复用 Slint 内部的 `fontique::Collection`，确保列出的名称与界面实际可渲染字体一致；
//! 并通过 `font_mask` 对选中的英文字体（仅保留非中日韩码位）与中文字体（仅保留中日韩码位）
//! 做 `cmap` 隔离，保证双语混排时即使所选字体包含对侧字形也绝不串用。

use crate::font_mask::{MaskOutcome, ScriptMask, mask_sfnt_face};
use slint::fontique_012::{self, fontique};
use std::collections::BTreeSet;
use std::sync::Arc;
use yume_core::model::settings::{DEFAULT_EN_FONT_FAMILY, DEFAULT_ZH_FONT_FAMILY};

/// 由本模块生成并注册进 fontique 的掩码字体家族名前缀，枚举系统字体时须跳过。
pub const MASKED_FAMILY_PREFIX: &str = "__yume_mask__";

/// 优先置顶显示的常见中文界面字体。
const PREFERRED_CJK: &[&str] = &[
    "Microsoft YaHei UI",
    "Microsoft YaHei",
    "DengXian",
    "SimHei",
    "SimSun",
    "KaiTi",
    "FangSong",
    "Noto Sans SC",
    "Noto Sans CJK SC",
    "Source Han Sans SC",
    "Source Han Sans CN",
    "LXGW WenKai",
    "PingFang SC",
    "HarmonyOS Sans SC",
    "MiSans",
];

/// 优先置顶显示的常见英文/西文字体。
const PREFERRED_LATIN: &[&str] = &[
    "Segoe UI",
    "Inter",
    "Arial",
    "Calibri",
    "Cambria",
    "Consolas",
    "Cascadia Code",
    "Cascadia Mono",
    "Times New Roman",
    "Georgia",
    "Verdana",
    "Tahoma",
    "Trebuchet MS",
    "Courier New",
    "JetBrains Mono",
    "Fira Code",
];

/// 常见中英文字体别名映射，让搜索「微软雅黑 / yahei / 黑体 / simhei」都能双向命中。
const FONT_ALIASES: &[(&str, &str)] = &[
    ("微软雅黑", "microsoft yahei"),
    ("新宋体", "nsimsun"),
    ("宋体", "simsun"),
    ("黑体", "simhei"),
    ("楷体", "kaiti"),
    ("仿宋", "fangsong"),
    ("等线", "dengxian"),
    ("思源黑体", "noto sans cjk source han sans"),
    ("思源宋体", "noto serif cjk source han serif"),
    ("霞鹜文楷", "lxgw wenkai"),
    ("苹方", "pingfang"),
    ("方正", "fz fangzheng"),
    ("微软正黑", "microsoft jhenghei"),
    ("细明体", "mingliu"),
];

/// 枚举当前 Slint 字体集合中可用的系统字体家族名，常用中文字体排在前面。
#[must_use]
pub fn list_system_fonts() -> Vec<String> {
    let mut collection = fontique_012::shared_collection();
    let mut all = BTreeSet::new();
    for name in collection.family_names() {
        if !name.is_empty() && !name.starts_with('@') && !name.starts_with(MASKED_FAMILY_PREFIX) {
            all.insert(name.to_owned());
        }
    }
    order_with_preferred(all.into_iter().collect(), PREFERRED_CJK)
}

/// 按常用中文字体置顶重排字体列表。
#[must_use]
pub fn order_zh_fonts(fonts: &[String]) -> Vec<String> {
    order_with_preferred(fonts.to_vec(), PREFERRED_CJK)
}

/// 按常用英文字体置顶重排字体列表。
#[must_use]
pub fn order_en_fonts(fonts: &[String]) -> Vec<String> {
    order_with_preferred(fonts.to_vec(), PREFERRED_LATIN)
}

/// 按搜索关键词筛选字体列表，返回命中的原列表下标（忽略大小写，支持空格分词与中英文常用别名）。
#[must_use]
pub fn filter_font_options(labels: &[String], query: &str) -> Vec<usize> {
    let tokens: Vec<String> = query
        .split_whitespace()
        .map(str::to_lowercase)
        .filter(|t| !t.is_empty())
        .collect();
    if tokens.is_empty() {
        return (0..labels.len()).collect();
    }

    labels
        .iter()
        .enumerate()
        .filter(|(_, label)| {
            let haystack = expand_search_aliases(label);
            tokens.iter().all(|tok| haystack.contains(tok))
        })
        .map(|(idx, _)| idx)
        .collect()
}

fn expand_search_aliases(label: &str) -> String {
    let mut lower = label.to_lowercase();
    for &(zh_alias, en_alias) in FONT_ALIASES {
        if lower.contains(zh_alias) {
            lower.push(' ');
            lower.push_str(en_alias);
        }
        if en_alias.split_whitespace().any(|word| lower.contains(word)) {
            lower.push(' ');
            lower.push_str(zh_alias);
        }
    }
    lower
}

fn order_with_preferred(fonts: Vec<String>, preferred: &[&str]) -> Vec<String> {
    let mut remaining: BTreeSet<String> = fonts.into_iter().collect();
    let mut ordered = Vec::with_capacity(remaining.len());
    for &name in preferred {
        if remaining.remove(name) {
            ordered.push(name.to_owned());
        }
    }
    ordered.extend(remaining);
    ordered
}

/// 已注册的自定义字体信息：家族名与是否包含中文字符（用于判断默认填入中文还是英文字体）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisteredFont {
    pub family: String,
    pub has_cjk: bool,
}

/// 将本地 `.ttf`/`.otf`/`.ttc` 字体字节注册到 Slint 共享字体集合。
#[must_use]
pub fn register_font_bytes(bytes: Vec<u8>) -> Option<RegisteredFont> {
    if bytes.is_empty() {
        return None;
    }
    let shared = Arc::new(bytes);
    let mut collection = fontique_012::shared_collection();
    let registered = collection.register_fonts(fontique::Blob::new(shared.clone()), None);
    let (family_id, fonts) = registered.first()?;
    let family = collection.family_name(*family_id)?.to_owned();
    let has_cjk = fonts.iter().any(|info| {
        info.charmap_index()
            .charmap(shared.as_slice())
            .and_then(|cm| cm.map(u32::from('中')))
            .is_some_and(|gid| gid != 0)
    });
    Some(RegisteredFont { family, has_cjk })
}

/// 将「英文隔离优先 + 中文隔离兜底」的回落链写入 Slint 共享字体集合，
/// 返回写入 `Theme.font-family` 的标识名（每次组合不同即可触发 Slint 重新排版）。
#[must_use]
pub fn apply_font_stack(en: Option<&str>, zh: Option<&str>) -> String {
    let mut collection = fontique_012::shared_collection();
    apply_font_stack_in(&mut collection, en, zh)
}

fn apply_font_stack_in(
    collection: &mut fontique::Collection,
    en: Option<&str>,
    zh: Option<&str>,
) -> String {
    let existing_sys: Vec<_> = collection
        .generic_families(fontique::GenericFamily::SystemUi)
        .collect();
    let existing_sans: Vec<_> = collection
        .generic_families(fontique::GenericFamily::SansSerif)
        .collect();

    let mut chain = Vec::new();
    let push_masked = |col: &mut fontique::Collection,
                       target: &mut Vec<fontique::FamilyId>,
                       name: &str,
                       mask: ScriptMask| {
        if let Some(id) = ensure_masked_family(col, name, mask)
            && !target.contains(&id)
        {
            target.push(id);
        }
    };

    // 1. 所选英文字体（仅保留非 CJK 码位），再兜底默认英文字体（同样隔离非 CJK）。
    if let Some(en_name) = en {
        push_masked(collection, &mut chain, en_name, ScriptMask::NonCjk);
    }
    for fallback in [
        DEFAULT_EN_FONT_FAMILY,
        "DejaVu Sans",
        "Liberation Sans",
        "Noto Sans",
    ] {
        if collection.family_id(fallback).is_some() {
            push_masked(collection, &mut chain, fallback, ScriptMask::NonCjk);
            break;
        }
    }

    // 2. 所选中文字体（仅保留 CJK 码位），再兜底默认中文字体（同样隔离 CJK）。
    let zh_target = zh.unwrap_or(DEFAULT_ZH_FONT_FAMILY);
    push_masked(collection, &mut chain, zh_target, ScriptMask::Cjk);
    for fallback in [
        DEFAULT_ZH_FONT_FAMILY,
        "Noto Sans CJK SC",
        "Noto Sans SC",
        "WenQuanYi Zen Hei",
    ] {
        push_masked(collection, &mut chain, fallback, ScriptMask::Cjk);
    }

    // 3. 保留系统其余后备字体（如 emoji 与生僻符号），跳过旧的合成掩码家族。
    for id in existing_sans.into_iter().chain(existing_sys) {
        if !chain.contains(&id) && !is_masked_family(collection, id) {
            chain.push(id);
        }
    }

    if !chain.is_empty() {
        for generic in [
            fontique::GenericFamily::SystemUi,
            fontique::GenericFamily::SansSerif,
            fontique::GenericFamily::UiSansSerif,
        ] {
            collection.set_generic_families(generic, chain.iter().copied());
        }
    }

    // 同时确保 Han 文字脚本按用户选择的中文字体优先（同样使用 CJK 隔离家族）。
    let hani_key = fontique::FallbackKey::new(fontique::Script::from_bytes(*b"Hani"), None);
    let existing_hani: Vec<_> = collection.fallback_families(hani_key).collect();
    let mut next_hani = Vec::new();
    push_masked(collection, &mut next_hani, zh_target, ScriptMask::Cjk);
    for fallback in [
        DEFAULT_ZH_FONT_FAMILY,
        "Noto Sans CJK SC",
        "Noto Sans SC",
        "WenQuanYi Zen Hei",
    ] {
        push_masked(collection, &mut next_hani, fallback, ScriptMask::Cjk);
    }
    for id in existing_hani {
        if !next_hani.contains(&id) && !is_masked_family(collection, id) {
            next_hani.push(id);
        }
    }
    if !next_hani.is_empty() {
        collection.set_fallbacks(hani_key, next_hani.into_iter());
    }

    format!(
        "__yume_stack__{}__{}",
        en.unwrap_or("default"),
        zh.unwrap_or("default")
    )
}

fn is_masked_family(collection: &mut fontique::Collection, id: fontique::FamilyId) -> bool {
    collection
        .family_name(id)
        .is_some_and(|n| n.starts_with(MASKED_FAMILY_PREFIX))
}

fn ensure_masked_family(
    collection: &mut fontique::Collection,
    source_name: &str,
    mask: ScriptMask,
) -> Option<fontique::FamilyId> {
    let src_id = collection.family_id(source_name)?;
    let tag = match mask {
        ScriptMask::NonCjk => "noncjk",
        ScriptMask::Cjk => "cjk",
    };
    let masked_name = format!("{MASKED_FAMILY_PREFIX}{tag}__{source_name}");
    if let Some(existing) = collection.family_id(&masked_name) {
        return Some(existing);
    }
    let Some(family) = collection.family(src_id) else {
        return Some(src_id);
    };

    let mut plans = Vec::with_capacity(family.fonts().len());
    for face in family.fonts() {
        let Some(blob) = face.load(None) else {
            continue;
        };
        let Some(outcome) = mask_sfnt_face(blob.data(), face.index(), mask) else {
            continue;
        };
        plans.push((outcome, face.width(), face.style(), face.weight()));
    }

    if plans.is_empty()
        || plans
            .iter()
            .all(|(outcome, _, _, _)| matches!(outcome, MaskOutcome::Unchanged))
    {
        return Some(src_id);
    }

    let mut registered_any = false;
    for (outcome, width, style, weight) in plans {
        if let MaskOutcome::Masked(bytes) = outcome {
            let added = collection.register_fonts(
                fontique::Blob::new(Arc::new(bytes)),
                Some(fontique::FontInfoOverride {
                    family_name: Some(&masked_name),
                    width: Some(width),
                    style: Some(style),
                    weight: Some(weight),
                    axes: None,
                }),
            );
            if !added.is_empty() {
                registered_any = true;
            }
        }
    }

    if registered_any {
        collection.family_id(&masked_name).or(Some(src_id))
    } else {
        Some(src_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_dual_script_ttf(latin_gid: u16, cjk_gid: u16) -> Vec<u8> {
        let mut head = vec![0u8; 54];
        head[0..4].copy_from_slice(&0x0001_0000u32.to_be_bytes());
        head[12..16].copy_from_slice(&0x5F0F_3CF5u32.to_be_bytes());
        head[18..20].copy_from_slice(&1000u16.to_be_bytes());

        // Format 12 cmap: 'A' (0x41) -> latin_gid, '中' (0x4E2D) -> cjk_gid
        let mut cmap = Vec::new();
        cmap.extend_from_slice(&0u16.to_be_bytes());
        cmap.extend_from_slice(&1u16.to_be_bytes());
        cmap.extend_from_slice(&3u16.to_be_bytes());
        cmap.extend_from_slice(&10u16.to_be_bytes());
        cmap.extend_from_slice(&12u32.to_be_bytes());
        cmap.extend_from_slice(&12u16.to_be_bytes());
        cmap.extend_from_slice(&0u16.to_be_bytes());
        cmap.extend_from_slice(&(16u32 + 24).to_be_bytes());
        cmap.extend_from_slice(&0u32.to_be_bytes());
        cmap.extend_from_slice(&2u32.to_be_bytes());
        for (cp, gid) in [
            (0x41u32, u32::from(latin_gid)),
            (0x4E2Du32, u32::from(cjk_gid)),
        ] {
            cmap.extend_from_slice(&cp.to_be_bytes());
            cmap.extend_from_slice(&cp.to_be_bytes());
            cmap.extend_from_slice(&gid.to_be_bytes());
        }

        let name = vec![0u8; 6];
        let tables = [
            (*b"cmap", cmap.as_slice()),
            (*b"head", head.as_slice()),
            (*b"name", name.as_slice()),
        ];
        let num_tables: u16 = 3;
        let header_len = 12 + 3 * 16;
        let mut out = Vec::new();
        out.extend_from_slice(b"\x00\x01\x00\x00");
        out.extend_from_slice(&num_tables.to_be_bytes());
        out.extend_from_slice(&32u16.to_be_bytes());
        out.extend_from_slice(&1u16.to_be_bytes());
        out.extend_from_slice(&16u16.to_be_bytes());
        out.resize(header_len, 0);
        for (idx, (tag, bytes)) in tables.iter().enumerate() {
            let off = u32::try_from(out.len()).unwrap_or(0);
            let len = u32::try_from(bytes.len()).unwrap_or(0);
            out.extend_from_slice(bytes);
            while out.len() % 4 != 0 {
                out.push(0);
            }
            let rec = 12 + idx * 16;
            out[rec..rec + 4].copy_from_slice(tag);
            out[rec + 8..rec + 12].copy_from_slice(&off.to_be_bytes());
            out[rec + 12..rec + 16].copy_from_slice(&len.to_be_bytes());
        }
        out
    }

    fn resolve_first_match(
        col: &mut fontique::Collection,
        chain: &[fontique::FamilyId],
        ch: char,
    ) -> Option<(String, u32)> {
        for &fid in chain {
            let name = col.family_name(fid)?.to_owned();
            let fam = col.family(fid)?;
            for face in fam.fonts() {
                let blob = face.load(None)?;
                if let Some(cm) = face.charmap_index().charmap(blob.data())
                    && let Some(gid) = cm.map(ch)
                    && gid != 0
                {
                    return Some((name, gid));
                }
            }
        }
        None
    }

    #[test]
    fn zh_and_en_font_ordering_prioritize_matching_scripts() {
        let input = vec![
            "Zed Font".to_owned(),
            "Segoe UI".to_owned(),
            "KaiTi".to_owned(),
            "Consolas".to_owned(),
            "Microsoft YaHei UI".to_owned(),
        ];
        let zh = order_zh_fonts(&input);
        assert_eq!(zh[0], "Microsoft YaHei UI");
        assert_eq!(zh[1], "KaiTi");

        let en = order_en_fonts(&input);
        assert_eq!(en[0], "Segoe UI");
        assert_eq!(en[1], "Consolas");
    }

    #[test]
    fn filter_font_options_matches_substrings_and_aliases() {
        let options = vec![
            "默认字体（微软雅黑 UI）".to_owned(),
            "Microsoft YaHei UI".to_owned(),
            "KaiTi".to_owned(),
            "Consolas".to_owned(),
            "Noto Sans CJK SC".to_owned(),
        ];
        assert_eq!(filter_font_options(&options, ""), vec![0, 1, 2, 3, 4]);
        assert_eq!(filter_font_options(&options, "con"), vec![3]);
        assert_eq!(filter_font_options(&options, "yahei"), vec![0, 1]);
        assert_eq!(filter_font_options(&options, "微软"), vec![0, 1]);
        assert_eq!(filter_font_options(&options, "楷体"), vec![2]);
        assert_eq!(filter_font_options(&options, "noto sc"), vec![4]);
        assert!(filter_font_options(&options, "不存在的字体").is_empty());
    }

    #[test]
    fn apply_font_stack_isolates_dual_script_fonts() {
        let mut col = fontique::Collection::new(fontique::CollectionOptions {
            shared: false,
            system_fonts: false,
        });
        col.register_fonts(
            fontique::Blob::new(Arc::new(build_dual_script_ttf(10, 11))),
            Some(fontique::FontInfoOverride {
                family_name: Some("DualEn"),
                ..Default::default()
            }),
        );
        col.register_fonts(
            fontique::Blob::new(Arc::new(build_dual_script_ttf(20, 21))),
            Some(fontique::FontInfoOverride {
                family_name: Some("DualZh"),
                ..Default::default()
            }),
        );

        let sentinel = apply_font_stack_in(&mut col, Some("DualEn"), Some("DualZh"));
        assert!(sentinel.contains("DualEn") && sentinel.contains("DualZh"));

        let chain: Vec<_> = col
            .generic_families(fontique::GenericFamily::SystemUi)
            .collect();
        let a_hit = resolve_first_match(&mut col, &chain, 'A');
        let zh_hit = resolve_first_match(&mut col, &chain, '中');
        assert_eq!(a_hit, Some(("__yume_mask__noncjk__DualEn".to_owned(), 10)));
        assert_eq!(zh_hit, Some(("__yume_mask__cjk__DualZh".to_owned(), 21)));
    }
}
