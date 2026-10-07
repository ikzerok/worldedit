//! 完整对象身份选择器：只读 core 分页目录，永不按显示名猜测身份。
mod browse;
mod page;
use egui::Ui;
pub(in crate::app) use page::CandidatePage;
use std::path::{Path, PathBuf};
use worldline_core::catalog::{Catalog, CatalogObject, TargetRef};
use worldline_core::object_search::ObjectSearchFilter;

pub(in crate::app) fn filter(allowed: &[&str], entity_type: Option<&str>) -> ObjectSearchFilter {
    ObjectSearchFilter {
        allowed_kinds: allowed.iter().map(|kind| (*kind).into()).collect(),
        match_source_path: true,
        entity_type: entity_type.map(str::to_owned),
    }
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
        .push_id(
            (
                &object.target.kind,
                &object.target.id,
                &object.file,
                object.line,
            ),
            |ui| ui.add(egui::Button::selectable(selected, job).wrap()),
        )
        .inner
        .on_hover_text(&full);
    response.context_menu(|ui| {
        if ui.button("复制完整身份与来源").clicked() {
            ui.ctx().copy_text(full.clone());
            ui.close();
        }
    });
    crate::theme::selection_frame(ui, &response, selected);
    response
}

pub(super) fn candidate_row_at_revision(
    ui: &mut Ui,
    object: &CatalogObject,
    root: Option<&Path>,
    selected: bool,
    revision: impl std::hash::Hash,
) -> egui::Response {
    ui.push_id(revision, |ui| candidate_row(ui, object, root, selected))
        .inner
}

pub(in crate::app) fn consume_candidate_ime_keys(ctx: &egui::Context) {
    ctx.input_mut(|input| {
        for key in [
            egui::Key::Enter,
            egui::Key::Escape,
            egui::Key::ArrowDown,
            egui::Key::ArrowUp,
            egui::Key::PageDown,
            egui::Key::PageUp,
        ] {
            input.consume_key(egui::Modifiers::NONE, key);
        }
    });
}

pub(super) fn set_workspace_snapshot(ctx: &egui::Context, root: &Path, version: u64) {
    set_workspace_root(ctx, root);
    ctx.data_mut(|data| {
        data.insert_temp(egui::Id::new("object-picker-workspace-version"), version)
    });
}

fn catalog_stamp(ui: &Ui, catalog: &Catalog, root: &Option<PathBuf>) -> String {
    use std::hash::{Hash, Hasher};
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    root.hash(&mut hash);
    ui.data(|data| data.get_temp::<u64>(egui::Id::new("object-picker-workspace-version")))
        .hash(&mut hash);
    for object in &catalog.objects {
        (
            &object.target.kind,
            &object.target.id,
            &object.display,
            &object.file,
            object.line,
        )
            .hash(&mut hash);
    }
    for alias in &catalog.aliases {
        (&alias.target.kind, &alias.target.id, &alias.name).hash(&mut hash);
    }
    for (id, entity) in &catalog.entities {
        (id, &entity.entity_type).hash(&mut hash);
    }
    format!("{:016x}", hash.finish())
}

#[derive(Clone, Default)]
struct PickerState {
    query: String,
    page: CandidatePage,
    selected: usize,
    ime: bool,
}

pub(super) fn object_picker(
    ui: &mut Ui,
    salt: impl std::hash::Hash,
    label: &str,
    current: &mut Option<TargetRef>,
    catalog: &Catalog,
    allowed: &[&str],
) -> bool {
    object_picker_focused(ui, salt, label, current, catalog, allowed, false).0
}

pub(super) fn object_picker_typed(
    ui: &mut Ui,
    salt: impl std::hash::Hash,
    label: &str,
    current: &mut Option<TargetRef>,
    catalog: &Catalog,
    filter: &ObjectSearchFilter,
) -> bool {
    picker(ui, salt, label, current, catalog, filter, false).0
}

/// 返回真实控件身份，供按需工具的进入/返回焦点使用。
pub(super) fn object_picker_focused(
    ui: &mut Ui,
    salt: impl std::hash::Hash,
    label: &str,
    current: &mut Option<TargetRef>,
    catalog: &Catalog,
    allowed: &[&str],
    request_focus: bool,
) -> (bool, egui::Id) {
    picker(
        ui,
        salt,
        label,
        current,
        catalog,
        &filter(allowed, None),
        request_focus,
    )
}

