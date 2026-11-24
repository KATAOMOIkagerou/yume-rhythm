//! 字体枚举、优先级栈配置与运行时加载（功能清单第 2 节）。
//!
//! 直接复用 Slint 内部的 `fontique::Collection`，确保列出的名称与界面实际可渲染字体一致，
//! 并通过 `GenericFamily::SansSerif` 保证「英文字体优先、中文字体兜底」的双语查找顺序。

use slint::fontique_012::{self, fontique};
use std::collections::BTreeSet;
use std::sync::Arc;
use yume_core::model::settings::{DEFAULT_EN_FONT_FAMILY, DEFAULT_ZH_FONT_FAMILY};

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

/// 枚举当前 Slint 字体集合中可用的系统字体家族名，常用中文字体排在前面。
#[must_use]
pub fn list_system_fonts() -> Vec<String> {
    let mut collection = fontique_012::shared_collection();
    let mut all = BTreeSet::new();
    for name in collection.family_names() {
        if !name.is_empty() && !name.starts_with('@') {
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
    let family_ids: Vec<_> = registered.iter().map(|(id, _)| *id).collect();
    if has_cjk {
        collection.append_fallbacks(
            fontique::FallbackKey::new(fontique::Script::from_bytes(*b"Hani"), None),
            family_ids.into_iter(),
        );
    }
    Some(RegisteredFont { family, has_cjk })
}

/// 将「英文字体优先、中文字体兜底」的回落链写入 Slint 共享字体集合，
/// 返回写入 `Theme.font-family` 的标识名（每次组合不同即可触发 Slint 重新排版）。
#[must_use]
pub fn apply_font_stack(en: Option<&str>, zh: Option<&str>) -> String {
    let mut collection = fontique_012::shared_collection();
    let existing_sys: Vec<_> = collection
        .generic_families(fontique::GenericFamily::SystemUi)
        .collect();
    let existing_sans: Vec<_> = collection
        .generic_families(fontique::GenericFamily::SansSerif)
        .collect();

    let mut chain = Vec::new();
    let push_name = |collection: &mut fontique::Collection,
                     target: &mut Vec<fontique::FamilyId>,
                     name: &str| {
        if let Some(id) = collection.family_id(name)
            && !target.contains(&id)
        {
            target.push(id);
        }
    };

    // 第一优先级：英文字体（若为默认则依次尝试 Segoe UI / 常见无衬线字体）。
    if let Some(en_name) = en {
        push_name(&mut collection, &mut chain, en_name);
    } else {
        for fallback in [
            DEFAULT_EN_FONT_FAMILY,
            "DejaVu Sans",
            "Liberation Sans",
            "Noto Sans",
        ] {
            if let Some(id) = collection.family_id(fallback) {
                if !chain.contains(&id) {
                    chain.push(id);
                }
                break;
            }
        }
    }

    // 第二优先级：中文字体（若为默认则尝试微软雅黑 UI / Noto Sans CJK SC）。
    let zh_target = zh.unwrap_or(DEFAULT_ZH_FONT_FAMILY);
    push_name(&mut collection, &mut chain, zh_target);
    push_name(&mut collection, &mut chain, DEFAULT_ZH_FONT_FAMILY);
    push_name(&mut collection, &mut chain, "Noto Sans CJK SC");
    push_name(&mut collection, &mut chain, "Noto Sans SC");

    for id in existing_sans.into_iter().chain(existing_sys) {
        if !chain.contains(&id) {
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

    // 同时确保 Han 文字脚本按用户选择的中文字体优先。
    let hani_key = fontique::FallbackKey::new(fontique::Script::from_bytes(*b"Hani"), None);
    let existing_hani: Vec<_> = collection.fallback_families(hani_key).collect();
    let mut next_hani = Vec::new();
    push_name(&mut collection, &mut next_hani, zh_target);
    for id in existing_hani {
        if !next_hani.contains(&id) {
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn apply_font_stack_puts_english_before_chinese() {
        let families = list_system_fonts();
        if families.len() < 2 {
            return;
        }
        let en = families[0].as_str();
        let zh = families[1].as_str();
        let sentinel = apply_font_stack(Some(en), Some(zh));
        assert!(sentinel.contains(en));
        assert!(sentinel.contains(zh));

        let mut collection = fontique_012::shared_collection();
        let en_id = collection.family_id(en);
        let zh_id = collection.family_id(zh);
        let chain: Vec<_> = collection
            .generic_families(fontique::GenericFamily::SystemUi)
            .collect();
        if let (Some(e), Some(z)) = (en_id, zh_id) {
            let e_pos = chain.iter().position(|id| *id == e);
            let z_pos = chain.iter().position(|id| *id == z);
            assert!(e_pos.is_some() && z_pos.is_some() && e_pos < z_pos);
        }
    }
}
