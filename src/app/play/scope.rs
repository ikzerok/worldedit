//! 试玩与严格重放的已应用快照决定点；只读检查不推进运行、不提交草稿。
use super::super::{export_scope::UnappliedInput, WorldeditApp};
use crate::theme;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use worldline_core::project::Project;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::app) struct AppliedPlayScope {
    pub version: u64,
    pub sources: BTreeMap<PathBuf, String>,
    pub excluded_inputs: Vec<UnappliedInput>,
    options: worldline_core::CompileOptions,
    root: PathBuf,
    baseline: String,
}
impl AppliedPlayScope {
    pub(super) fn workspace_root(&self) -> &Path {
        &self.root
    }

    pub(in crate::app) fn source_matches(&self, path: &Path, source: &str) -> bool {
        self.sources.get(path).is_some_and(|saved| saved == source)
    }

    pub(in crate::app) fn matches_project(&self, project: &Project, version: u64) -> bool {
        self.version == version
            && self.root == project.root
            && self.baseline == project.content_baseline()
            && self.options == project.compile_options()
            && self.sources == project.sources()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunAction {
    Start,
    Replay,
    Compare,
}

pub(in crate::app) struct PlayConfirmation {
    action: RunAction,
    scope: AppliedPlayScope,
    draft_signature: String,
    execution_signature: String,
    refreshed: bool,
}

pub(super) fn render_scope(ui: &mut egui::Ui, scope: &AppliedPlayScope) {
    ui.horizontal_wrapped(|ui| {
        ui.label(format!("实际运行：已应用工程稿 #{}", scope.version));
        ui.label(theme::muted(format!(
            "语言 {}",
            scope.options.language_version.as_str()
        )));
    });
    if !scope.excluded_inputs.is_empty() {
        ui.colored_label(
            theme::WARNING(),
            format!(
                "此次运行未纳入启动时的 {} 项草稿；输入仍保留。",
                scope.excluded_inputs.len()
            ),
        );
    }
    egui::CollapsingHeader::new(format!("稿件范围 · {} 个源文件", scope.sources.len()))
        .id_salt(("applied-play-sources", scope.version))
        .show(ui, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("applied-play-scope-details")
                .max_height(160.0)
                .show(ui, |ui| {
                    for input in &scope.excluded_inputs {
                        ui.label(format!("未纳入：{} · {}", input.kind, input.source));
                    }
                    for path in scope.sources.keys() {
                        ui.label(
                            path.strip_prefix(&scope.root)
                                .unwrap_or(path)
                                .display()
                                .to_string(),
                        );
                    }
                });
        });
}

impl WorldeditApp {
    pub(in crate::app) fn unapplied_play_inputs(&self) -> Vec<UnappliedInput> {
        // 纯展示/编排/交换设置不改变本次执行；源码及结构表单保守纳入。
        let mut inputs: Vec<_> = self
            .unapplied_export_inputs()
            .into_iter()
            .filter(|input| {
                !matches!(
                    input.kind,
                    "书稿 / 正文草稿"
                        | "地图草稿"
                        | "新建地图"
                        | "展示预设"
                        | "共享网络布局"
                        | "共享查询定义"
                        | "审阅批注"
                        | "审阅提案"
                        | "工程模板草稿"
                        | "本地化草稿"
                        | "文件名称"
                        | "源码路径"
                )
            })
            .collect();
        // WritingBuffer 以源文件为唯一身份；同源多章不重复，编排变化不阻断。
        for buffer in self
            .manuscript
            .writing_buffers()
            .iter()
            .filter(|b| b.is_changed())
        {
            inputs.push(UnappliedInput {
                kind: "书稿 / 正文草稿",
                source: buffer
                    .path()
                    .strip_prefix(&self.project.root)
                    .unwrap_or(buffer.path())
                    .display()
                    .to_string(),
            });
        }
        inputs.sort_by(|a, b| (a.kind, &a.source).cmp(&(b.kind, &b.source)));
        inputs.dedup();
        inputs
    }

    pub(in crate::app::play) fn applied_play_scope(&self) -> Option<AppliedPlayScope> {
        let snapshot = self.snapshot.as_ref()?;
        (!snapshot.result.has_errors()).then(|| AppliedPlayScope {
            version: self.version,
            sources: snapshot.result.sources.clone(),
            excluded_inputs: self.unapplied_play_inputs(),
            options: snapshot.result.options,
            root: self.project.root.clone(),
            baseline: self.project.content_baseline(),
        })
    }