fn picker(
    ui: &mut Ui,
    salt: impl std::hash::Hash,
    label: &str,
    current: &mut Option<TargetRef>,
    catalog: &Catalog,
    filter: &ObjectSearchFilter,
    request_focus: bool,
) -> (bool, egui::Id) {
    let before = current.clone();
    let root = ui
        .ctx()
        .data(|data| data.get_temp::<PathBuf>(egui::Id::new("object-picker-workspace-root")));
    let revision = catalog_stamp(ui, catalog, &root);
    let id = ui.make_persistent_id(salt);
    let was_open = ui
        .data(|data| data.get_temp::<bool>(id.with("was-open")))
        .unwrap_or(false);
    let mut state = ui
        .data_mut(|data| data.get_temp::<PickerState>(id))
        .unwrap_or_default();
    let ime_frame = ui.input(|input| {
        input
            .events
            .iter()
            .filter_map(|event| {
                if let egui::Event::Ime(event) = event {
                    Some(event)
                } else {
                    None
                }
            })
            .fold(false, |_, event| {
                state.ime = matches!(event, egui::ImeEvent::Enabled | egui::ImeEvent::Preedit(_));
                true
            })
    });
    if was_open && (ime_frame || state.ime) {
        consume_candidate_ime_keys(ui.ctx());
        let query_id = id.with("query");
        if ui.memory(|memory| memory.had_focus_last_frame(query_id)) {
            ui.memory_mut(|memory| memory.request_focus(query_id));
        }
    }
    let caption = current
        .as_ref()
        .map(|target| {
            catalog
                .object(target)
                .map(|object| format!("{} · {}:{}", object.display, target.kind, target.id))
                .unwrap_or_else(|| format!("已失效 · {}:{}", target.kind, target.id))
        })
        .unwrap_or_else(|| "请选择".into());
    let selected_caption = caption.clone();
    let mut selected_explicitly = false;
    ui.label(label);
    let width = ui
        .available_width()
        .min((ui.ctx().screen_rect().width() - 48.0).max(1.0))
        .max(1.0);
    let response = egui::ComboBox::from_id_salt(id)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .selected_text(caption)
        .truncate()
        .height(420.0)
        .width(width)
        .show_ui(ui, |ui| {
            ui.set_width((width - 16.0).max(1.0));
            let search = ui.add(
                egui::TextEdit::singleline(&mut state.query)
                    .id(id.with("query"))
                    .desired_width(ui.available_width())
                    .hint_text("搜索名称、类型、ID或来源"),
            );
            if !was_open {
                search.request_focus();
            }
            ui.memory_mut(|memory| {
                memory.set_focus_lock_filter(
                    search.id,
                    egui::EventFilter {
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        escape: true,
                        ..Default::default()
                    },
                )
            });
            ui.small("已应用目录 · 支持别名 · ↑↓选择 · 翻页键换页 · Enter确认");
            if search.changed() {
                state.selected = 0;
            }
            let previous_serial = state.page.serial;
            let stale = state.page.refresh(catalog, &state.query, filter, &revision);
            let query_changed = previous_serial != state.page.serial;
            if query_changed {
                state.selected = 0;
            }
            if stale {
                state.selected = 0;
                ui.colored_label(
                    crate::theme::GOLD(),
                    "目录已变化，请在新页重新选择；原引用保留",
                );
            }
            if ui
                .selectable_label(current.is_none(), "不指定 / 清空")
                .clicked()
            {
                *current = None;
                selected_explicitly = true;
                ui.close();
            }
            let keyboard = was_open && !state.ime && !ime_frame && !stale;
            let mut turned = state.page.controls(ui);
            if keyboard {
                if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::PageDown)) {
                    turned |= state.page.turn(false);
                }
                if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::PageUp)) {
                    turned |= state.page.turn(true);
                }
            }
            if turned {
                state.selected = 0;
                state.page.refresh(catalog, &state.query, filter, &revision);
            }
            let items = state
                .page
                .result
                .as_ref()
                .and_then(|page| page.as_ref().ok())
                .map(|page| page.items.clone())
                .unwrap_or_default();
            let mut moved = query_changed || search.changed() || turned || !was_open;
            if keyboard {
                let (down, up) = ui.input_mut(|i| {
                    (
                        i.count_and_consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                        i.count_and_consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                    )
                });
                moved |= down > 0 || up > 0;
                state.selected = state.selected.saturating_add(down).saturating_sub(up);
            }
            state.selected = state.selected.min(items.len().saturating_sub(1));
            if items.is_empty() && matches!(state.page.result, Some(Ok(_))) {
                ui.label("没有匹配对象；原引用保持不变");
            }
            let mut selected_visible = false;
            egui::ScrollArea::vertical()
                .id_salt(id.with("matches"))
                .max_height(260.0)
                .show(ui, |ui| {
                    for (index, object) in items.iter().enumerate() {
                        let selected = state.selected == index;
                        let row = candidate_row_at_revision(
                            ui,
                            object,
                            root.as_deref(),
                            selected,
                            state.page.serial,
                        );
                        if selected {
                            selected_visible = ui.clip_rect().contains_rect(row.rect);
                            if moved {
                                row.scroll_to_me(None);
                            }
                        }
                        if row.clicked() && !stale {
                            *current = Some(object.target.clone());
                            selected_explicitly = true;
                            ui.close();
                        }
                    }
                });
            if keyboard
                && selected_visible
                && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
            {
                if let Some(object) = items.get(state.selected) {
                    *current = Some(object.target.clone());
                    selected_explicitly = true;
                    ui.close();
                }
            }
        });
    let focus_id = response.response.id;
    let now_open = egui::ComboBox::is_open(ui.ctx(), focus_id);
    ui.data_mut(|data| {
        data.insert_temp(id.with("was-open"), now_open);
        data.insert_temp(id, state);
    });
    let escaped = was_open && !now_open && ui.input(|i| i.key_pressed(egui::Key::Escape));
    if (request_focus && !now_open) || escaped || selected_explicitly {
        response.response.request_focus();
    }
    response.response.on_hover_text(selected_caption);
    if let Some(target) = current {
        let valid = catalog
            .object(target)
            .is_some_and(|object| filter.accepts_object(catalog, object));
        if !valid {
            ui.colored_label(
                crate::theme::ERROR(),
                "当前引用无法在此范围确认；原身份保留，未自动替换",
            );
        }
    }
    (*current != before, focus_id)
}

#[cfg(test)]
mod interaction_tests;
#[cfg(test)]
mod tests;
