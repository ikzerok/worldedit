use super::Navigation;
use crate::app::WorldeditApp;

impl WorldeditApp {
    pub(in crate::app) fn navigate_condition_authoring(
        &mut self,
        ctx: &egui::Context,
        request: Navigation,
    ) {
        let composing =
            super::input::event_composing(ctx, &self.project.root, self.event_editor.is_some());
        if self.ime_composing || composing {
            return;
        }
        match request {
            Navigation::Owner(target) => self.open_condition_object_inspection(ctx, target),
            Navigation::Definition(source) => {
                let result = (|| {
                    let compiled = &self
                        .snapshot
                        .as_ref()
                        .ok_or("条件来源编译快照不可用")?
                        .result;
                    let resolved =
                        worldline_core::state_inspection_source::resolve_state_inspection_source(
                            compiled, &source,
                        )?;
                    let text = self.project.document(&resolved.path)?;
                    if compiled.sources.get(&resolved.path).map(String::as_str) != Some(text) {
                        return Err("状态声明已改变；没有按同名或行号跳到新稿".into());
                    }
                    self.project
                        .verify_source_navigation(&resolved.path, text)?;
                    let preview = text
                        .get(resolved.range.clone())
                        .ok_or("状态声明范围已改变")?
                        .to_owned();
                    self.go_author_source_position_after_local_ime_guard(
                        ctx,
                        &worldline_core::search_replace::SearchMatch {
                            path: resolved.path,
                            range: resolved.range,
                            line: resolved.line,
                            column: resolved.column,
                            preview,
                            context: None,
                            identity: None,
                            replaceable: false,
                            draft: false,
                        },
                        true,
                    )
                })();
                self.message = Some(match result {
                    Ok(()) => {
                        "已定位条件所引用的真实状态定义；Alt+Left 返回条件作者位置，输入仍保留"
                            .into()
                    }
                    Err(error) => error,
                });
            }
        }
    }
}
