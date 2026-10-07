use super::*;
use egui::{Event, ImeEvent, Key};

impl PageDirectory {
    pub(in super::super) fn directory_ui(
        &mut self,
        ui: &mut egui::Ui,
        preview: &ReaderExportPreview,
        height: f32,
        keyboard_allowed: bool,
    ) {
        let top = ui.cursor().top();
        let search_id = ui.make_persistent_id("reader-page-query");
        let commands = self.keyboard(ui, search_id, keyboard_allowed);
        ui.heading("页面目录 / 查找");
        ui.label("只查找本次已审核的公开标题、正文与相对路径；不改变公开选择。");
        let search = ui.add(
            egui::TextEdit::singleline(&mut self.query)
                .id(search_id)
                .hint_text("查找公开页面")
                .desired_width(f32::INFINITY),
        );
        if std::mem::take(&mut self.focus_search) {
            search.request_focus();
        }
        let changed = self.search();
        let mut open = None;
        if !changed {
            for command in commands {
                match command {
                    Key::ArrowDown => self.move_candidate(true),
                    Key::ArrowUp => self.move_candidate(false),
                    Key::Enter => open = self.candidate.clone(),
                    _ => {}
                }
            }
        }
        ui.label(format!(
            "公开页总数 {} · 当前匹配 {}",
            preview.content.len(),
            self.matches.len()
        ));
        ui.horizontal_wrapped(|ui| {
            if crate::theme::add_enabled(ui, self.page > 0, egui::Button::new("上一组结果"))
                .clicked()
            {
                self.set_page(self.page - 1);
            }
            ui.label(format!(
                "目录第 {} / {} 页 · 每页至多 {PAGE_SIZE} 项",
                if self.matches.is_empty() {
                    0
                } else {
                    self.page + 1
                },
                self.page_count()
            ));
            if crate::theme::add_enabled(
                ui,
                self.page + 1 < self.page_count(),
                egui::Button::new("下一组结果"),
            )
            .clicked()
            {
                self.set_page(self.page + 1);
            }
        });
        if self.matches.is_empty() {
            ui.label("当前公开页中没有匹配内容");
        }
        self.result_ids.clear();
        let remaining = (height - (ui.cursor().top() - top)).max(40.0);
        egui::ScrollArea::vertical()
            .id_salt(("reader-directory-results", self.page, &self.cached_query))
            .max_height(remaining)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for offset in self.range() {
                    let page = &preview.content[self.matches[offset]];
                    ui.push_id(&page.output_path, |ui| {
                        let selected = self.candidate.as_ref() == Some(&page.output_path);
                        let row = ui.add(
                            egui::Button::selectable(
                                selected,
                                format!(
                                    "{}\n{}{}",
                                    page.title,
                                    page.output_path,
                                    if page.empty_content {
                                        "\n空正文"
                                    } else {
                                        ""
                                    }
                                ),
                            )
                            .wrap(),
                        );
                        if row.clicked() {
                            self.candidate = Some(page.output_path.clone());
                        }
                        if selected && self.scroll_candidate {
                            row.scroll_to_me(Some(egui::Align::Center));
                        }
                        if selected && self.focus_candidate {
                            row.request_focus();
                        }
                        self.result_ids.push((row.id, page.output_path.clone()));
                        let button = ui.button("打开此页");
                        if button.clicked() {
                            self.candidate = Some(page.output_path.clone());
                            open = Some(page.output_path.clone());
                        }
                        self.result_ids.push((button.id, page.output_path.clone()));
                        ui.separator();
                    });
                }
            });
        self.scroll_candidate = false;
        self.focus_candidate = false;
        if let Some(path) = open {
            self.open(&path);
        }
    }

    // 在TextEdit和原生按钮处理事件之前消费目录导航；Tab/Shift+Tab与鼠标不被接管。
    fn keyboard(&mut self, ui: &egui::Ui, search_id: egui::Id, allowed: bool) -> Vec<Key> {
        let allowed = allowed && ui.is_enabled();
        let mut ime_frame = false;
        ui.input(|input| {
            for event in &input.events {
                if let Event::Ime(event) = event {
                    ime_frame = true;
                    self.composing = matches!(event, ImeEvent::Enabled | ImeEvent::Preedit(_));
                }
            }
        });
        let focused = ui.memory(|memory| memory.focused());
        let result = self.result_ids.iter().find(|(id, _)| Some(*id) == focused);
        if focused != Some(search_id) && result.is_none() {
            return Vec::new();
        }
        if let Some((_, path)) = result {
            self.candidate = Some(path.clone());
        }
        let in_results = result.is_some();
        let mut keys = Vec::new();
        ui.input_mut(|input| {
            input.events.retain(|event| {
                if let Event::Key {
                    key,
                    pressed: true,
                    repeat,
                    modifiers,
                    ..
                } = event
                {
                    let navigation =
                        matches!(key, Key::ArrowUp | Key::ArrowDown) && modifiers.is_none();
                    if navigation || *key == Key::Enter {
                        if allowed
                            && !self.composing
                            && !ime_frame
                            && !repeat
                            && modifiers.is_none()
                        {
                            keys.push(*key);
                        }
                        return false;
                    }
                    if in_results
                        && *key == Key::Space
                        && (!allowed || self.composing || ime_frame || *repeat)
                    {
                        return false;
                    }
                }
                true
            });
        });
        self.focus_candidate = in_results
            && keys
                .iter()
                .any(|key| matches!(key, Key::ArrowDown | Key::ArrowUp));
        keys
    }
}
