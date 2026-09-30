use super::super::WorldeditApp;
use super::*;
use egui::{RichText, TextEdit};

impl WorldeditApp {
    pub(in crate::app) fn reader_publish_window(&mut self, ctx: &egui::Context) {
        self.poll_reader_publish_job();
        #[cfg(not(target_arch = "wasm32"))]
        if self.reader_publish.job.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if !self.reader_publish.open {
            return;
        }

        let objects = self.reader_publish.object_choices.clone();
        let attachments = self.reader_publish.attachment_choices.clone();
        let manuscripts = self.reader_publish.manuscript_choices.clone();

        let mut action = None;
        let mut open = self.reader_publish.open;
        let state = &mut self.reader_publish;
        egui::Window::new("发布给读者")
            .id(egui::Id::new("reader-publish-window"))
            .open(&mut open)
            .resizable(true)
            .default_width(780.0)
            .default_height(980.0)
            .vscroll(true)
            .show(ctx, |ui| {
                ui.label(RichText::new("只生成明确选择的离线静态内容。完整工程备份仍保留原有全部文件。").color(crate::theme::MUTED));
                ui.label(RichText::new("离线选择不是权限认证；拿到阅读包的人可以查看包内全部内容。").strong().color(crate::theme::GOLD));
                if let Some(status) = &state.status {
                    ui.label(status);
                }
                let mut changed = false;
                ui.horizontal(|ui| {
                    ui.label("读者站点标题");
                    changed |= ui
                        .add(TextEdit::singleline(&mut state.site_title).desired_width(360.0))
                        .changed();
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("reader-publish-choices")
                    .max_height(440.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui.button("选择全部资料").clicked() {
                                state.objects = objects.iter().map(|o| o.target.clone()).collect();
                                changed = true;
                            }
                            if ui.button("清空资料选择").clicked() {
                                state.objects.clear();
                                changed = true;
                            }
                        });
                        ui.label("全部资料不包含地图、章节和附件；它们仍需逐项选择并核对。");
                        ui.heading(format!("资料对象（{}）", objects.len()));
                        for choice in &objects {
                            let mut selected = state.objects.contains(&choice.target);
                            let label = format!(
                                "{} · {} ({})",
                                choice.display, choice.target.kind, choice.target.id
                            );
                            if ui.checkbox(&mut selected, label).changed() {
                                if selected {
                                    state.objects.insert(choice.target.clone());
                                } else {
                                    state.objects.remove(&choice.target);
                                }
                                changed = true;
                            }
                        }
                        if objects.is_empty() {
                            ui.label("当前工程没有可选择的资料对象。");
                        }

                        ui.add_space(8.0);
                        ui.heading("书稿章节");
                        for book in &manuscripts {
                            ui.label(RichText::new(&book.title).strong());
                            if let Some(reason) = &book.unavailable {
                                ui.label(RichText::new(reason).color(crate::theme::GOLD));
                                continue;
                            }
                            for (chapter_id, chapter_title) in &book.chapters {
                                let selected = state
                                    .chapters
                                    .get(&book.id)
                                    .is_some_and(|chapters| chapters.contains(chapter_id));
                                let mut selected = selected;
                                if ui
                                    .checkbox(
                                        &mut selected,
                                        format!("{} ({})", chapter_title, chapter_id),
                                    )
                                    .changed()
                                {
                                    let chapters = state.chapters.entry(book.id.clone()).or_default();
                                    if selected {
                                        chapters.insert(chapter_id.clone());
                                    } else {
                                        chapters.remove(chapter_id);
                                    }
                                    changed = true;
                                }
                            }
                            if book.chapters.is_empty() {
                                ui.label("没有可选择的章节。");
                            }
                        }
                        if manuscripts.is_empty() {
                            ui.label("当前工程没有注册书稿。");
                        }

                        ui.add_space(8.0);
                        ui.heading(format!("附件（{}）", attachments.len()));
                        for choice in &attachments {
                            let mut selected = state.attachments.contains(&choice.id);
                            let label = format!("{} ({})", choice.display, choice.id);
                            let response = ui.add_enabled(
                                choice.available,
                                egui::Checkbox::new(&mut selected, label),
                            );
                            if response.changed() {
                                if selected {
                                    state.attachments.insert(choice.id.clone());
                                } else {
                                    state.attachments.remove(&choice.id);
                                }
                                changed = true;
                            }
                            if !choice.available {
                                ui.label(RichText::new("此附件当前不可用。").color(crate::theme::GOLD));
                            }
                        }
                        if attachments.is_empty() {
                            ui.label("当前工程没有已登记的附件。");
                        }
                        changed |= state.map_choices_ui(ui);
                    });
                if changed {
                    state.invalidate_review();
                }

                ui.separator();
                #[cfg(not(target_arch = "wasm32"))]
                ui.horizontal(|ui| {
                    ui.label("ZIP 目标（新文件，工作区外）");
                    let response = ui.add(
                        TextEdit::singleline(&mut state.destination)
                            .desired_width(300.0)
                            .hint_text("选择一个尚不存在的 .zip 文件"),
                    );
                    if response.changed() {
                        state.confirmed = false;
                    }
                    if ui.button("浏览…").clicked() {
                        action = Some(PublishAction::Browse);
                    }
                });
                #[cfg(target_arch = "wasm32")]
                ui.label("确认后将下载 `worldedit-reader-site.zip`；失败时会显示错误，不改变工程保存状态。");

                if let Some(reviewed) = &state.reviewed {
                    ui.group(|ui| {
                        ui.heading("作者只读预览");
                        ui.label(format!(
                            "{} 项公开条目 · {} 个静态文件 · {} B 原始文件 · {} B ZIP",
                            reviewed.preview.included.len(),
                            reviewed.files.len(),
                            reviewed.raw_bytes,
                            reviewed.zip.len()
                        ));
                        ui.label(format!(
                            "{} 项未纳入报告；预览计划 {} · 文件逐字节解包核对通过",
                            reviewed.preview.exclusions.len(),
                            reviewed.preview.plan_digest
                        ));
                        ui.label("只有下列公开索引内容会进入站点；排除报告仅供作者核对，不写入包。");
                        let mut confirmed = state.confirmed;
                        if ui.checkbox(
                            &mut confirmed,
                            "我已逐项核对预览，确认只发布以上离线内容（不代表在线权限控制）",
                        ).changed() {
                            state.confirmed = confirmed;
                        }
                        ui.horizontal(|ui| {
                            #[cfg(not(target_arch = "wasm32"))]
                            if ui.add_enabled(state.confirmed, egui::Button::new("发布 ZIP")).clicked() {
                                action = Some(PublishAction::Publish);
                            }
                            #[cfg(target_arch = "wasm32")]
                            if ui.add_enabled(state.confirmed, egui::Button::new("下载阅读包")).clicked() {
                                action = Some(PublishAction::Publish);
                            }
                            if ui.button("取消发布").clicked() {
                                action = Some(PublishAction::Cancel);
                            }
                        });
                        ui.separator();
                        for (title, url, text) in reviewed.public_pages.iter().take(8) {
                            ui.label(RichText::new(title).strong());
                            ui.label(format!("{url} · {text}"));
                        }
                        if reviewed.public_pages.len() > 8 {
                            ui.label(format!("另有 {} 页……", reviewed.public_pages.len() - 8));
                        }
                        egui::CollapsingHeader::new(format!(
                            "查看排除明细（{} 项；仅供作者核对）",
                            reviewed.preview.exclusions.len()
                        ))
                        .default_open(false)
                        .show(ui, |ui| {
                            for exclusion in &reviewed.preview.exclusions {
                                ui.label(RichText::new(format!(
                                    "排除：{} · {}",
                                    exclusion.reason_code,
                                    exclusion.source_path.as_deref().unwrap_or("未公开内容")
                                )).color(crate::theme::MUTED));
                            }
                        });
                    });
                } else {
                    ui.label("尚未生成预览；未选任何内容时不会创建空包。");
                }

                ui.horizontal(|ui| {
                    #[cfg(target_arch = "wasm32")]
                    let busy = false;
                    #[cfg(not(target_arch = "wasm32"))]
                    let busy = state.job.is_some();
                    if ui
                        .add_enabled(!busy && state.has_selection(), egui::Button::new("生成 / 更新预览"))
                        .clicked()
                    {
                        action = Some(PublishAction::Preview(state.selection()));
                    }
                    if busy && ui.button("取消生成").clicked() {
                        action = Some(PublishAction::Cancel);
                    }
                    if ui.button("取消发布").clicked() {
                        action = Some(PublishAction::Cancel);
                    }
                });
            });
        self.reader_publish.open = open;
        if !open {
            self.reader_publish = ReaderPublishState::default();
            return;
        }

        match action {
            Some(PublishAction::Preview(selection)) => self.start_reader_publish_preview(selection),
            Some(PublishAction::Cancel) => self.reader_publish = ReaderPublishState::default(),
            Some(PublishAction::Publish) => self.publish_reader_package(),
            #[cfg(not(target_arch = "wasm32"))]
            Some(PublishAction::Browse) => self.choose_reader_package_destination(),
            None => {}
        }
    }
}
