//! 普通试玩的预算控件；与重放预算保持独立。
use super::super::ReplayDebugger;
use worldline_runtime::ContinuationOutcome;

pub(super) fn render_live_budget(
    ui: &mut egui::Ui,
    debugger: &mut ReplayDebugger,
    keyboard: &super::keyboard::PlayKeyboard,
) {
    let budget = egui::CollapsingHeader::new("普通试玩预算（每次推进）")
        .id_salt("live-play-budget")
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("步数上限");
                keyboard.setting(ui, "live-steps", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut debugger.live_max_steps).range(1..=1_000_000_000),
                    )
                });
            });
            ui.horizontal(|ui| {
                ui.label("时限 ms");
                keyboard.setting(ui, "live-time", |ui| {
                    ui.add(egui::DragValue::new(&mut debugger.live_time_budget_ms).range(1..=2_000))
                });
            });
            ui.label("超限会暂停，已执行的输出和状态保留；可继续或调高预算，不会重放副作用。");
        });
    keyboard.reveal_setting(ui, &budget.header_response);
}

pub(super) fn interruption_text(outcome: ContinuationOutcome) -> &'static str {
    match outcome {
        ContinuationOutcome::StepBudgetExceeded => "达到步数上限，已暂停；可调高预算或继续。",
        ContinuationOutcome::TimeBudgetExceeded => "达到时间上限，已暂停；可调高预算或继续。",
        ContinuationOutcome::Cancelled => "试玩已停止；已发生的输出与状态保留。",
        ContinuationOutcome::Choice => "等待选择。",
        ContinuationOutcome::Ended => "故事正常结束。",
    }
}
