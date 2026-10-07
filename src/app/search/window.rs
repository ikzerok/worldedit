use super::*;

impl WorldeditApp {
    pub(in crate::app) fn project_search(&mut self, ctx: &egui::Context) {
        if !self.search_open {
            return;
        }
        if self.search_state.source_view {
            self.search_source_return_window(ctx);
            return;
        }
        let mut open = true;
        let mut close = false;
        let viewport = ctx.screen_rect().shrink(12.0);
        let style = ctx.style();
        let frame = egui::Frame::window(&style);
        let title_height = ctx
            .fonts(|fonts| fonts.row_height(&style.text_styles[&egui::TextStyle::Heading]))
            .max(style.spacing.interact_size.y);
        let frame_size = frame.total_margin().sum();
        let chrome_height =
            frame_size.y + title_height + frame.inner_margin.sum().y + frame.stroke.width;
        egui::Window::new("查找与替换")
            .id(egui::Id::new("author-search-window"))
            .open(&mut open)
            .collapsible(false)
            .default_size(egui::vec2(
                760.0_f32.min(viewport.width() - 32.0),
                (viewport.height() - 64.0).clamp(360.0, 700.0),
            ))
            .min_width(360.0_f32.min(viewport.width()))
            .min_height(360.0_f32.min(viewport.height()))
            .max_width((viewport.width() - frame_size.x).max(260.0))
            .max_height((viewport.height() - chrome_height).max(300.0))
            .vscroll(false)
            .constrain_to(viewport)
            .show(ctx, |ui| {
                self.search_query_controls(ui);
                let (hits, loaded) = match self.current_search_hits() {
                    Ok(hits) => (hits, true),
                    Err(error) => {
                        self.invalidate_search_review("查找范围或来源已失效，旧选择与预览已清空");
                        self.search_state.error = Some(error);
                        (Vec::new(), false)
                    }
                };
                self.reconcile_search_review(&hits);
                // Review owns keyboard traversal; source editing has an explicit returnable view.
                if self.edit_layer_is_top("search") {
                    ui.memory_mut(|memory| memory.set_modal_layer(ui.layer_id()));
                }
                egui::TopBottomPanel::bottom("search-review-actions")
                    .frame(egui::Frame::NONE)
                    .resizable(false)
                    .show_inside(ui, |ui| {
                        ui.separator();
                        close = self.search_review_actions(ui, &hits, loaded);
                    });
                self.search_review_pane(ui, &hits);
            });
        if !open || close {
            self.close_search(ctx);
        }
    }

