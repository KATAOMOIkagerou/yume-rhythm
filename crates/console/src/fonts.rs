//! 字体枚举与运行时加载（功能清单第 2 节）。
//!
//! 直接复用 Slint 内部的 `fontique::Collection`，确保列出的名称与界面实际可渲染字体一致，
//! 并且无需任何 `unsafe` 或额外第三方库。

use slint::fontique_012::{self, fontique};
use std::collections::BTreeSet;
use std::sync::Arc;

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

    let mut ordered = Vec::with_capacity(all.len());
    for &preferred in PREFERRED_CJK {
        if all.remove(preferred) {
            ordered.push(preferred.to_owned());
        }
    }
    ordered.extend(all);
    ordered
}

/// 将本地 `.ttf`/`.otf`/`.ttc` 字体字节注册到 Slint 共享字体集合，返回首个注册成功的家族名。
#[must_use]
pub fn register_font_bytes(bytes: Vec<u8>) -> Option<String> {
    if bytes.is_empty() {
        return None;
    }
    let mut collection = fontique_012::shared_collection();
    let registered = collection.register_fonts(fontique::Blob::new(Arc::new(bytes)), None);
    let (family_id, _) = registered.first()?;
    let name = collection.family_name(*family_id)?.to_owned();
    let family_ids: Vec<_> = registered.iter().map(|(id, _)| *id).collect();
    collection.append_fallbacks(
        fontique::FallbackKey::new(fontique::Script::from_bytes(*b"Hani"), None),
        family_ids.into_iter(),
    );
    Some(name)
}
