//! 新建普通试玩会话的已应用快照。
use super::super::{PlayState, WorldeditApp};
use worldline_core::{Analysis, Program};
use worldline_runtime::Story;

impl WorldeditApp {
    pub(super) fn start_play_inner(&mut self, scope: super::scope::AppliedPlayScope) {
        self.play_keyboard.cancel();
        if !scope.matches_project(&self.project, self.version) {
            self.replay_debugger.notice =
                Some("已应用源码与编译快照不一致，请重新编译后运行".into());
            return;
        }
        let Some(snap) = &self.snapshot else { return };
        if snap.result.has_errors() {
            return;
        }
        self.play_keyboard.new_session();
        self.replay_debugger.explanations = None;
        let source_catalog = snap.result.analysis.catalog.clone();
        let source_wiki = worldline_core::wiki::KeywordIndex::new(&snap.result);
        let entry_diagnostics = worldline_core::analysis::execution_diagnostics(
            &snap.result.program,
            &snap.result.analysis,
        );
        // 泄漏快照换取 'static 生命周期(点击级频率;见 PlayState 注释)
        let leaked: &'static (Program, Analysis) = Box::leak(Box::new((
            snap.result.program.clone(),
            snap.result.analysis.clone(),
        )));
        match Story::new_with_seed(&leaked.0, &leaked.1, self.replay_debugger.seed) {
            Ok(story) => {
                self.play = Some(PlayState {
                    story: Some(story),
                    transcript: String::new(),
                    transcript_links: Vec::new(),
                    ended: false,
                    error: None,
                    version: scope.version,
                    scope,
                    source_catalog,
                    source_wiki,
                    paused: false,
                    stopped: false,
                    interruption: None,
                    entry_diagnostics,
                });
                self.play_scroll_bottom = true;
            }
            Err(e) => {
                self.play = Some(PlayState {
                    story: None,
                    transcript: String::new(),
                    transcript_links: Vec::new(),
                    ended: true,
                    error: Some(e.to_string()),
                    version: scope.version,
                    scope,
                    source_catalog,
                    source_wiki,
                    paused: true,
                    stopped: false,
                    interruption: None,
                    entry_diagnostics,
                });
            }
        }
    }
}
