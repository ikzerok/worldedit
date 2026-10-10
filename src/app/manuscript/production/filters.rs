use super::*;
use crate::app::writing_workspace::register_input;

pub(super) fn draw(app: &mut WorldeditApp, ui: &mut egui::Ui, enabled: bool) {
    let speakers: Vec<_> = app
        .snapshot
        .as_ref()
        .map(|snapshot| {
            snapshot
                .result
                .analysis
                .symbols
                .characters
                .iter()
                .map(|(id, character)| (id.clone(), character.display.clone()))
                .collect()
        })
        .unwrap_or_default();
    let state = &mut app.manuscript.production;
    let before = (
        state.scope,
        state.selected_only,
        state.speaker.clone(),
        state.locale.clone(),
        state.fallback,
        state.fragments,
        state.narration,
        state.choices,
        state.status,
        state.search.clone(),
    );
    theme::add_enabled_ui(ui, enabled, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label("范围");
            keyboard::selectable(ui, &mut state.scope, 0, "当前正文目标");
            keyboard::selectable(ui, &mut state.scope, 1, "书稿筛选章节");
            keyboard::selectable(ui, &mut state.scope, 2, "全工程活动定义");
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("已应用稿角色候选");
            let speaker_combo = egui::ComboBox::from_id_salt("production-speaker")
                .selected_text(
                    state
                        .speaker
                        .as_ref()
                        .map(|target| format!("character:{}", target.id))
                        .unwrap_or_else(|| "全部正式角色".into()),
                )
                .show_ui(ui, |ui| {
                    keyboard::selectable(ui, &mut state.speaker, None, "全部正式角色");
                    for (id, display) in &speakers {
                        keyboard::selectable(
                            ui,
                            &mut state.speaker,
                            Some(worldline_core::TargetRef::new("character", id)),
                            format!("{display} · character:{id}"),
                        );
                    }
                });
            keyboard::control(ui, speaker_combo.response);
            ui.label("交付语言");
            let response = keyboard::input(
                ui,
                egui::TextEdit::singleline(&mut state.locale)
                    .id_salt("production-locale")
                    .hint_text("留空为源文")
                    .desired_width(130.0),
            );
            register_input(&response);
            let status_combo = egui::ComboBox::from_id_salt("production-status")
                .selected_text(state.status.map(rows::status_label).unwrap_or("全部状态"))
                .show_ui(ui, |ui| {
                    keyboard::selectable(ui, &mut state.status, None, "全部状态");
                    for status in [
                        ProductionStatus::Source,
                        ProductionStatus::Translated,
                        ProductionStatus::Missing,
                        ProductionStatus::Stale,
                        ProductionStatus::Invalid,
                    ] {
                        keyboard::selectable(
                            ui,
                            &mut state.status,
                            Some(status),
                            rows::status_label(status),
                        );
                    }
                });
            keyboard::control(ui, status_combo.response);
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("character 的完整 ID");
            let mut id = state
                .speaker
                .as_ref()
                .map(|target| target.id.clone())
                .unwrap_or_default();
            let response = keyboard::input(
                ui,
                egui::TextEdit::singleline(&mut id)
                    .id_salt("production-speaker-id")
                    .hint_text("留空为全部，例如 traveler")
                    .desired_width(220.0),
            );
            register_input(&response);
            if response.changed() {
                state.speaker =
                    (!id.is_empty()).then(|| worldline_core::TargetRef::new("character", &id));
            }
            ui.label(theme::muted(
                "可填写尚在正文草稿中的新人物 ID；生成时由 core 核对当前完整稿，结果显示正式身份。",
            ));
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("完整范围查找");
            register_input(&keyboard::input(
                ui,
                egui::TextEdit::singleline(&mut state.search)
                    .id_salt("production-search")
                    .desired_width(220.0),
            ));
            keyboard::checkbox(ui, &mut state.fragments, "纳入共享片段定义");
        });
        keyboard::collapsing(ui, "交付范围与语言选项", |ui| {
            keyboard::checkbox(ui, &mut state.narration, "同时纳入普通旁白");
            keyboard::checkbox(ui, &mut state.choices, "同时纳入选择文案");
            keyboard::checkbox(
                ui,
                &mut state.fallback,
                "明确允许缺译、过期或无效行回退源文",
            );
            ui.label(theme::muted("默认严格检查最终所选单元。人物名称与演出备注始终是源语言元数据；不猜测冒号姓名，也不按 POV/with 过滤说话者。"));
            #[cfg(target_arch = "wasm32")]
            ui.label(theme::muted(
                "浏览器在点击生成后进行有界计算；大型工程可缩小范围，没有后台线程或持续磁盘监视。",
            ));
        });
        if !state.fragments {
            ui.colored_label(
                theme::WARNING(),
                "直接范围 · 未扩展调用的共享片段，不能称为包含调用内容的完整角色台本。",
            );
        }
        if state.scope == 1 {
            keyboard::checkbox(
                ui,
                &mut state.selected_only,
                "从当前完整书稿筛选中明确勾选章节",
            );
        }
    });
    let after = (
        state.scope,
        state.selected_only,
        state.speaker.clone(),
        state.locale.clone(),
        state.fallback,
        state.fragments,
        state.narration,
        state.choices,
        state.status,
        state.search.clone(),
    );
    if before != after {
        state.confirmed = false;
        state.offset = 0;
    }
    if state.scope == 1 && state.selected_only {
        chapter_selection(app, ui, enabled);
    }
}

