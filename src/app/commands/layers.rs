//! 按打开顺序跟踪临时层和受保护表单，不能因下层表单存在而拦截上层取消。
use crate::app::{Tab, WorldeditApp};

impl WorldeditApp {
    pub(in crate::app) fn edit_layer_is_top(&self, kind: &str) -> bool {
        self.command_palette
            .focus_stack
            .last()
            .is_some_and(|(top, _)| *top == kind)
    }

    pub(in crate::app) fn sync_edit_layers(&mut self, ctx: &egui::Context) {
        let active: Vec<_> = [
            ("guard-entity", self.entity_editor.is_some()),
            ("guard-wiki", self.wiki_editor.is_some()),
            ("guard-relation", self.relation_editor.is_some()),
            ("guard-relation-type", self.relation_type_editor.is_some()),
            ("guard-delete", self.delete_form.is_some()),
            ("guard-rename", self.rename_form.is_some()),
            ("guard-source-move", self.source_move_form.is_some()),
            ("guard-preset", self.preset_editor.is_some()),
            ("guard-markdown", self.markdown_import_wizard.is_some()),
            (
                "character-index",
                self.tab == Tab::Characters && self.character_focus.index_open,
            ),
            (
                "character-details",
                self.tab == Tab::Characters && self.character_focus.inspector_open,
            ),
            (
                "problems",
                self.personal.settings.diagnostics && !self.personal.settings.focus,
            ),
            ("temporal-issues", self.temporal_issues.open),
            ("search", self.search_open),
            ("commands", self.command_palette.open),
            ("preferences", self.personal.preferences_open),
            ("reader", self.reader_publish.open),
            ("schema", self.schema_ui.open),
            ("guard-export", self.export_confirmation.is_some()),
            ("guard-play", self.play_confirmation.is_some()),
            ("guard-capabilities", self.capability_ui.is_some()),
            ("guard-draft-exit", self.draft_action.is_some()),
            ("guard-pending", self.pending.is_some()),
            ("guard-directory", self.directory.is_some()),
            ("guard-new-file", self.new_file.is_some()),
            ("guard-period", self.new_period.is_some()),
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
            let focus = match kind {
                "search" => self.search_return_focus(),
                "commands" => self.command_palette.previous_focus,
                "schema" => self.schema_ui.return_focus,
                "temporal-issues" => self.temporal_issues.return_focus,
                "problems" => self
                    .problems
                    .return_focus
                    .or(self.command_palette.frame_focus),
                // 命令执行后关闭自身并打开设置：新层应继承命令的返回目标，
                // 不能记录本帧开始时已经消失的命令输入框。
                _ => closed_focus.or(self.command_palette.frame_focus),
            };
            self.command_palette.focus_stack.push((kind, focus));
        }
    }
}
