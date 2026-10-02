//! 完整对象身份选择器：只读 core 目录，永不按显示名猜测身份。
use egui::Ui;
use std::path::{Path, PathBuf};
use worldline_core::catalog::{Catalog, CatalogObject, TargetRef};

pub(super) fn matches(object: &CatalogObject, query: &str, allowed: &[&str]) -> bool {
    (allowed.is_empty() || allowed.contains(&object.target.kind.as_str()))
        && [
            object.display.as_str(),
            object.target.kind.as_str(),
            object.target.id.as_str(),
            object.file.as_str(),
        ]
        .iter()
        .any(|part| part.to_lowercase().contains(&query.trim().to_lowercase()))
}
pub(super) fn candidates<'a>(
    catalog: &'a Catalog,
    query: &str,
    allowed: &[&str],
) -> Vec<&'a CatalogObject> {
    let query = query.trim().to_lowercase();
    catalog
        .objects
        .iter()
        .filter(|object| {
            (allowed.is_empty() || allowed.contains(&object.target.kind.as_str()))
                && (matches(object, &query, allowed)
                    || catalog
                        .aliases_for(&object.target)
                        .iter()
                        .any(|alias| alias.to_lowercase().contains(&query)))
        })
        .collect()
}
/// 命令面板与资料选择器显示同一完整身份，不以同名合并候选。
pub(super) fn candidate_label(object: &CatalogObject) -> String {
    candidate_caption(object, None)
}

pub(super) fn candidate_caption(object: &CatalogObject, root: Option<&Path>) -> String {
    let path = Path::new(&object.file);
    let source = root.map_or_else(
        || object.file.clone(),
        |root| crate::theme::relative_source(root, path),
    );
    format!(
        "{} · {}:{}\n{}:{}",
        object.display,
        super::catalog::kind_label(&object.target.kind),
        object.target.id,
        source,
        object.line
    )
}

/// 只用于当前界面的来源 caption；不参与检索、对象身份或文件操作。
pub(super) fn set_workspace_root(ctx: &egui::Context, root: &Path) {
    ctx.data_mut(|data| {
        data.insert_temp(
            egui::Id::new("object-picker-workspace-root"),
            root.to_path_buf(),
        );
    });
}

pub(super) fn candidate_row(
    ui: &mut Ui,
    object: &CatalogObject,
    root: Option<&Path>,
    selected: bool,
) -> egui::Response {
    let text = candidate_caption(object, root);
    let (identity, source) = text.split_once('\n').unwrap_or((&text, ""));
    let mut job = egui::text::LayoutJob::default();
    job.append(
        identity,
        0.0,
        egui::TextFormat {
            font_id: egui::TextStyle::Body.resolve(ui.style()),
            color: ui.visuals().text_color(),
            ..Default::default()
        },
    );
    job.append(
        &format!("\n{source}"),
        0.0,
        egui::TextFormat {
            font_id: egui::TextStyle::Small.resolve(ui.style()),
            color: crate::theme::MUTED(),
            ..Default::default()
        },
    );
    let full = candidate_label(object);
    let response = ui
        .add(egui::Button::selectable(selected, job).wrap())
        .on_hover_text(&full);
    response.context_menu(|ui| {
        if ui.button("复制完整身份与来源").clicked() {
            ui.ctx().copy_text(full.clone());
            ui.close();
        }
    });
    response
}

