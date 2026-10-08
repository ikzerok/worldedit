//! 有界章节导航；页面、路径、筛选和真实计数只消费 core DTO。
use super::{navigation::NavigationState, *};
use crate::theme;
use std::sync::Arc;
use worldline_core::manuscript::{
    ManuscriptQueryPage, ManuscriptQueryRequest, ManuscriptQuerySnapshot, ManuscriptQuerySource,
    ManuscriptQueryView,
};

pub(super) fn draw(
    ui: &mut egui::Ui,
    layout: Layout,
    local: &mut LocalBook,
    snapshot: &Result<Arc<ManuscriptQuerySnapshot>, String>,
    state: &mut NavigationState,
    root: &std::path::Path,
    enter_on_click: bool,
) {
    let compact = ui.ctx().screen_rect().height() < 420.0;
    if !compact {
        ui.heading("章节导航");
    }
    let before = (
        state.session.text.clone(),
        state.session.status.clone(),
        state.session.pov.clone(),
        state.session.section_id.clone(),
        state.session.layout,
    );
    let search = ui.add(
        egui::TextEdit::singleline(&mut state.session.text)
            .id(super::navigation_input::id(root, &local.draft.id, "text"))
            .hint_text("查找标题、ID、摘要、目标")
            .desired_width(f32::INFINITY),
    );
    super::navigation_input::register(&search);
    let mut focus_results =
        super::navigation_input::requests_results(ui, &search, state.input.blocked());
    let filter_focused = ui.memory(|memory| memory.focused()).is_some_and(|focused| {
        ["status", "pov"]
            .iter()
            .any(|field| focused == super::navigation_input::id(root, &local.draft.id, field))
    });
    if compact && !filter_focused {
        focus_results |= ui
            .menu_button(
                if state.session.status.is_empty() && state.session.pov.is_empty() {
                    "筛选与显示"
                } else {
                    "筛选与显示 · 已筛选"
                },
                |ui| {
                    let requested = filter_controls(ui, local, state, root);
                    if requested {
                        ui.close();
                    }
                    requested
                },
            )
            .inner
            .unwrap_or(false);
    } else {
        focus_results |= filter_controls(ui, local, state, root);
    }
    if state.input.blocked() {
        theme::disable(ui);
    }
    state.session.layout = layout;
    if before
        != (
            state.session.text.clone(),
            state.session.status.clone(),
            state.session.pov.clone(),
            state.session.section_id.clone(),
            layout,
        )
    {
        state.reset_page();
    }
    if let Some(section) = state.session.section_id.clone() {
        ui.horizontal_wrapped(|ui| {
            ui.label(theme::muted(format!("范围：分节 {section}")));
            if ui.small_button("整书").clicked() {
                state.session.section_id = None;
                state.reset_page();
            }
        });
    } else {
        ui.label(theme::muted(format!(
            "整书 · manuscript:{}",
            local.draft.id
        )));
    }
    let snapshot = match snapshot {
        Ok(snapshot) => snapshot,
        Err(error) => {
            ui.colored_label(theme::ERROR(), format!("当前稿导航未生成：{error}"));
            ui.label("未显示旧稿结果；编排和正文输入均保留。");
            return;
        }
    };
    let mut collapsed: Vec<_> = local.collapsed.iter().cloned().collect();
    collapsed.sort();
    let mut request = ManuscriptQueryRequest {
        manuscript_id: local.draft.id.clone(),
        text: state.session.text.clone(),
        status: state.session.status.clone(),
        pov: state.session.pov.clone(),
        section_id: state.session.section_id.clone(),
        view: if layout == Layout::Tree {
            ManuscriptQueryView::Tree
        } else {
            ManuscriptQueryView::Chapters
        },
        collapsed,
        selected_id: local.selected_entry.clone(),
        offset: state.session.offset,
        limit: if layout == Layout::Cards { 12 } else { 50 },
        ..Default::default()
    };
    let mut page = match state.query(snapshot, &request) {
        Ok(page) => page,
        Err(error) => {
            ui.colored_label(theme::ERROR(), error);
            return;
        }
    };
    if state.session.offset != page.offset {
        state.pending_scroll = Some(0.0);
    }
    state.session.offset = page.offset;
    if page_controls(ui, &page, state) {
        request.offset = state.session.offset;
        page = match state.query(snapshot, &request) {
            Ok(page) => page,
            Err(error) => {
                ui.colored_label(theme::ERROR(), error);
                return;
            }
        };
    }
    if compact {
        ui.label(theme::muted(
            if page.source == ManuscriptQuerySource::Draft {
                "未应用稿 · 静态"
            } else {
                "工程稿 · 静态"
            },
        ))
        .on_hover_text("仅已载入快照，静态统计不代表实际路线；不会应用或保存");
    } else {
        ui.label(theme::muted(format!(
            "{} · {}",
            if page.source == ManuscriptQuerySource::Draft {
                "当前未应用稿"
            } else {
                "当前工程稿"
            },
            "已载入快照 · 静态统计，不代表实际路线"
        )));
    }
    if !page.complete {
        ui.colored_label(
            theme::WARNING(),
            "范围未完整确认；计数仅覆盖可识别项，零命中不表示无问题。",
        );
    }
    diagnostics(ui, &page, state);
    if let Some(row) = page.rows.iter().find(|row| {
        local.selected_entry.as_deref() == Some(&row.entry.id) && !row.identity_ambiguous
    }) {
        let path = super::outline_rows::path(row);
        ui.add(egui::Label::new(theme::muted(&path)).truncate())
            .on_hover_text(path);
    }
    if page.selection_matches == Some(false) {
        ui.label(theme::muted(
            "当前章节不在筛选结果内，或身份已失效；正文与输入仍保留。",
        ));
    } else if page.selection_matches == Some(true) && page.selected_offset.is_none() {
        ui.label(theme::muted("当前章节位于折叠的分节内；正文仍保留。"));
        if ui.small_button("全部展开分节").clicked() {
            local.collapsed.clear();
        }
    } else if let Some(position) = page
        .selected_offset
        .filter(|position| *position < page.offset || *position >= page.offset + page.rows.len())
    {
        if ui.small_button("定位当前章节所在页").clicked() {
            state.session.offset = position / page.limit * page.limit;
            state.pending_scroll = Some(0.0);
        }
    }
    if page.rows.is_empty() {
        ui.label(theme::muted(if page.recognized_chapters == 0 {
            "当前范围没有可识别章节；可插入分节或新建章节。"
        } else {
            "没有匹配项；请调整筛选或返回整书范围。"
        }));
        return;
    }
    if focus_results {
        if let Some(row) = page.rows.iter().find(|row| !row.identity_ambiguous) {
            local.selected_entry = Some(row.entry.id.clone());
            state.request_row_focus = true;
        }
    }
    let mut scroll = if layout == Layout::List {
        egui::ScrollArea::both()
    } else {
        egui::ScrollArea::vertical()
    }
    .id_salt(("manuscript-query-rows", &local.draft.id, layout as u8))
    .auto_shrink([false, false])
    .max_height(ui.available_height().max(100.0));
    if let Some(offset) = state.pending_scroll.take() {
        scroll = scroll.vertical_scroll_offset(offset).animated(false);
    }
    let output = scroll.show(ui, |ui| {
        if layout == Layout::List {
            super::outline_rows::headers(ui, &state.session.columns);
        }
        for row in &page.rows {
            let response =
                super::outline_rows::draw(ui, layout, row, local, &state.session.columns);
            if state.request_row_focus && local.selected_entry.as_deref() == Some(&row.entry.id) {
                response.request_focus();
                response.scroll_to_me(Some(egui::Align::Center));
                state.request_row_focus = false;
            }
            let enter = response.has_focus()
                && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
            if (response.clicked() || enter) && !row.identity_ambiguous {
                local.selected_entry = Some(row.entry.id.clone());
                state.enter_editor = (enter || response.double_clicked() || enter_on_click)
                    && row.entry.kind == ManuscriptEntryKind::Chapter;
            }
            if response.has_focus() {
                let delta = ui.input_mut(|input| {
                    if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                        1
                    } else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                        -1
                    } else {
                        0
                    }
                });
                if delta != 0 {
                    move_selection(&page, local, delta);
                    focus_results = true;
                }
            }
        }
    });
    #[cfg(test)]
    if layout == Layout::List {
        ui.ctx().data_mut(|data| {
            data.insert_temp(
                egui::Id::new(("manuscript-list-scroll", &local.draft.id)),
                output.state.offset,
            )
        });
    }
    state.session.scroll_y = output.state.offset.y;
    if focus_results {
        state.request_row_focus = true;
    }
}