    fn play_draft_signature(&self) -> String {
        let mut values = BTreeMap::new();
        for input in self.unapplied_play_inputs() {
            let signature = match input.kind {
                "世界资料导入" => Some(self.catalog_import.input_signature()),
                "事件正文与分支" => self.event_editor.as_ref().map(|f| {
                    let d = &f.draft;
                    serde_json::json!([
                        f.path,
                        f.original,
                        f.baseline,
                        d.id,
                        d.summary,
                        d.storyline,
                        d.characters,
                        d.order,
                        d.period,
                        d.predecessors,
                        d.perm,
                        d.after,
                        format!("{:?}", d.effects),
                        d.body
                    ])
                    .to_string()
                }),
                "人物资料" => self.character_editor.as_ref().map(|f| {
                    let d = &f.draft;
                    serde_json::json!([
                        f.path,
                        f.original,
                        d.id,
                        d.display,
                        d.properties,
                        d.relations
                    ])
                    .to_string()
                }),
                "世界观" => self.world_editor.as_ref().map(|d| {
                    serde_json::json!([d.id, d.display, d.description, d.properties]).to_string()
                }),
                "实体资料" => self
                    .entity_editor
                    .as_ref()
                    .map(|f| serde_json::json!([f.path, f.original, f.draft]).to_string()),
                "语义关系" => self
                    .relation_editor
                    .as_ref()
                    .map(|f| serde_json::json!([f.original, f.draft]).to_string()),
                "关系类型" => self
                    .relation_type_editor
                    .as_ref()
                    .map(|f| format!("{:?}", (&f.original, &f.draft))),
                "标签" => self.tag_editor.as_ref().map(|(id, d)| {
                    serde_json::json!([id, d.id, d.display, d.description, d.properties])
                        .to_string()
                }),
                "状态" => self.state_editor.as_ref().map(|(id, d)| {
                    serde_json::json!([id, d.id, d.display, d.target, d.tags]).to_string()
                }),
                "锚点" => self
                    .anchor_editor
                    .as_ref()
                    .map(|(id, d)| format!("{id:?}:{d:?}")),
                "时段资料" => Some(format!("{:?}", self.new_period)),
                "对象重命名" => self
                    .rename_form
                    .as_ref()
                    .map(|f| serde_json::json!([f.target, f.new_id]).to_string()),
                "Wiki词条" => self.wiki_editor.as_ref().map(|f| f.play_draft_signature()),
                "持续资料约束草稿" => Some(self.schema_ui.play_draft_signature()),
                "正文替换预览" => Some(self.play_search_signature()),
                "Markdown 导入草稿" => self
                    .markdown_import_wizard
                    .as_ref()
                    .map(|f| f.play_draft_signature()),
                "正在输入的源码 / 输入法" => Some(format!(
                    "{:?}",
                    (
                        self.ime_composing,
                        &self.ime_source_draft,
                        &self.ime_source_baseline
                    )
                )),
                "书稿 / 正文草稿" => Some(format!(
                    "{:?}",
                    self.manuscript
                        .writing_buffers()
                        .iter()
                        .filter(|b| b.is_changed())
                        .map(|b| (
                            b.path().to_owned(),
                            (
                                b.baseline().to_owned(),
                                b.generation(),
                                b.source().to_owned()
                            )
                        ))
                        .collect::<BTreeMap<_, _>>()
                )),
                _ => Some(input.source.clone()),
            };
            values.insert((input.kind, input.source), signature);
        }
        format!("{values:?}")
    }

    fn execution_signature(&self, action: RunAction) -> String {
        match action {
            RunAction::Start => format!(
                "{}:{}:{}",
                self.replay_debugger.seed,
                self.replay_debugger.live_max_steps,
                self.replay_debugger.live_time_budget_ms
            ),
            RunAction::Compare => self.comparison.signature(&self.replay_debugger.saved_paths),
            RunAction::Replay => serde_json::json!([
                self.replay_debugger.selected_path,
                self.replay_debugger.selected_path.and_then(|index| {
                    self.replay_debugger
                        .saved_paths
                        .get(index)
                        .map(|path| serde_json::json!([path.name, path.trace]))
                }),
                self.replay_debugger.max_steps,
                self.replay_debugger.time_budget_ms
            ])
            .to_string(),
        }
    }

    fn play_confirmation_for(&self, action: RunAction) -> Option<PlayConfirmation> {
        Some(PlayConfirmation {
            action,
            scope: self.applied_play_scope()?,
            draft_signature: self.play_draft_signature(),
            execution_signature: self.execution_signature(action),
            refreshed: false,
        })
    }

    pub(super) fn request_play(&mut self) {
        let Some(confirmation) = self.play_confirmation_for(RunAction::Start) else {
            return;
        };
        if confirmation.scope.excluded_inputs.is_empty() {
            self.start_play_inner(confirmation.scope);
        } else {
            self.play_confirmation = Some(confirmation);
        }
    }

    pub(super) fn request_replay(&mut self, ctx: &egui::Context) {
        if self.replay_debugger.job.is_some() {
            return;
        }
        if self
            .replay_debugger
            .selected_path
            .and_then(|index| self.replay_debugger.saved_paths.get(index))
            .is_none()
        {
            self.replay_debugger.notice = Some("请先选择或导入一条路径".into());
            return;
        }
        let Some(confirmation) = self.play_confirmation_for(RunAction::Replay) else {
            self.replay_debugger.notice = Some("已应用工程稿没有可用的编译快照".into());
            return;
        };
        if confirmation.scope.excluded_inputs.is_empty() {
            self.begin_replay_applied(ctx, confirmation.scope);
        } else {
            self.play_confirmation = Some(confirmation);
        }
    }

