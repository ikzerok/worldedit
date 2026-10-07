use super::*;

impl Wizard {
    pub(super) fn contents(
        &mut self,
        ui: &mut egui::Ui,
        app: &mut WorldeditApp,
        _ctx: &egui::Context,
    ) {
        ui.label("步骤 1 来源　→　步骤 2 只读预检　→　步骤 3 确认并应用");
        ui.label("预检不写入文件；应用时会重新检查来源与工程基线。取消不会创建半成品工程。");
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            if ui
                .radio_value(
                    &mut self.target_mode,
                    TargetMode::NewProject,
                    "导入到新工程",
                )
                .changed()
            {
                self.invalidate_preview();
            }
            if ui
                .radio_value(
                    &mut self.target_mode,
                    TargetMode::CurrentProject,
                    "导入到当前工程",
                )
                .changed()
            {
                self.invalidate_preview();
            }
        });

        ui.label("Markdown 来源目录");
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (changed, pick) =
                directory_path_row(ui, &mut self.source_root, "选择 Markdown 来源目录…");
            if changed {
                self.invalidate_preview();
            }
            if pick {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.source_root = path.display().to_string();
                    self.source_files = None;
                    self.invalidate_preview();
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        ui.horizontal(|ui| {
            ui.label(if self.source_root.is_empty() {
                "尚未选择文件夹".to_string()
            } else {
                self.source_root.clone()
            });
            if ui.button("选择 Markdown 文件夹…").clicked() {
                self.invalidate_preview();
                crate::web::select_files(_ctx, true, "", crate::web::FileAction::MarkdownImport);
            }
        });

        match self.target_mode {
            TargetMode::NewProject => {
                #[cfg(not(target_arch = "wasm32"))]
                ui.label("新工程目录（须已存在且为空）");
                #[cfg(target_arch = "wasm32")]
                ui.label("应用后切换到新浏览器工程；当前工程不会在预检或取消时改变。");
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let (changed, pick) =
                        directory_path_row(ui, &mut self.target_root, "选择空工程目录…");
                    if changed {
                        self.invalidate_preview();
                    }
                    if pick {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            self.target_root = path.display().to_string();
                            self.invalidate_preview();
                        }
                    }
                }
            }
            TargetMode::CurrentProject => {
                #[cfg(not(target_arch = "wasm32"))]
                ui.label(format!("目标工程：{}", app.project.root.display()));
                #[cfg(target_arch = "wasm32")]
                ui.label("目标工程：当前浏览器工程");
                if !app.saved_location {
                    ui.colored_label(
                        crate::theme::resolved(ui.ctx()).colors.danger,
                        "当前工程尚未保存到工作区；请先另存工程，或选择导入到新工程。",
                    );
                } else if app.project.is_dirty() || app.has_open_authoring_form() {
                    ui.colored_label(
                        crate::theme::resolved(ui.ctx()).colors.danger,
                        "当前工程有未保存的草稿；先保存或完成草稿，再开始迁移。",
                    );
                }
            }
        }

        ui.horizontal_wrapped(|ui| {
            ui.label("迁移命名空间");
            let changed = ui
                .add(
                    egui::TextEdit::singleline(&mut self.namespace)
                        .hint_text("留空时由来源路径稳定生成")
                        .desired_width(190.0),
                )
                .changed();
            if changed {
                self.invalidate_preview();
            }
            if ui.button("预检导入").clicked() {
                self.preview(app);
            }
            if ui.button("取消").clicked() {
                self.closed = true;
            }
        });

        if let Some(error) = &self.error {
            ui.colored_label(crate::theme::resolved(ui.ctx()).colors.danger, error);
        }
        let Some(plan) = self.plan.take() else {
            return;
        };

        ui.separator();
        ui.label(format!(
            "预检完成：{} 个页面、{} 个页面链接、{} 个附件、{} 个损失、{} 个阻塞冲突。",
            plan.pages.len(),
            plan.links.len(),
            plan.attachments.len(),
            plan.losses.len(),
            plan.conflicts.len()
        ));
        ui.label(format!(
            "命名空间：{}　·　输出文件：{}　·　来源指纹：{}",
            plan.namespace,
            plan.files.len(),
            plan.source_fingerprint
        ));
        if self.stale {
            ui.colored_label(
                crate::theme::resolved(ui.ctx()).colors.danger,
                "预检已过期，应用已锁定。请重新预检后再确认。",
            );
        }
        if plan.requires_language_upgrade {
            ui.checkbox(
                &mut self.allow_language_upgrade,
                "我已检查目标语言版本升级及其影响",
            );
        }
        if !plan.losses.is_empty() {
            ui.checkbox(&mut self.accept_losses, "我已检查并接受预览中的损失");
        }

        let can_apply = !self.stale
            && plan.conflicts.is_empty()
            && (plan.losses.is_empty() || self.accept_losses)
            && (!plan.requires_language_upgrade || self.allow_language_upgrade);
        if can_apply {
            ui.label("必需确认已完成，可以应用。");
        } else {
            let mut blockers = Vec::new();
            if self.stale {
                blockers.push("预检已过期，请重新预检".to_string());
            }
            if !plan.conflicts.is_empty() {
                blockers.push(format!("仍有 {} 项阻塞冲突", plan.conflicts.len()));
            }
            if !plan.losses.is_empty() && !self.accept_losses {
                blockers.push("尚未确认损失".into());
            }
            if plan.requires_language_upgrade && !self.allow_language_upgrade {
                blockers.push("尚未确认语言升级".into());
            }
            ui.colored_label(
                crate::theme::resolved(ui.ctx()).colors.danger,
                format!("暂不能应用：{}。", blockers.join("；")),
            );
        }
        let show_refresh = self.stale;
        let mut should_apply = false;
        let mut should_preview = false;
        let mut should_close = false;
        ui.horizontal_wrapped(|ui| {
            should_apply =
                crate::theme::add_enabled(ui, can_apply, egui::Button::new("应用导入")).clicked();
            if show_refresh {
                should_preview = ui.button("重新预检").clicked();
            }
            should_close = ui.button("取消").clicked();
        });

        egui::ScrollArea::vertical()
            .id_salt(("markdown-import-details", self.preview_generation))
            .auto_shrink([false, false])
            .max_height(ui.available_height().max(180.0))
            .show(ui, |ui| self.show_plan_sections(ui, &plan));
        self.plan = Some(plan);
        if should_apply {
            self.apply(app);
        } else if should_preview {
            self.preview(app);
        }
        if should_close {
            self.closed = true;
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn directory_path_row(ui: &mut egui::Ui, path: &mut String, hint: &str) -> (bool, bool) {
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // 先分配按钮实际宽度，再让输入框填充剩余空间（含其边距）。
            // 固定减去一个估算宽度会让窗口在每次重绘时反向撑大。
            let pick = ui.button("选择目录…").clicked();
            let changed = ui
                .add(
                    egui::TextEdit::singleline(path)
                        .hint_text(hint)
                        .desired_width(f32::INFINITY),
                )
                .changed();
            (changed, pick)
        })
        .inner
    })
    .inner
}
