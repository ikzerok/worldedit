use super::*;

impl ReaderPublishState {
    pub(super) fn review_summary(&self, ui: &mut egui::Ui) {
        let Some(reviewed) = &self.reviewed else {
            ui.label("尚未生成预览；未选任何内容时不会创建空包。");
            return;
        };
        ui.heading("作者只读预览");
        ui.label(format!(
            "{} 项公开条目 · {} 个静态文件 · {} B 原始文件 · {} B ZIP",
            reviewed.preview.included.len(),
            reviewed.files.len(),
            reviewed.raw_bytes,
            reviewed.zip.len()
        ));
        ui.label("文件逐字节解包核对通过；排除报告仅供作者核对，不写入包。");
        egui::CollapsingHeader::new("审核计划详情").show(ui, |ui| {
            crate::theme::technical_value(ui, "计划", &reviewed.preview.plan_digest);
            crate::theme::technical_value(ui, "基线", &reviewed.preview.content_baseline);
        });
    }

    pub(super) fn resource_review_ui(&mut self, ui: &mut egui::Ui) {
        self.review_summary(ui);
        let Some(reviewed) = &self.reviewed else {
            return;
        };
        let range =
            selection_ui::page_range(ui, &mut self.resource_page, reviewed.preview.included.len());
        for included in &reviewed.preview.included[range] {
            ui.label(format!("✓ {} · {}", included.title, included.output_path));
        }
        for page in &reviewed.preview.content {
            if page.empty_content {
                ui.colored_label(
                    crate::theme::WARNING(),
                    format!("{}：空正文；可返回逐项选择公开属性。", page.title),
                );
            }
        }
        egui::CollapsingHeader::new(format!(
            "查看排除明细（{} 项；仅供作者核对）",
            reviewed.preview.exclusions.len()
        ))
        .show(ui, |ui| {
            for exclusion in &reviewed.preview.exclusions {
                ui.add(
                    egui::Label::new(crate::theme::muted(format!(
                        "排除：{} · {}",
                        exclusion.reason_code,
                        exclusion.source_path.as_deref().unwrap_or("未公开内容")
                    )))
                    .wrap(),
                );
            }
        });
        egui::CollapsingHeader::new("实际静态文件与附件").show(ui, |ui| {
            let range = selection_ui::page_range(ui, &mut self.files_page, reviewed.files.len());
            for (path, bytes) in reviewed.files.iter().skip(range.start).take(range.len()) {
                ui.label(format!("{} · {} B", path.display(), bytes.len()));
            }
        });
    }

    pub(super) fn page_review_ui(&mut self, ui: &mut egui::Ui) -> Option<PublishAction> {
        self.review_summary(ui);
        let Some(reviewed) = &self.reviewed else {
            return None;
        };
        ui.label("下面是核心公开文本投影，静态条件和效果仅供阅读，不执行故事。");
        #[cfg(not(target_arch = "wasm32"))]
        let mut action = None;
        #[cfg(target_arch = "wasm32")]
        let action = None;
        let count = reviewed.preview.content.len();
        self.preview_page = self.preview_page.min(count.saturating_sub(1));
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(self.preview_page > 0, egui::Button::new("上一阅读页"))
                .clicked()
            {
                self.preview_page -= 1;
            }
            ui.label(format!(
                "阅读页 {} / {count}（全部页面可逐页核对）",
                if count == 0 { 0 } else { self.preview_page + 1 }
            ));
            if ui
                .add_enabled(
                    self.preview_page + 1 < count,
                    egui::Button::new("下一阅读页"),
                )
                .clicked()
            {
                self.preview_page += 1;
            }
        });
        for page in reviewed
            .preview
            .content
            .iter()
            .skip(self.preview_page)
            .take(1)
        {
            egui::CollapsingHeader::new(format!("{} · {}", page.title, page.output_path))
                .id_salt(&page.output_path)
                .default_open(true)
                .show(ui, |ui| {
                    #[cfg(not(target_arch = "wasm32"))]
                    if ui.button("实际页面临时预览").clicked() {
                        action = Some(PublishAction::BrowserPreview(page.output_path.clone()));
                    }
                    if page.empty_content {
                        ui.colored_label(crate::theme::WARNING(), "此页没有静态阅读正文。");
                    } else {
                        ui.add(egui::Label::new(&page.text).wrap());
                    }
                    if let Some(bytes) = reviewed.files.get(std::path::Path::new(&page.output_path))
                    {
                        egui::CollapsingHeader::new("核对实际HTML文件").show(ui, |ui| {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(String::from_utf8_lossy(bytes)).monospace(),
                                )
                                .wrap(),
                            );
                        });
                    }
                });
        }
        action
    }

    pub(super) fn generation_ui(&mut self, ui: &mut egui::Ui) -> Option<PublishAction> {
        self.review_summary(ui);
        ui.add_space(crate::theme::SPACE_XL);
        if self.profile.as_ref().is_none_or(|profile| {
            profile.selection.schema_version
                == worldline_core::reader_export::READER_SITE_SCHEMA_VERSION
        }) {
            ui.label("本次对象选择包括全部别名与允许的类型结构；字段和附件仍按逐项白名单。");
        }
        #[cfg(not(target_arch = "wasm32"))]
        let mut action = None;
        #[cfg(target_arch = "wasm32")]
        let action = None;
        #[cfg(not(target_arch = "wasm32"))]
        {
            ui.label("ZIP目标（工作区外，尚不存在的新文件）");
            if ui
                .add(egui::TextEdit::singleline(&mut self.destination).desired_width(f32::INFINITY))
                .changed()
            {
                self.confirmed = false;
            }
            if ui.button("浏览…").clicked() {
                action = Some(PublishAction::Browse);
            }
        }
        #[cfg(target_arch = "wasm32")]
        ui.label("确认后请求浏览器下载worldedit-reader-site.zip；最终保存位置由浏览器管理。");
        ui.checkbox(
            &mut self.confirmed,
            "我已逐项核对预览，确认只发布以上离线内容（不代表在线权限控制）",
        );
        action
    }
}