fn chapter_selection(app: &mut WorldeditApp, ui: &mut egui::Ui, enabled: bool) {
    let mut request = match app.production_book_query() {
        Ok(request) => request,
        Err(error) => {
            ui.colored_label(theme::ERROR(), error);
            return;
        }
    };
    request.offset = app.manuscript.production.chapter_offset;
    request.limit = 20;
    let drafts = app.production_drafts();
    let snapshot = app.manuscript.query_cache.current(
        &app.project,
        app.manuscript.writing_buffers.values(),
        &drafts,
    );
    let page =
        snapshot.and_then(|snapshot| snapshot.query(&request).map_err(|error| error.to_string()));
    let page = match page {
        Ok(page) => page,
        Err(error) => {
            ui.colored_label(theme::ERROR(), error);
            return;
        }
    };
    let state = &mut app.manuscript.production;
    theme::add_enabled_ui(ui, enabled && page.complete, |ui| {
        keyboard::header(
            ui,
            egui::CollapsingHeader::new(format!(
                "勾选章节 · 已选 {} / 匹配 {}",
                state.selected_chapters.len(),
                page.matching_chapters
            ))
            .default_open(true),
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    if keyboard::add(ui, request.offset > 0, egui::Button::new("前20章")).clicked()
                    {
                        state.chapter_offset = request.offset.saturating_sub(20);
                    }
                    if keyboard::add(
                        ui,
                        request.offset + page.rows.len() < page.total_rows,
                        egui::Button::new("后20章"),
                    )
                    .clicked()
                    {
                        state.chapter_offset += 20;
                    }
                    if keyboard::button(ui, "清空章节勾选").clicked() {
                        state.selected_chapters.clear();
                        state.confirmed = false;
                    }
                });
                for row in &page.rows {
                    let mut selected = state.selected_chapters.contains(&row.entry.id);
                    if keyboard::add(
                        ui,
                        !row.identity_ambiguous && !row.context_only,
                        egui::Checkbox::new(
                            &mut selected,
                            format!("{} · {}", row.entry.title, row.entry.id),
                        ),
                    )
                    .changed()
                    {
                        if selected {
                            state.selected_chapters.insert(row.entry.id.clone());
                        } else {
                            state.selected_chapters.remove(&row.entry.id);
                        }
                        state.confirmed = false;
                    }
                }
                ui.label(theme::muted(
                    "勾选按稳定章节 ID 核对；不勾任何章即明确空范围，折叠和当前页不缩小查询。",
                ));
            },
        );
    });
}
