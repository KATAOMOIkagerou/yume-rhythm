use super::*;
use slint::fontique_012::fontique;
use std::sync::Arc;

fn build_synthetic_sfnt(cmap_table: Vec<u8>) -> Vec<u8> {
    let mut head = vec![0u8; 54];
    head[0..4].copy_from_slice(&0x0001_0000u32.to_be_bytes());
    head[12..16].copy_from_slice(&0x5F0F_3CF5u32.to_be_bytes());
    head[18..20].copy_from_slice(&1000u16.to_be_bytes());
    let name = vec![0u8; 6];
    let orig = [
        (*b"head", head.as_slice()),
        (*b"name", name.as_slice()),
        (*b"cmap", cmap_table.as_slice()),
    ];
    rebuild_sfnt(*b"\x00\x01\x00\x00", &orig, &cmap_table).unwrap_or_default()
}

fn build_format4_cmap() -> Vec<u8> {
    // 3 个段：'A'..'Z' (65..=90 -> gid 1..=26), '中' (0x4E2D -> gid 30), sentinel 0xFFFF
    let seg_count: u16 = 3;
    let seg_x2 = seg_count * 2;
    let mut sub = Vec::new();
    sub.extend_from_slice(&4u16.to_be_bytes());
    let sub_len = 16 + seg_count * 8;
    sub.extend_from_slice(&sub_len.to_be_bytes());
    sub.extend_from_slice(&0u16.to_be_bytes());
    sub.extend_from_slice(&seg_x2.to_be_bytes());
    sub.extend_from_slice(&4u16.to_be_bytes());
    sub.extend_from_slice(&1u16.to_be_bytes());
    sub.extend_from_slice(&2u16.to_be_bytes());
    for end in [90u16, 0x4E2D, 0xFFFF] {
        sub.extend_from_slice(&end.to_be_bytes());
    }
    sub.extend_from_slice(&0u16.to_be_bytes());
    for start in [65u16, 0x4E2D, 0xFFFF] {
        sub.extend_from_slice(&start.to_be_bytes());
    }
    let d1 = 1u16.wrapping_sub(65);
    let d2 = 30u16.wrapping_sub(0x4E2D);
    for delta in [d1, d2, 1u16] {
        sub.extend_from_slice(&delta.to_be_bytes());
    }
    for _ in 0..3 {
        sub.extend_from_slice(&0u16.to_be_bytes());
    }
    let mut cmap = Vec::new();
    cmap.extend_from_slice(&0u16.to_be_bytes());
    cmap.extend_from_slice(&1u16.to_be_bytes());
    cmap.extend_from_slice(&3u16.to_be_bytes());
    cmap.extend_from_slice(&1u16.to_be_bytes());
    cmap.extend_from_slice(&12u32.to_be_bytes());
    cmap.extend_from_slice(&sub);
    cmap
}

#[test]
fn codepoint_classification() {
    assert!(is_cjk_codepoint('中' as u32));
    assert!(is_cjk_codepoint('。' as u32));
    assert!(is_cjk_codepoint('あ' as u32));
    assert!(is_cjk_codepoint('，' as u32));
    assert!(!is_cjk_codepoint('A' as u32));
    assert!(!is_cjk_codepoint('9' as u32));
    assert!(!is_cjk_codepoint(' ' as u32));
    assert!(!is_cjk_codepoint('-' as u32));
}

#[test]
fn masks_format4_and_format12_cleanly() {
    let sfnt4 = build_synthetic_sfnt(build_format4_cmap());
    let Some(MaskOutcome::Masked(non_cjk)) = mask_sfnt_face(&sfnt4, 0, ScriptMask::NonCjk) else {
        assert_eq!("expected masked NonCjk", "");
        return;
    };
    let Some(MaskOutcome::Masked(cjk_only)) = mask_sfnt_face(&sfnt4, 0, ScriptMask::Cjk) else {
        assert_eq!("expected masked Cjk", "");
        return;
    };

    let mut col = fontique::Collection::new(fontique::CollectionOptions {
        shared: false,
        system_fonts: false,
    });
    let r_en = col.register_fonts(
        fontique::Blob::new(Arc::new(non_cjk)),
        Some(fontique::FontInfoOverride {
            family_name: Some("MaskEn"),
            ..Default::default()
        }),
    );
    let r_zh = col.register_fonts(
        fontique::Blob::new(Arc::new(cjk_only)),
        Some(fontique::FontInfoOverride {
            family_name: Some("MaskZh"),
            ..Default::default()
        }),
    );
    assert_eq!(r_en.len(), 1);
    assert_eq!(r_zh.len(), 1);
    let (_, en_fonts) = &r_en[0];
    let (_, zh_fonts) = &r_zh[0];
    let Some(en_blob) = en_fonts[0].load(None) else {
        assert_eq!("missing en blob", "");
        return;
    };
    let Some(zh_blob) = zh_fonts[0].load(None) else {
        assert_eq!("missing zh blob", "");
        return;
    };
    let Some(en_cm) = en_fonts[0].charmap_index().charmap(en_blob.data()) else {
        assert_eq!("missing en charmap", "");
        return;
    };
    let Some(zh_cm) = zh_fonts[0].charmap_index().charmap(zh_blob.data()) else {
        assert_eq!("missing zh charmap", "");
        return;
    };
    assert_eq!(en_cm.map('A'), Some(1));
    assert_eq!(en_cm.map('Z'), Some(26));
    assert_eq!(en_cm.map('中'), None);
    assert_eq!(zh_cm.map('中'), Some(30));
    assert_eq!(zh_cm.map('A'), None);

    let sfnt12_en = en_blob.data().to_vec();
    assert_eq!(
        mask_sfnt_face(&sfnt12_en, 0, ScriptMask::NonCjk),
        Some(MaskOutcome::Unchanged)
    );
    let Some(MaskOutcome::Masked(empty_cjk)) = mask_sfnt_face(&sfnt12_en, 0, ScriptMask::Cjk)
    else {
        assert_eq!("expected masked empty Cjk", "");
        return;
    };
    let r_empty = col.register_fonts(
        fontique::Blob::new(Arc::new(empty_cjk)),
        Some(fontique::FontInfoOverride {
            family_name: Some("EmptyZh"),
            ..Default::default()
        }),
    );
    let empty_f = &r_empty[0].1[0];
    let Some(empty_blob) = empty_f.load(None) else {
        assert_eq!("missing empty blob", "");
        return;
    };
    let Some(empty_cm) = empty_f.charmap_index().charmap(empty_blob.data()) else {
        assert_eq!("missing empty charmap", "");
        return;
    };
    assert_eq!(empty_cm.map('A'), None);
    assert_eq!(empty_cm.map('中'), None);
}