    fn search_query_controls(&mut self, ui: &mut egui::Ui) {
        let input = ui.add(
            egui::TextEdit::singleline(&mut self.project_query)
                .id(review::query_id())
                .hint_text("字面查找文字 · 输入框内 Enter 定位下一处")
                .desired_width(f32::INFINITY),
        );
        if self.search_focus {
            input.request_focus();
            self.search_focus = false;
        }
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.search_state.scope, Scope::Current, "当前文稿");
            ui.selectable_value(&mut self.search_state.scope, Scope::Selection, "捕获选区");
            ui.selectable_value(&mut self.search_state.scope, Scope::Project, "工程文件");
            ui.checkbox(&mut self.search_state.replace, "替换");
        });
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut self.search_state.options.case_sensitive, "区分大小写");
            ui.checkbox(&mut self.search_state.options.whole_word, "整词");
            ui.checkbox(&mut self.search_state.source, "源码（保护项仅查找）");
            if self.search_state.scope == Scope::Project {
                ui.menu_button(
                    format!("文件与对象 · {} 文件", self.search_state.files.len()),
                    |ui| {
                        self.search_project_options(ui);
                    },
                );
            }
        });
        if self.search_state.replace {
            ui.horizontal(|ui| {
                ui.label("替换为");
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut self.search_state.replacement)
                            .id(egui::Id::new("author-search-replacement"))
                            .hint_text("仅普通正文，保留 ID、引用与标记")
                            .desired_width(f32::INFINITY),
                    )
                    .changed()
                {
                    self.invalidate_search_preview("替换文字已变化，请重新预览");
                }
            });
        }
    }

    fn search_review_actions(
        &mut self,
        ui: &mut egui::Ui,
        hits: &[SearchMatch],
        loaded: bool,
    ) -> bool {
        let replaceable = hits.iter().filter(|hit| hit.replaceable).count();
        ui.strong(format!(
            "{} 处命中 · {} 可替换 · {} 保护 · 已选 {} 处",
            hits.len(),
            replaceable,
            hits.len() - replaceable,
            self.search_state.chosen.len()
        ));
        let scope = match self.search_state.scope {
            Scope::Current => "当前文稿".to_owned(),
            Scope::Selection => "捕获选区".to_owned(),
            Scope::Project => format!("工程中显式选择的 {} 个文件", self.search_state.files.len()),
        };
        ui.small(if loaded {
            format!("{scope} · 本范围命中已全部载入，分页面审阅（上限 10000 处）")
        } else {
            format!("{scope} · 查找失败，未载入结果；请缩小范围或处理下方错误")
        });
        ui.horizontal_wrapped(|ui| {
            if crate::theme::add_enabled(ui, !hits.is_empty(), egui::Button::new("上一处"))
                .clicked()
            {
                self.navigate_search(ui.ctx(), true);
            }
            if crate::theme::add_enabled(ui, !hits.is_empty(), egui::Button::new("下一处"))
                .clicked()
            {
                self.navigate_search(ui.ctx(), false);
            }
            if crate::theme::add_enabled(ui, !hits.is_empty(), egui::Button::new("查看当前原文"))
                .clicked()
            {
                if let Some(hit) = hits.get(self.search_state.selected) {
                    self.show_search_source(ui.ctx(), hit);
                }
            }
            if let Some(hit) = hits.get(self.search_state.selected) {
                ui.small(format!(
                    "当前第 {} 处{}",
                    self.search_state.selected + 1,
                    if self.search_hit_is_chosen(hit) {
                        " · 已加入待改"
                    } else {
                        " · 未加入待改"
                    }
                ));
            }
        });
        if self.search_state.replace {
            ui.horizontal_wrapped(|ui| {
                let current = hits
                    .get(self.search_state.selected)
                    .filter(|hit| hit.replaceable);
                if crate::theme::add_enabled(ui, current.is_some(), egui::Button::new("只选当前处"))
                    .clicked()
                {
                    if let Some(hit) = current {
                        self.clear_search_choices();
                        self.choose_search_hit(hit, true);
                    }
                }
                if crate::theme::add_enabled(
                    ui,
                    replaceable > 0,
                    egui::Button::new(format!("全选本范围可替换 {replaceable} 处")),
                )
                .clicked()
                {
                    self.choose_all_search_hits();
                }
                if crate::theme::add_enabled(
                    ui,
                    !self.search_state.chosen.is_empty(),
                    egui::Button::new("清空选择"),
                )
                .clicked()
                {
                    self.clear_search_choices();
                }
            });
            ui.horizontal_wrapped(|ui| {
                if crate::theme::add_enabled(
                    ui,
                    !self.search_state.chosen.is_empty(),
                    egui::Button::new(format!("预览已选 {} 处", self.search_state.chosen.len())),
                )
                .clicked()
                {
                    self.preview_selected_search_replacement();
                }
                if let Some(plan) = &self.search_state.plan {
                    let label = format!("确认应用 {} 处", plan.hits.len());
                    let enabled = !plan.changes.is_empty();
                    if crate::theme::add_enabled(ui, enabled, crate::theme::primary(&label))
                        .clicked()
                    {
                        self.apply_search_replacement();
                    }
                    if ui.button("取消预览").clicked() {
                        self.cancel_search_preview();
                    }
                }
                ui.menu_button("更多", |ui| {
                    ui.label("原有全部替换：检查整个当前范围，含保护项时整批拒绝");
                    if crate::theme::add_enabled(
                        ui,
                        !hits.is_empty(),
                        egui::Button::new("预览本范围全部命中"),
                    )
                    .clicked()
                    {
                        self.preview_search_replacement();
                        ui.close();
                    }
                });
            });
        }
        if let Some(count) = self.search_state.applied_count {
            ui.label(format!(
                "已应用 {count} 处 · 可一次撤销 / 重做 · 尚未保存工程"
            ));
        } else if let Some(plan) = &self.search_state.plan {
            ui.small(format!(
                "待确认：{} 处 / {} 文件；跨文件应用会包含这些文件当前稿，尚未保存",
                plan.hits.len(),
                plan.changes.len()
            ));
        }
        if let Some(notice) = &self.search_state.review_notice {
            ui.small(notice);
        }
        if let Some(error) = &self.search_state.error {
            ui.colored_label(crate::theme::ERROR(), error);
        }
        ui.button("关闭查找 · Esc").clicked()
    }

    fn search_project_options(&mut self, ui: &mut egui::Ui) {
        ui.set_max_width(480.0);
        ui.label("明确选择文件；读取每个文件当前唯一草稿，不扩大到未知文件");
        egui::ScrollArea::vertical()
            .id_salt("search-files")
            .max_height(180.0)
            .show(ui, |ui| {
                let paths: Vec<_> = self
                    .project
                    .documents
                    .iter()
                    .filter(|(_, d)| !d.is_deleted())
                    .map(|(path, _)| path.clone())
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
                    }
                }
            });
        if !self.project_query.trim().is_empty() {
            self.search_object_candidates(ui);
        }
    }

    fn search_object_candidates(&mut self, ui: &mut egui::Ui) {
        let mut view = self.search_objects_in_current_drafts();
        ui.separator();
        ui.strong("对象 · 名称、别名、类型、ID和来源");
        if let Some(warning) = &view.warning {
            ui.colored_label(
                crate::theme::GOLD(),
                "已应用目录 · 当前稿不可解析；总数仅含已解析对象",
            )
            .on_hover_text(warning);
        } else {
            ui.small("当前稿目录");
        }
        if self.search_state.object_page.controls(ui) {
            view = self.search_objects_in_current_drafts();
        }
        match view.page {
            Ok(page) => {
                if page.total == 0 {
                    ui.label("没有匹配对象");
                }
                egui::ScrollArea::vertical()
                    .id_salt("object-search")
                    .max_height(180.0)
                    .show(ui, |ui| {
                        for object in &page.items {
                            if crate::app::object_picker::candidate_row_at_revision(
                                ui,
                                object,
                                Some(&self.project.root),
                                false,
                                self.search_state.object_page.serial,
                            )
                            .clicked()
                                && !view.stale
                            {
                                self.navigate_search_object(ui.ctx(), object, view.applied);
                            }
                        }
                    });
            }
            Err(error) => {
                ui.colored_label(crate::theme::ERROR(), error);
            }
        }
    }

    fn search_source_return_window(&mut self, ctx: &egui::Context) {
        match self.current_search_hits() {
            Ok(hits) => self.reconcile_search_review(&hits),
            Err(error) => {
                self.invalidate_search_review(
                    "来源或捕获选区已变化；旧选择与预览已清空，请返回后重新查找",
                );
                self.search_state.error = Some(error);
            }
        }
        egui::Window::new("查看命中原文")
            .id(egui::Id::new("search-source-return"))
            .anchor(egui::Align2::RIGHT_TOP, [-20.0, 100.0])
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(format!(
                    "审阅已保留 · 已选 {} 处",
                    self.search_state.chosen.len()
                ));
                if let Some(notice) = &self.search_state.review_notice {
                    ui.small(notice);
                }
                if let Some(error) = &self.search_state.error {
                    ui.colored_label(crate::theme::ERROR(), error);
                }
                if ui.button("返回审阅 · Alt+←").clicked() {
                    self.return_to_search_review(ctx);
                }
                if ui.button("关闭查找 · Esc").clicked() {
                    self.close_search(ctx);
                }
            });
    }
}