pub(super) fn object_picker(
    ui: &mut Ui,
    salt: impl std::hash::Hash,
    label: &str,
    current: &mut Option<TargetRef>,
    catalog: &Catalog,
    allowed: &[&str],
) -> bool {
    let before = current.clone();
    let root = ui
        .ctx()
        .data(|data| data.get_temp::<PathBuf>(egui::Id::new("object-picker-workspace-root")));
    let id = ui.make_persistent_id(salt);
    let caption = current
        .as_ref()
        .map(|target| {
            catalog
                .object(target)
                .map(|object| format!("{} · {}:{}", object.display, target.kind, target.id))
                .unwrap_or_else(|| format!("已失效 · {}:{}", target.kind, target.id))
        })
        .unwrap_or_else(|| "请选择".into());
    ui.label(label);
    let width = ui
        .available_width()
        .min((ui.ctx().screen_rect().width() - 48.0).max(1.0))
        .max(1.0);
    let selected_caption = caption.clone();
    let response = egui::ComboBox::from_id_salt(id)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .selected_text(caption)
        .truncate()
        // 搜索/清空固定区 + 260px候选滚动区，避免外层默认200px再次剪裁内层。
        .height(360.0)
        .width(width)
        .show_ui(ui, |ui| {
            ui.set_width((width - 16.0).max(1.0));
            let mut query = ui
                .data_mut(|data| data.get_temp::<String>(id))
                .unwrap_or_default();
            ui.add(
                egui::TextEdit::singleline(&mut query)
                    .desired_width(ui.available_width())
                    .hint_text("搜索名称、类型、ID或来源"),
            );
            ui.data_mut(|data| data.insert_temp(id, query.clone()));
            if ui
                .selectable_label(current.is_none(), "不指定 / 清空")
                .clicked()
            {
                *current = None;
                ui.close();
            }
            let candidates = candidates(catalog, &query, allowed);
            if candidates.is_empty() {
                ui.label("没有可用候选；请调整搜索或先创建对象");
            }
            egui::ScrollArea::vertical()
                .id_salt(id.with("matches"))
                .max_height(260.0)
                .show(ui, |ui| {
                    for object in candidates.iter().take(1000) {
                        let response = candidate_row(
                            ui,
                            object,
                            root.as_deref(),
                            current.as_ref() == Some(&object.target),
                        );
                        if response.clicked() {
                            *current = Some(object.target.clone());
                            ui.close();
                        }
                    }
                });
            if candidates.len() > 1000 {
                ui.label("匹配超过1000项，请继续输入缩小范围");
            }
        });
    response.response.on_hover_text(selected_caption);
    if let Some(target) = current {
        if catalog.object(target).is_none()
            || (!allowed.is_empty() && !allowed.contains(&target.kind.as_str()))
        {
            ui.colored_label(
                crate::theme::ERROR(),
                "当前引用失效或类型不允许；未自动选择其他对象",
            );
        }
    }
    *current != before
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_names_never_collapse_kind_or_source() {
        let a = CatalogObject {
            target: TargetRef::new("entity", "same"),
            display: "林".into(),
            file: "a.wl".into(),
            line: 1,
        };
        let b = CatalogObject {
            target: TargetRef::new("character", "same"),
            display: "林".into(),
            file: "b.wl".into(),
            line: 2,
        };
        assert!(matches(&a, "林", &[]));
        assert!(matches(&b, "林", &[]));
        assert!(!matches(&a, "same", &["character"]));
        assert!(matches(&b, "b.wl", &["character"]));
    }
    #[test]
    fn same_basename_sources_remain_distinguishable_in_the_visible_candidate() {
        let make = |file: &str| CatalogObject {
            target: TargetRef::new("character", "same"),
            display: "林".into(),
            file: file.into(),
            line: 3,
        };
        let left = candidate_label(&make("/project/甲/人物.wl"));
        let right = candidate_label(&make("/project/乙/人物.wl"));
        assert_ne!(left, right);
        assert!(left.contains("/project/甲/人物.wl:3"));
        assert!(right.contains("/project/乙/人物.wl:3"));
    }

    #[test]
    fn relative_caption_preserves_directories_identity_and_full_path_search() {
        let object = CatalogObject {
            target: TargetRef::new("entity", "record_294"),
            display: "潮汐档案第295号".into(),
            file: "/project/深层 目录/档案/人物.wl".into(),
            line: 17,
        };
        let caption = candidate_caption(&object, Some(Path::new("/project")));
        assert_eq!(
            caption,
            "潮汐档案第295号 · 实体:record_294\n深层 目录/档案/人物.wl:17"
        );
        assert!(matches(&object, "/project/深层 目录/档案/人物.wl", &[]));
        assert!(candidate_label(&object).contains(&object.file));
        assert!(candidate_caption(&object, Some(Path::new("/another"))).contains(&object.file));
        assert!(candidate_caption(&object, None).contains(&object.file));
    }

    #[test]
    fn candidate_row_uses_two_text_levels_and_a_relative_source() {
        let ctx = egui::Context::default();
        let object = CatalogObject {
            target: TargetRef::new("character", "lin"),
            display: "林舟".into(),
            file: "/project/人物/林舟.wl".into(),
            line: 3,
        };
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                candidate_row(ui, &object, Some(Path::new("/project")), true);
            });
        });
        let job = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.job.text.contains("林舟") => {
                    Some(&text.galley.job)
                }
                _ => None,
            })
            .expect("候选应实际绘制");
        assert_eq!(job.text, "林舟 · 人物:lin\n人物/林舟.wl:3");
        assert!(job.sections[0].format.font_id.size > job.sections[1].format.font_id.size);
    }
}
