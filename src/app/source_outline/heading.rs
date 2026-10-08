//! 仅按当前可用视口收束页头；光标、名称和诊断内容不参与布局选择。
use super::*;
use egui::emath::GuiRounding;
use std::path::Path;

fn compact(ui: &egui::Ui, line_height: f32) -> bool {
    let row = ui
        .text_style_height(&egui::TextStyle::Heading)
        .max(ui.spacing().interact_size.y);
    let context = ui.text_style_height(&egui::TextStyle::Body) * 2.0;
    let heading_budget = row * 3.0 + context + crate::theme::SPACE_SM;
    ui.available_width() < 640.0 || ui.available_height() < heading_budget + line_height * 3.0
}

impl WorldeditApp {
    /// 返回是否将辅助信息收入菜单，供调用方保持同一正文预算。
    pub(in crate::app) fn source_outline_heading(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        path: &Path,
        relative: &str,
    ) -> bool {
        // 导航动作可能在本帧切换文件；只校验绘制开始时的真实来源身份。
        debug_assert_eq!(path, self.active_file);
        self.refresh_source_outline(false);
        let current = self.source_outline_current(ctx);
        let subtitle = if self.source_outline_blocked(ctx) {
            "编辑位置 · 输入法组合或未提交稿，结构定位暂不可用".to_owned()
        } else if let Some(cache) = &self.source_outline.cache {
            if cache.outline.status != SourceOutlineStatus::Ready {
                format!(
                    "编辑位置 · {}",
                    cache
                        .outline
                        .message
                        .as_deref()
                        .unwrap_or("当前稿结构暂不可用")
                )
            } else if let Some(entry) = current.and_then(|index| cache.outline.entries.get(index)) {
                format!("编辑位置 · {}\n{}", entry.display, metadata(entry))
            } else {
                "编辑位置 · 文件正文 / 空白（无所属声明）".into()
            }
        } else {
            "编辑位置 · 当前稿结构暂不可用".into()
        };
        let appearance = self.personal.appearance();
        let compact = compact(ui, appearance.source_size * appearance.line_spacing);
        if compact {
            self.compact_source_heading(ctx, ui, path, relative, &subtitle);
        } else {
            self.source_jump_heading(ctx, ui, relative);
            // 保持原有两行预算；光标变动不能挤走源码的鼠标坐标和滚动视口。
            let context_height = ui.text_style_height(&egui::TextStyle::Body) * 2.0;
            egui::ScrollArea::vertical()
                .id_salt(("source-outline-context", path))
                .max_height(context_height)
                .min_scrolled_height(context_height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.add(egui::Label::new(crate::theme::muted(&subtitle)).wrap());
                });
            ui.add_space(crate::theme::SPACE_SM);
            ui.horizontal_wrapped(|ui| self.source_heading_actions(ctx, ui, false));
        }
        compact
    }

    fn compact_source_heading(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        path: &Path,
        relative: &str,
        subtitle: &str,
    ) {
        let height = ui
            .text_style_height(&egui::TextStyle::Heading)
            .max(ui.spacing().interact_size.y)
            .round_to_pixels(ui.pixels_per_point());
        let menu_width = (ui.text_style_height(&egui::TextStyle::Button) * 4.0
            + ui.spacing().button_padding.x * 2.0)
            .min(ui.available_width() * 0.4)
            .round_to_pixels(ui.pixels_per_point());
        let (row, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), height),
            egui::Sense::hover(),
        );
        let menu = egui::Rect::from_min_max(
            egui::pos2(row.right() - menu_width, row.top()),
            row.right_bottom(),
        );
        // 无/有/失效问题始终占同一槽位，状态文字不挤动文件、行列和正文。
        let status_width = (ui.text_style_height(&egui::TextStyle::Body) * 3.5)
            .min(row.width() * 0.2)
            .round_to_pixels(ui.pixels_per_point());
        let status = egui::Rect::from_min_max(
            egui::pos2(
                menu.left() - ui.spacing().item_spacing.x - status_width,
                row.top(),
            ),
            egui::pos2(menu.left() - ui.spacing().item_spacing.x, row.bottom()),
        );
        let title = egui::Rect::from_min_max(
            row.min,
            egui::pos2(
                (status.left() - ui.spacing().item_spacing.x).max(row.left()),
                row.bottom(),
            ),
        );
        let mut heading_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("compact-source-position")
                .max_rect(title),
        );
        heading_ui.set_clip_rect(ui.clip_rect().intersect(title));
        self.source_jump_compact_heading(ctx, &mut heading_ui, relative);
        if let Some((label, color)) = self.problem_source_status(path) {
            let mut status_ui = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt("compact-source-problem-status")
                    .max_rect(status)
                    .layout(egui::Layout::left_to_right(egui::Align::Center)),
            );
            status_ui.set_clip_rect(ui.clip_rect().intersect(status));
            status_ui
                .add(egui::Label::new(egui::RichText::new(label).color(color)).truncate())
                .on_hover_text("源码工具中可查看完整问题原因、位置状态并回到问题详情");
        }
        let mut menu_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt("compact-source-tools")
                .max_rect(menu)
                .layout(egui::Layout::left_to_right(egui::Align::Center)),
        );
        menu_ui.set_clip_rect(ui.clip_rect().intersect(menu));
        menu_ui
            .menu_button("源码工具", |ui| {
                ui.set_max_width((ctx.screen_rect().width() - 32.0).min(420.0));
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                egui::ScrollArea::vertical()
                    .id_salt(("compact-source-details", path))
                    .max_height((ctx.screen_rect().height() - 32.0).max(0.0))
                    .min_scrolled_height(0.0)
                    .show(ui, |ui| {
                        self.source_heading_actions(ctx, ui, true);
                        ui.separator();
                        ui.add(egui::Label::new(relative).wrap());
                        ui.add(egui::Label::new(crate::theme::muted(subtitle)).wrap());
                        self.problem_source_summary(ui, path);
                    });
            })
            .response
            .on_hover_text("本文件结构、返回作者位置、批注与完整文件/编辑位置上下文");
    }

    fn source_heading_actions(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, menu: bool) {
        let shortcut = if ctx.os() == egui::os::OperatingSystem::Mac {
            "⌘⇧O"
        } else {
            "Ctrl+Shift+O"
        };
        if ui.button(format!("本文件结构  {shortcut}")).clicked() {
            if menu {
                ui.close();
            }
            self.open_source_outline(ctx);
        }
        if crate::theme::add_enabled(
            ui,
            !self.personal.history.is_empty() && !self.source_outline_blocked(ctx),
            egui::Button::new("返回作者位置"),
        )
        .on_hover_text("Alt+← · 原文版本一致才恢复选区和滚动")
        .clicked()
        {
            if menu {
                ui.close();
            }
            self.author_back(ctx);
        }
        if crate::theme::add_enabled(
            ui,
            !self.ime_composing,
            egui::Button::new("为当前选区添加批注"),
        )
        .clicked()
        {
            if menu {
                ui.close();
            }
            self.comment_current_selection(ctx);
        }
        ui.label(crate::theme::muted("Ctrl+S 保存"))
            .on_hover_text("当前缓冲区与整个工程一起编译；Ctrl+Enter 打开源码引用或按选中文本建档；Ctrl+S 保存全部文件");
    }
}
