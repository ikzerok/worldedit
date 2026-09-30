//! 完整对象身份选择器：只读 core 目录，永不按显示名猜测身份。
use egui::Ui;
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
pub(super) fn object_picker(
    ui: &mut Ui,
    salt: impl std::hash::Hash,
    label: &str,
    current: &mut Option<TargetRef>,
    catalog: &Catalog,
    allowed: &[&str],
) -> bool {
    let before = current.clone();
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
    egui::ComboBox::from_id_salt(id)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .selected_text(caption)
        .width(ui.available_width().max(160.0) - 12.0)
        .show_ui(ui, |ui| {
            let mut query = ui
                .data_mut(|data| data.get_temp::<String>(id))
                .unwrap_or_default();
            ui.add(egui::TextEdit::singleline(&mut query).hint_text("搜索名称、类型、ID或来源"));
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
                        let text = format!(
                            "{} · {}:{}\n{}:{}",
                            object.display,
                            super::catalog::kind_label(&object.target.kind),
                            object.target.id,
                            object.file,
                            object.line
                        );
                        if ui
                            .selectable_label(current.as_ref() == Some(&object.target), text)
                            .clicked()
                        {
                            *current = Some(object.target.clone());
                            ui.close();
                        }
                    }
                });
            if candidates.len() > 1000 {
                ui.label("匹配超过1000项，请继续输入缩小范围");
            }
        });
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
}