    pub(super) fn request_comparison(&mut self, ctx: &egui::Context) {
        if self.comparison.running() {
            return;
        }
        if !self
            .comparison
            .has_paths(self.replay_debugger.saved_paths.len())
        {
            self.comparison.notice = Some("请先为 A、B 选择或导入真实路径".into());
            return;
        }
        let Some(confirmation) = self.play_confirmation_for(RunAction::Compare) else {
            self.comparison.notice = Some("已应用工程稿没有可用的编译快照".into());
            return;
        };
        if confirmation.scope.excluded_inputs.is_empty() {
            self.begin_comparison_applied(ctx, confirmation.scope);
        } else {
            self.play_confirmation = Some(confirmation);
        }
    }

    fn refresh_play_confirmation(&self, old: &PlayConfirmation) -> Option<PlayConfirmation> {
        let mut current = self.play_confirmation_for(old.action)?;
        current.refreshed = old.refreshed
            || current.scope != old.scope
            || current.draft_signature != old.draft_signature
            || current.execution_signature != old.execution_signature;
        Some(current)
    }

    fn run_scope_notice(&mut self, action: RunAction, message: &str) {
        if action == RunAction::Compare {
            self.comparison.notice = Some(message.into());
        } else {
            self.replay_debugger.notice = Some(message.into());
        }
    }

    fn confirm_play_scope(&mut self, ctx: &egui::Context, shown: PlayConfirmation) {
        let Some(mut current) = self.refresh_play_confirmation(&shown) else {
            self.run_scope_notice(
                shown.action,
                "已应用工程稿已变化且存在错误，请修复后重新运行",
            );
            return;
        };
        if current.scope != shown.scope
            || current.draft_signature != shown.draft_signature
            || current.execution_signature != shown.execution_signature
        {
            current.refreshed = true;
            self.play_confirmation = Some(current);
            return;
        }
        match shown.action {
            RunAction::Start => self.start_play_inner(shown.scope),
            RunAction::Replay => self.begin_replay_applied(ctx, shown.scope),
            RunAction::Compare => self.begin_comparison_applied(ctx, shown.scope),
        }
    }

    pub(in crate::app) fn play_scope_dialog(&mut self, ctx: &egui::Context) {
        let Some(old) = self.play_confirmation.take() else {
            return;
        };
        let Some(confirmation) = self.refresh_play_confirmation(&old) else {
            self.run_scope_notice(old.action, "已应用工程稿存在编译错误，运行已取消；草稿保留");
            return;
        };
        let changed = confirmation.scope != old.scope
            || confirmation.draft_signature != old.draft_signature
            || confirmation.execution_signature != old.execution_signature;
        let mut proceed = false;
        let mut cancel = false;
        let mut return_to = None;
        egui::Modal::new(egui::Id::new("play-applied-snapshot")).show(ctx, |ui| {
            ui.set_width(590.0);
            ui.heading("运行已应用工程稿");
            ui.label(match confirmation.action {
                RunAction::Start => format!("开始 / 重新开始 · seed {}", self.replay_debugger.seed),
                RunAction::Compare => format!(
                    "共同已应用稿路线对照 · {}",
                    self.comparison
                        .path_names(&self.replay_debugger.saved_paths)
                ),
                RunAction::Replay => format!(
                    "严格重放 · {}",
                    self.replay_debugger
                        .selected_path
                        .and_then(|index| self.replay_debugger.saved_paths.get(index))
                        .map(|path| path.name.as_str())
                        .unwrap_or("所选路径已失效")
                ),
            });
            ui.label(format!(
                "将运行已应用工程稿 #{}；已应用但未保存的修改也会参与。",
                confirmation.scope.version
            ));
            ui.label("以下未应用输入不会参与。不会自动应用、保存或清空草稿。");
            if confirmation.refreshed {
                ui.colored_label(
                    theme::WARNING(),
                    "工程、草稿或运行设置已变化；请重新核对当前范围。",
                );
            }
            egui::ScrollArea::vertical()
                .max_height(260.0)
                .show(ui, |ui| {
                    for input in &confirmation.scope.excluded_inputs {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(format!("{} · {}", input.kind, input.source));
                            if ui.small_button("返回处理").clicked() {
                                return_to = Some(input.kind);
                            }
                        });
                    }
                });
            if confirmation.scope.excluded_inputs.is_empty() {
                ui.label("当前已无执行相关的未应用输入；仍需重新确认本次范围。");
            }
            ui.horizontal_wrapped(|ui| {
                proceed = ui
                    .add_enabled(!changed, theme::primary("明确运行已应用稿"))
                    .clicked();
                cancel = ui.button("取消运行").clicked();
            });
        });
        if let Some(kind) = return_to {
            self.return_to_export_input(kind);
        } else if proceed {
            self.confirm_play_scope(ctx, confirmation);
        } else if !cancel {
            self.play_confirmation = Some(confirmation);
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
