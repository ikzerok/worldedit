use crate::app::WorldeditApp;
fn replacement_shortcut(
    os: egui::os::OperatingSystem,
) -> (egui::Modifiers, egui::Key, &'static str) {
    if os == egui::os::OperatingSystem::Mac {
        (
            egui::Modifiers::COMMAND | egui::Modifiers::ALT,
            egui::Key::F,
            "⌘⌥F",
        )
    } else {
        (egui::Modifiers::COMMAND, egui::Key::H, "Ctrl+H")
    }
}
impl WorldeditApp {
    pub(in crate::app) fn edit_undo(&mut self, forward: bool) {
        if !self.search_draft_undo(forward) && !self.search_project_undo(forward) {
            self.undo(forward);
        }
        self.message = None;
    }
    pub(in crate::app) fn edit_shortcuts(&mut self, ctx: &egui::Context) {
        self.command_palette.frame_focus = ctx.memory(|m| m.focused());
        let text_focus = ctx
            .memory(|m| m.focused())
            .filter(|id| egui::TextEdit::load_state(ctx, *id).is_some());
        self.command_palette.edit_focus = text_focus;
        let (replace_modifiers, replace_key, _) = replacement_shortcut(ctx.os());
        if ctx.input_mut(|i| {
            i.consume_key(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::F,
            )
        }) {
            self.open_search(ctx, true, false);
        } else if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::F)) {
            self.open_search(ctx, false, false);
        } else if ctx.input_mut(|i| i.consume_key(replace_modifiers, replace_key)) {
            self.open_search(ctx, false, true);
        }
        if self.search_open {
            let previous =
                ctx.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, egui::Key::Enter));
            let next = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
            if previous || next {
                self.navigate_search(ctx, previous);
            }
        }
        if text_focus.is_none() {
            if ctx.input_mut(|i| {
                i.consume_key(
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                    egui::Key::Z,
                )
            }) || ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Y))
            {
                self.edit_undo(true);
            } else if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) {
                self.edit_undo(false);
            }
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if egui::Popup::is_any_open(ctx) {
                return;
            }

            let top = self.command_palette.focus_stack.last().map(|entry| entry.0);
            let blocked = self.entity_editor.is_some()
                || self.wiki_editor.is_some()
                || self.relation_editor.is_some()
                || self.relation_type_editor.is_some()
                || self.delete_form.is_some()
                || self.rename_form.is_some()
                || self.preset_editor.is_some()
                || self.markdown_import_wizard.is_some()
                || self.draft_action.is_some()
                || self.pending.is_some();
            if blocked && !matches!(top, Some("schema" | "reader")) {
                self.message = Some("上层表单有待处理输入，请使用其明确取消或应用操作".into());
                return;
            }
            let closed = match top {
                Some("schema") => {
                    self.schema_ui.close(ctx);
                    true
                }
                Some("reader") => {
                    self.reader_publish.open = false;
                    true
                }
                Some("preferences") => {
                    self.personal.preferences_open = false;
                    true
                }
                Some("commands") => {
                    self.command_palette.open = false;
                    true
                }
                Some("search") => {
                    self.close_search(ctx);
                    true
                }
                _ if self.search_open => {
                    self.close_search(ctx);
                    true
                }
                _ if self.personal.catalog_drawer_open => {
                    self.personal.catalog_drawer_open = false;
                    true
                }
                _ => false,
            };
            if closed {
                ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
                if let Some((_, Some(id))) = self.command_palette.focus_stack.pop() {
                    ctx.memory_mut(|m| m.request_focus(id));
                }
            }
        }
    }
    pub(in crate::app) fn capture_edit_focus(&mut self, ctx: &egui::Context) {
        self.refresh_search_return_focus(ctx);
        let active: Vec<_> = [
            ("search", self.search_open),
            ("commands", self.command_palette.open),
            ("preferences", self.personal.preferences_open),
            ("reader", self.reader_publish.open),
            ("schema", self.schema_ui.open),
        ]
        .into_iter()
        .filter_map(|(kind, open)| open.then_some(kind))
        .collect();
        let closed_focus = self
            .command_palette
            .focus_stack
            .last()
            .filter(|(kind, _)| !active.contains(kind))
            .and_then(|(_, focus)| *focus);
        self.command_palette
            .focus_stack
            .retain(|(kind, _)| active.contains(kind));
        let new: Vec<_> = active
            .into_iter()
            .filter(|kind| {
                !self
                    .command_palette
                    .focus_stack
                    .iter()
                    .any(|(known, _)| known == kind)
            })
            .collect();
        if new.is_empty() {
            if let Some(id) = closed_focus {
                ctx.memory_mut(|m| m.request_focus(id));
            }
        }
        for kind in new {
            let focus = if kind == "search" {
                self.search_return_focus()
            } else if kind == "commands" {
                self.command_palette.previous_focus
            } else {
                self.command_palette.frame_focus
            };
            self.command_palette.focus_stack.push((kind, focus));
        }
        if ctx.input(|i| {
            i.events
                .iter()
                .any(|event| matches!(event, egui::Event::Text(_) | egui::Event::Paste(_)))
        }) {
            self.message = None;
        }
    }
    pub(in crate::app) fn edit_menu(&mut self, ui: &mut egui::Ui) {
        let mac = ui.ctx().os() == egui::os::OperatingSystem::Mac;
        let command = if mac { "⌘" } else { "Ctrl+" };
        let (_, _, replace_label) = replacement_shortcut(ui.ctx().os());
        ui.menu_button("编辑", |ui| {
            for (label, forward, key) in [("撤销", false, "Z"), ("重做", true, "Shift+Z")] {
                if ui.button(format!("{label}  {command}{key}")).clicked() {
                    if let Some(id) = self.command_palette.edit_focus {
                        ui.memory_mut(|m| m.request_focus(id));
                        ui.input_mut(|i| {
                            i.events.push(egui::Event::Key {
                                key: egui::Key::Z,
                                physical_key: None,
                                pressed: true,
                                repeat: false,
                                modifiers: if forward {
                                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT
                                } else {
                                    egui::Modifiers::COMMAND
                                },
                            })
                        });
                    } else {
                        self.edit_undo(forward);
                    }
                    ui.close();
                }
            }
            ui.separator();
            if ui.button(format!("查找当前稿  {command}F")).clicked() {
                self.open_search(ui.ctx(), false, false);
                ui.close();
            }
            if ui.button(format!("替换正文  {replace_label}")).clicked() {
                self.open_search(ui.ctx(), false, true);
                ui.close();
            }
            if ui
                .button(format!("查找工程文件  {command}Shift+F"))
                .clicked()
            {
                self.open_search(ui.ctx(), true, false);
                ui.close();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replacement_shortcut_avoids_native_platform_global_commands() {
        for os in [
            egui::os::OperatingSystem::Nix,
            egui::os::OperatingSystem::Windows,
        ] {
            let (modifiers, key, label) = replacement_shortcut(os);
            assert_eq!(modifiers, egui::Modifiers::COMMAND);
            assert_eq!(key, egui::Key::H);
            assert_eq!(label, "Ctrl+H");
        }
        let (modifiers, key, label) = replacement_shortcut(egui::os::OperatingSystem::Mac);
        assert_eq!(modifiers, egui::Modifiers::COMMAND | egui::Modifiers::ALT);
        assert_eq!(key, egui::Key::F);
        assert_eq!(label, "⌘⌥F");
    }
}
