use super::*;
use crate::theme;

impl ImportState {
    pub(super) fn render(&mut self, app: &mut WorldeditApp, ctx: &egui::Context) {
        let ime = app.ime_composing
            || app.command_palette.ime
            || app.command_palette.ime_frame
            || ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::Ime(_))));
        if !ime
            && !egui::Popup::is_any_open(ctx)
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            app.switch_tab(Tab::Catalog);
        }
        egui::CentralPanel::default().frame(theme::panel()).show(ctx, |ui| {
            egui::TopBottomPanel::bottom("catalog-import-actions").resizable(false)
                .show_inside(ui, |ui| self.footer(app, ui, ime));
            ui.heading("世界资料导入");
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(&mut self.step, Step::Mapping, "1 · 来源与列映射");
                ui.add_enabled_ui(self.plan.is_some(), |ui| {
                    ui.selectable_value(&mut self.step, Step::Review, "2 · 逐行审阅");
                });
            });
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(!ime, egui::Button::new("选择 CSV 快照…")).clicked() { self.choose_file(ctx); }
                if ui.add_enabled(!ime, egui::Button::new("返回资料库（保留输入）")).clicked() { app.switch_tab(Tab::Catalog); }
            });
            if !self.source_name.is_empty() {
                ui.add(egui::Label::new(format!("快照：{}", self.source_name)).truncate()).on_hover_text(&self.source_name);
                ui.collapsing("快照与写入边界", |ui| {
                    ui.add(egui::Label::new(&self.source_name).wrap());
                    ui.label("这是选择时的文件副本；修改原 CSV 后须重新选择。不持续监视原 CSV，也不修改它。");
                    ui.label("未映射资料、人物关系与其他声明保留；能力须显式启用。应用仅更新内存，保存仍由“保存全部”完成。");
                });
            }
            if let Some(job) = &self.job {
                ui.horizontal_wrapped(|ui| {
                    ui.spinner();
                    ui.label(if job.cancelled { "已取消；等待旧检查退出，结果不会采用" } else { "后台处理中 · 不会应用部分结果" });
                });
            }
            if self.stale { ui.colored_label(theme::WARNING(), "预览已过期 · 应用已锁定，请刷新预览后重新确认"); }
            if let Some(error) = &self.error { ui.add(egui::Label::new(egui::RichText::new(error).color(theme::ERROR())).wrap()); }
            if let Some(status) = &self.status {
                ui.add(egui::Label::new(status).wrap());
            }
            let blockers = app.catalog_import_blockers();
            if !blockers.is_empty() {
                ui.colored_label(theme::WARNING(), format!("未应用输入阻止预览与应用：{}", blockers.join("、")));
                if ui.button("回到未应用草稿").clicked() { app.return_to_export_input(blockers[0]); }
            }
            self.confirmations(ui, ctx, ime);
            ui.separator();
            match self.step {
                Step::Mapping => { egui::ScrollArea::vertical().id_salt("catalog-import-mapping")
                    .auto_shrink([false, false]).show(ui, |ui| self.mapping_ui(app, ui)); }
                Step::Review => self.review_ui(app, ui),
            }
        });
    }

    fn footer(&mut self, app: &mut WorldeditApp, ui: &mut egui::Ui, ime: bool) {
        ui.separator();
        if self.step == Step::Review && !self.submitted {
            ui.add_enabled_ui(!ime && !self.stale && self.job.is_none(), |ui| {
                ui.checkbox(&mut self.acknowledged, "已审阅整批字段差异与存档影响");
            });
        }
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !ime && self.job.is_none() && !self.source_name.is_empty(),
                    egui::Button::new(if self.table.is_none() {
                        "读取快照"
                    } else {
                        "刷新预览"
                    }),
                )
                .clicked()
            {
                self.preview(app, ui.ctx());
            }
            if self.job.is_some()
                && ui
                    .add_enabled(!ime, egui::Button::new("取消检查"))
                    .clicked()
            {
                if let Some(job) = &mut self.job {
                    job.cancel();
                }
                self.acknowledged = false;
                self.status =
                    Some("已请求取消；原生不可抢占的检查阶段结束前，不会启动另一批检查。".into());
            }
            if ui
                .add_enabled(!ime && self.can_apply(app), theme::primary("确认整批应用"))
                .clicked()
            {
                self.apply(app);
            }
            if self.submitted
                && ui
                    .add_enabled(
                        !ime && app.project.is_dirty(),
                        egui::Button::new("保存全部"),
                    )
                    .clicked()
            {
                app.save();
            }
            if ui
                .add_enabled(
                    !ime && (!self.source_name.is_empty() || self.replacement.is_some()),
                    egui::Button::new("丢弃导入输入…"),
                )
                .clicked()
            {
                self.discard_confirm = true;
            }
        });
        ui.label(theme::muted(
            "应用时会同步复核当前基线，可能短暂停顿；整批可一次撤销。Esc 返回并保留输入。",
        ));
    }

    fn confirmations(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, ime: bool) {
        if self.replacement.is_some() {
            ui.colored_label(
                theme::WARNING(),
                "已选择另一份 CSV；替换将清除当前列映射与预览，工程原文不变。",
            );
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(
                        !ime && self.job.is_none(),
                        egui::Button::new("确认替换导入快照"),
                    )
                    .clicked()
                {
                    if let Some((name, csv)) = self.replacement.take() {
                        self.accept_file(name, csv, ctx);
                    }
                }
                if ui
                    .add_enabled(!ime, egui::Button::new("保留当前快照"))
                    .clicked()
                {
                    self.replacement = None;
                }
            });
        }
        if self.discard_confirm {
            ui.colored_label(
                theme::WARNING(),
                "将丢弃此处 CSV 快照、映射和预览；已应用的工程修改不受影响。",
            );
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(!ime, egui::Button::new("确认丢弃导入输入"))
                    .clicked()
                {
                    self.discard();
                }
                if ui
                    .add_enabled(!ime, egui::Button::new("取消丢弃"))
                    .clicked()
                {
                    self.discard_confirm = false;
                }
            });
        }
    }
}
