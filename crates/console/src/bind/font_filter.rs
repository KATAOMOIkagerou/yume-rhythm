//! 字体搜索过滤与 `ui::FontOption` 列表构造。

use crate::fonts::filter_font_options;
use crate::ui::{self, Store};
use crate::view::View;
use slint::{Model, VecModel};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Default)]
pub(super) struct FontFilterState {
    pub zh_query: String,
    pub en_query: String,
}

pub(super) fn build_options(
    labels: &[String],
    selected_idx: i32,
    query: &str,
) -> Vec<ui::FontOption> {
    filter_font_options(labels, query)
        .into_iter()
        .filter_map(|orig_idx| {
            let index = i32::try_from(orig_idx).ok()?;
            let label = labels.get(orig_idx)?;
            Some(ui::FontOption {
                index,
                label: label.as_str().into(),
                selected: index == selected_idx,
            })
        })
        .collect()
}

pub(super) fn sync_options<T: Clone + PartialEq + 'static>(
    model: &VecModel<T>,
    rows: impl Iterator<Item = T>,
) {
    let mut len = 0;
    for (i, row) in rows.enumerate() {
        len = i + 1;
        if i < model.row_count() {
            if model.row_data(i).as_ref() != Some(&row) {
                model.set_row_data(i, row);
            }
        } else {
            model.push(row);
        }
    }
    while model.row_count() > len {
        model.remove(model.row_count() - 1);
    }
}

pub(super) fn wire(
    store: &Store<'_>,
    shown: &Rc<RefCell<Option<View>>>,
    filter: &Rc<RefCell<FontFilterState>>,
    zh_model: &Rc<VecModel<ui::FontOption>>,
    en_model: &Rc<VecModel<ui::FontOption>>,
) {
    let shown_zh = shown.clone();
    let filter_zh = filter.clone();
    let model_zh = zh_model.clone();
    store.on_filter_zh_fonts(move |query| {
        let q = query.to_string();
        filter_zh.borrow_mut().zh_query.clone_from(&q);
        if let Some(v) = shown_zh.borrow().as_ref() {
            let opts = build_options(&v.zh_fonts, v.settings.zh_font_index, &q);
            sync_options(&model_zh, opts.into_iter());
        }
    });

    let shown_en = shown.clone();
    let filter_en = filter.clone();
    let model_en = en_model.clone();
    store.on_filter_en_fonts(move |query| {
        let q = query.to_string();
        filter_en.borrow_mut().en_query.clone_from(&q);
        if let Some(v) = shown_en.borrow().as_ref() {
            let opts = build_options(&v.en_fonts, v.settings.en_font_index, &q);
            sync_options(&model_en, opts.into_iter());
        }
    });
}
