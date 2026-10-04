//! 两条真实路径的只读主内容对照；运行与含义由 runtime 统一提供。
mod details;
mod focus;
mod job;
mod navigation;
mod view;

use super::super::WorldeditApp;
use super::scope::AppliedPlayScope;
use worldline_runtime::{ReplayCancellation, RouteComparisonResult};

#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::app) struct ComparisonLocation {
    active: bool,
    result_id: Option<u64>,
    a: Option<usize>,
    b: Option<usize>,
    reversed: bool,
    selected_state: Option<String>,
    selected_action: Option<(bool, u64)>,
    scroll: f32,
    focus: Option<egui::Id>,
}

pub(super) struct ComparedRoutes {
    id: u64,
    scope: AppliedPlayScope,
    names: [String; 2],
    paths: [usize; 2],
    result: RouteComparisonResult,
    reversed: bool,
}
impl ComparedRoutes {
    fn display_index(&self, right: bool) -> usize {
        usize::from(right ^ self.reversed)
    }
    fn side(&self, right: bool) -> &worldline_runtime::RouteSideResult {
        if self.display_index(right) == 0 {
            &self.result.left
        } else {
            &self.result.right
        }
    }
    fn name(&self, right: bool) -> &str {
        &self.names[self.display_index(right)]
    }
    fn selections_match(&self, a: Option<usize>, b: Option<usize>) -> bool {
        a == Some(self.paths[self.display_index(false)])
            && b == Some(self.paths[self.display_index(true)])
    }
}

pub(super) struct ComparisonJob {
    id: u64,
    scope: AppliedPlayScope,
    names: [String; 2],
    paths: [usize; 2],
    cancellation: ReplayCancellation,
    #[cfg(not(target_arch = "wasm32"))]
    receiver: std::sync::mpsc::Receiver<Result<RouteComparisonResult, String>>,
    #[cfg(target_arch = "wasm32")]
    snapshot: worldline_core::CompileResult,
    #[cfg(target_arch = "wasm32")]
    session: worldline_runtime::RouteComparisonSession,
}
impl Drop for ComparisonJob {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

pub(in crate::app) struct ComparisonState {
    pub(in crate::app) active: bool,
    a: Option<usize>,
    b: Option<usize>,
    generation: u64,
    result: Option<ComparedRoutes>,
    job: Option<ComparisonJob>,
    pub(super) notice: Option<String>,
    max_steps: u64,
    time_budget_ms: u64,
    selected_state: Option<String>,
    selected_action: Option<(bool, u64)>,
    scroll: f32,
    restore_scroll: bool,
    restore_focus: Option<egui::Id>,
}
impl Default for ComparisonState {
    fn default() -> Self {
        Self {
            active: false,
            a: None,
            b: None,
            generation: 0,
            result: None,
            job: None,
            notice: None,
            max_steps: 100_000,
            time_budget_ms: 30_000,
            selected_state: None,
            selected_action: None,
            scroll: 0.0,
            restore_scroll: false,
            restore_focus: None,
        }
    }
}
impl ComparisonState {
    fn swap(&mut self) {
        std::mem::swap(&mut self.a, &mut self.b);
        if let Some(result) = &mut self.result {
            result.reversed = !result.reversed;
        }
    }
    pub(super) fn signature(&self, paths: &[super::super::SavedReplayPath]) -> String {
        let selected = [self.a, self.b].map(|index| {
            index
                .and_then(|index| paths.get(index))
                .map(|path| serde_json::json!([path.name, path.trace]))
        });
        serde_json::json!([
            self.a,
            self.b,
            selected,
            self.max_steps,
            self.time_budget_ms
        ])
        .to_string()
    }
    pub(super) fn running(&self) -> bool {
        self.job.is_some()
    }
    pub(super) fn has_paths(&self, count: usize) -> bool {
        self.a.is_some_and(|i| i < count) && self.b.is_some_and(|i| i < count)
    }
    pub(super) fn path_names(&self, paths: &[super::super::SavedReplayPath]) -> String {
        [self.a, self.b]
            .map(|index| {
                index
                    .and_then(|i| paths.get(i))
                    .map(|p| p.name.as_str())
                    .unwrap_or("未选择")
            })
            .join(" / ")
    }
}

impl WorldeditApp {
    pub(in crate::app) fn comparison_location(
        &self,
        ctx: Option<&egui::Context>,
    ) -> Option<ComparisonLocation> {
        (self.tab == super::super::Tab::Play).then(|| ComparisonLocation {
            active: self.comparison.active,
            result_id: self.comparison.result.as_ref().map(|r| r.id),
            a: self.comparison.a,
            b: self.comparison.b,
            reversed: self.comparison.result.as_ref().is_some_and(|r| r.reversed),
            selected_state: self.comparison.selected_state.clone(),
            selected_action: self.comparison.selected_action,
            scroll: self.comparison.scroll,
            focus: ctx.and_then(|ctx| ctx.memory(|m| m.focused())),
        })
    }
    pub(in crate::app) fn restore_comparison_location(
        &mut self,
        location: Option<ComparisonLocation>,
    ) {
        let Some(location) = location else { return };
        self.comparison.active = location.active;
        if location.result_id != self.comparison.result.as_ref().map(|r| r.id) {
            self.comparison.notice = Some("已返回路线对照；结果已更新，未恢复旧动作位置".into());
            return;
        }
        self.comparison.a = location.a;
        self.comparison.b = location.b;
        if let Some(result) = &mut self.comparison.result {
            result.reversed = location.reversed;
        }
        self.comparison.selected_state = location.selected_state;
        self.comparison.selected_action = location.selected_action;
        self.comparison.scroll = location.scroll.max(0.0);
        self.comparison.restore_scroll = true;
        self.comparison.restore_focus = location.focus;
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