fn page_controls(
    ui: &mut egui::Ui,
    page: &ManuscriptQueryPage,
    state: &mut NavigationState,
) -> bool {
    let previous = state.session.offset;
    ui.horizontal_wrapped(|ui| {
        if crate::theme::add_enabled(ui, page.offset > 0, egui::Button::new("上一页")).clicked()
        {
            state.session.offset = page.offset.saturating_sub(page.limit);
        }
        if crate::theme::add_enabled(ui, page.next_cursor.is_some(), egui::Button::new("下一页"))
            .clicked()
        {
            state.session.offset = page.offset + page.rows.len();
        }
        ui.label(format!(
            "{}–{} / {} 行",
            if page.total_rows == 0 {
                0
            } else {
                page.offset + 1
            },
            page.offset + page.rows.len(),
            page.total_rows
        ));
    });
    ui.label(format!(
        "匹配 {} / 可识别 {} 章",
        page.matching_chapters, page.recognized_chapters
    ));
    if previous != state.session.offset {
        state.pending_scroll = Some(0.0);
        true
    } else {
        false
    }
}

fn diagnostics(ui: &mut egui::Ui, page: &ManuscriptQueryPage, state: &mut NavigationState) {
    if page.diagnostics.is_empty() {
        return;
    }
    egui::CollapsingHeader::new(format!("快照问题 · {} 项", page.diagnostics.len())).show(
        ui,
        |ui| {
            state.diagnostic_offset = state
                .diagnostic_offset
                .min((page.diagnostics.len() - 1) / 8 * 8);
            ui.horizontal_wrapped(|ui| {
                if crate::theme::add_enabled(
                    ui,
                    state.diagnostic_offset > 0,
                    egui::Button::new("前8项"),
                )
                .clicked()
                {
                    state.diagnostic_offset = state.diagnostic_offset.saturating_sub(8);
                }
                if crate::theme::add_enabled(
                    ui,
                    state.diagnostic_offset + 8 < page.diagnostics.len(),
                    egui::Button::new("后8项"),
                )
                .clicked()
                {
                    state.diagnostic_offset += 8;
                }
            });
            egui::ScrollArea::vertical()
                .id_salt("manuscript-query-diagnostics")
                .max_height(120.0)
                .show(ui, |ui| {
                    for diagnostic in page
                        .diagnostics
                        .iter()
                        .skip(state.diagnostic_offset)
                        .take(8)
                    {
                        ui.colored_label(
                            theme::ERROR(),
                            format!("{}：{}", diagnostic.code, diagnostic.message),
                        );
                    }
                });
        },
    );
}

