use super::*;
impl WorldeditApp {
    pub(in crate::app) fn project_search(&mut self, ctx: &egui::Context) {
        if !self.search_open {
            return;
        }
        let mut open = true;
        let mut navigate = None;
        let mut close = false;
        let viewport = ctx.screen_rect().shrink(12.0);
        egui::Window::new("查找与替换")
            .id(egui::Id::new("author-search-window"))
            .open(&mut open)
            .collapsible(false)
            .default_width(660.0_f32.min(viewport.width() - 48.0))
            .max_height((viewport.height() - 90.0).max(120.0))
            .constrain_to(viewport)
            .vscroll(true)
            .show(ctx, |ui| {
                let input = ui.add(
                    egui::TextEdit::singleline(&mut self.project_query)
                        .hint_text("字面查找文字")
                        .desired_width(f32::INFINITY),
                );
                if self.search_focus {
                    input.request_focus();
                    self.search_focus = false;
                }
                if input.changed() {
                    self.search_state.selected = 0;
                    self.search_state.plan = None;
                }
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(&mut self.search_state.scope, Scope::Current, "当前文稿");
                    ui.selectable_value(&mut self.search_state.scope, Scope::Selection, "捕获选区");
                    ui.selectable_value(&mut self.search_state.scope, Scope::Project, "工程文件");
                    ui.checkbox(&mut self.search_state.source, "源码（保护标记只查不替换）");
                });
                ui.horizontal_wrapped(|ui| {
                    ui.checkbox(&mut self.search_state.options.case_sensitive, "区分大小写");
                    ui.checkbox(&mut self.search_state.options.whole_word, "整词");
                    ui.checkbox(&mut self.search_state.replace, "替换");
                });
                if self.search_state.scope == Scope::Project {
                    ui.label("显式选择文件；包含每个文件当前未应用草稿，同源只列一次");
                    egui::ScrollArea::vertical()
                        .id_salt("search-files")
                        .max_height(110.0)
                        .show(ui, |ui| {
                            let paths: Vec<_> = self
                                .project
                                .documents
                                .iter()
                                .filter(|(_, d)| !d.is_deleted())
                                .map(|(p, _)| p.clone())
                                .collect();
                            for path in paths {
                                let mut selected = self.search_state.files.contains(&path);
                                if ui
                                    .checkbox(
                                        &mut selected,
                                        path.strip_prefix(&self.project.root)
                                            .unwrap_or(&path)
                                            .display()
                                            .to_string(),
                                    )
                                    .changed()
                                {
                                    if selected {
                                        self.search_state.files.insert(path);
                                    } else {
                                        self.search_state.files.remove(&path);
                                    }
                                    self.search_state.plan = None;
                                }
                            }
                        });
                }
                if self.search_state.scope == Scope::Project
                    && !self.project_query.trim().is_empty()
                {
                    let (objects, warning) = self.search_objects_in_current_drafts();
                    let applied_catalog = warning.is_some();
                    if let Some(warning) = warning {
                        ui.colored_label(crate::theme::GOLD(), warning);
                    }
                    ui.label(format!("{} 个对象 · 名称、ID 或别名匹配", objects.len()));
                    egui::ScrollArea::vertical()
                        .id_salt("object-search")
                        .max_height(130.0)
                        .show(ui, |ui| {
                            for object in objects.iter().take(300) {
                                if ui
                                    .link(format!(
                                        "{} · {} ({})",
                                        crate::app::catalog::kind_label(&object.target.kind),
                                        object.display,
                                        object.target.id
                                    ))
                                    .clicked()
                                {
                                    self.navigate_search_object(ctx, object, applied_catalog);
                                }
                            }
                            if objects.len() > 300 {
                                ui.label("只显示前300个对象，请缩小查询");
                            }
                        });
                }
                let hits = match self.current_search_hits() {
                    Ok(hits) => hits,
                    Err(error) => {
                        ui.colored_label(crate::theme::ERROR(), error);
                        Vec::new()
                    }
                };
                self.refresh_search_navigation(&hits);
                let ime = !self.edit_layer_is_top("search")
                    || self.ime_composing
                    || self.command_palette.ime
                    || self.command_palette.ime_frame;
                ui.horizontal(|ui| {
                    let previous = ui.button("上一个 · Shift+Enter").clicked()
                        || (!ime
                            && ui.input_mut(|i| {
                                i.consume_key(egui::Modifiers::SHIFT, egui::Key::Enter)
                            }));
                    let next = ui
                        .button(if self.search_state.located.is_some() {
                            "下一个 · Enter"
                        } else {
                            "定位选中项 · Enter"
                        })
                        .clicked()
                        || (!ime
                            && ui.input_mut(|i| {
                                i.consume_key(egui::Modifiers::NONE, egui::Key::Enter)
                            }));
                    if !hits.is_empty() && (previous || next) {
                        self.search_state.selected = self.next_search_index(hits.len(), previous);
                        navigate = hits.get(self.search_state.selected).cloned();
                    }
                    ui.label(format!(
                        "{} 处命中 · {}",
                        hits.len(),
                        if self.search_state.located.is_some() {
                            "已定位"
                        } else {
                            "未定位"
                        }
                    ));
                });
                egui::ScrollArea::vertical()
                    .id_salt("search-results")
                    .max_height(190.0)
                    .show(ui, |ui| {
                        for (index, hit) in hits.iter().take(1000).enumerate() {
                            let label = format!(
                                "{}:{}:{}{}{}  {}",
                                hit.path
                                    .strip_prefix(&self.project.root)
                                    .unwrap_or(&hit.path)
                                    .display(),
                                hit.line,
                                hit.column,
                                if hit.draft { " · 草稿" } else { "" },
                                if hit.replaceable { "" } else { " · 受保护" },
                                crate::visual::truncated(&hit.preview, 90)
                            );
                            if ui
                                .selectable_label(index == self.search_state.selected, label)
                                .clicked()
                            {
                                self.search_state.selected = index;
                                navigate = Some(hit.clone());
                            }
                        }
                    });
                if self.search_state.replace {
                    ui.label("替换为（普通正文；不改 ID、引用、内插和行标记）");
                    if ui
                        .text_edit_singleline(&mut self.search_state.replacement)
                        .changed()
                    {
                        self.search_state.plan = None;
                    }
                    if ui.button("预览替换").clicked() {
                        self.preview_search_replacement();
                    }
                    if let Some(plan) = self.search_state.plan.clone() {
                        ui.label(format!(
                            "{} 个文件 · {} 处；跨文件应用包含这些文件当前稿，未保存",
                            plan.changes.len(),
                            plan.hits.len()
                        ));
                        for change in &plan.changes {
                            ui.collapsing(
                                format!("{} · {}处", change.path.display(), change.count),
                                |ui| {
                                    let mut delta = 0isize;
                                    for hit in
                                        plan.hits.iter().filter(|hit| hit.path == change.path)
                                    {
                                        let after_start =
                                            (hit.range.start as isize + delta) as usize;
                                        ui.label(format!("第{}行 · 修改前", hit.line));
                                        ui.monospace(context_excerpt(
                                            &change.before,
                                            hit.range.start,
                                        ));
                                        ui.label("修改后");
                                        ui.monospace(context_excerpt(&change.after, after_start));
                                        delta += plan.request().replacement.len() as isize
                                            - hit.range.len() as isize;
                                    }
                                },
                            );
                        }
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(
                                    !plan.changes.is_empty(),
                                    egui::Button::new("确认应用替换"),
                                )
                                .clicked()
                            {
                                self.apply_search_replacement();
                            }
                            if ui.button("取消替换").clicked() {
                                self.search_state.plan = None;
                            }
                        });
                    }
                }
                if let Some(error) = &self.search_state.error {
                    ui.colored_label(crate::theme::ERROR(), error);
                }
                if ui.button("关闭查找 · Esc").clicked() {
                    close = true;
                }
            });
        if let Some(hit) = navigate {
            self.go_search_hit(ctx, &hit);
        }
        if !open || close {
            self.close_search(ctx);
        }
    }
}

fn context_excerpt(source: &str, byte: usize) -> String {
    let index = source[..byte.min(source.len())].chars().count();
    let start = index.saturating_sub(60);
    format!(
        "{}{}",
        if start > 0 { "…" } else { "" },
        source.chars().skip(start).take(180).collect::<String>()
    )
}