fn move_selection(page: &ManuscriptQueryPage, local: &mut LocalBook, delta: i32) {
    let candidates: Vec<_> = page
        .rows
        .iter()
        .filter(|row| !row.identity_ambiguous)
        .collect();
    if candidates.is_empty() {
        return;
    }
    let current = candidates
        .iter()
        .position(|row| local.selected_entry.as_deref() == Some(&row.entry.id))
        .unwrap_or(0);
    let next = (current as i32 + delta).clamp(0, candidates.len() as i32 - 1) as usize;
    local.selected_entry = Some(candidates[next].entry.id.clone());
}

fn filter_controls(
    ui: &mut egui::Ui,
    local: &LocalBook,
    state: &mut NavigationState,
    root: &std::path::Path,
) -> bool {
    let mut requested = false;
    ui.horizontal(|ui| {
        let width = ((ui.available_width() - ui.spacing().item_spacing.x) / 2.0).max(32.0);
        let status = ui.add_sized(
            [width, ui.spacing().interact_size.y],
            egui::TextEdit::singleline(&mut state.session.status)
                .id(super::navigation_input::id(root, &local.draft.id, "status"))
                .hint_text("筛选状态，如 draft"),
        );
        super::navigation_input::register(&status);
        requested |= super::navigation_input::requests_results(ui, &status, state.input.blocked());
        let pov = ui.add_sized(
            [width, ui.spacing().interact_size.y],
            egui::TextEdit::singleline(&mut state.session.pov)
                .id(super::navigation_input::id(root, &local.draft.id, "pov"))
                .hint_text("筛选视角：名称或 ID"),
        );
        super::navigation_input::register(&pov);
        requested |= super::navigation_input::requests_results(ui, &pov, state.input.blocked());
    });
    if state.input.blocked() {
        theme::disable(ui);
    }
    ui.horizontal_wrapped(|ui| {
        ui.menu_button("显示列", |ui| {
            let columns = &mut state.session.columns;
            ui.checkbox(&mut columns.identity, "编排 ID");
            ui.checkbox(&mut columns.summary, "摘要");
            ui.checkbox(&mut columns.perspective, "POV 与身份");
            ui.checkbox(&mut columns.status, "状态");
            ui.checkbox(&mut columns.statistics, "静态统计");
            ui.checkbox(&mut columns.goal, "写作目标");
            ui.checkbox(&mut columns.source, "正文来源");
            ui.label(theme::muted("列选择只保存在个人会话"));
        });
        if ui.small_button("清除筛选").clicked() {
            state.session.text.clear();
            state.session.status.clear();
            state.session.pov.clear();
            state.session.section_id = None;
        }
        if let Some(selected) = local
            .selected_entry
            .as_deref()
            .and_then(|id| local.draft.entries.iter().find(|entry| entry.id == id))
            .filter(|entry| entry.kind == ManuscriptEntryKind::Section)
        {
            if ui.small_button("仅此分节").clicked() {
                state.session.section_id = Some(selected.id.clone());
            }
        }
    });
    requested
}
