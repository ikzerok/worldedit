//! 验证当前比较中的实际证据，再进入已有作者来源桥。
use super::ComparedRoutes;
use crate::app::WorldeditApp;
use worldline_core::evidence_source::EvidenceSource;

pub(super) struct NavigationAccess<'a> {
    pub blocked: Option<&'a str>,
    pub focus: super::focus::FocusReveal,
}

pub(super) struct ComparisonSourceRequest {
    pub result_id: u64,
    pub source: EvidenceSource,
}

fn contains_source(compared: &ComparedRoutes, source: &EvidenceSource) -> bool {
    let first = compared.result.alignment.first_difference.as_ref();
    first.is_some_and(|difference| {
        [
            difference.left.source.as_ref(),
            difference.right.source.as_ref(),
        ]
        .contains(&Some(source))
    }) || [&compared.result.left, &compared.result.right]
        .into_iter()
        .any(|side| {
            side.state_actions
                .records
                .iter()
                .any(|record| record.source.as_ref() == Some(source))
        })
}

pub(super) fn source_button(
    ui: &mut egui::Ui,
    compared: &ComparedRoutes,
    source: Option<&EvidenceSource>,
    access: &NavigationAccess<'_>,
    label: &str,
    request: &mut Option<ComparisonSourceRequest>,
) -> bool {
    let reason = access
        .blocked
        .or(source.is_none().then_some("此证据没有可确认的作者来源"));
    let response = ui.add_enabled(reason.is_none(), egui::Button::new(label));
    access.focus.reveal(ui, &response);
    let clicked = response.clicked();
    if clicked {
        *request = source.map(|source| ComparisonSourceRequest {
            result_id: compared.id,
            source: source.clone(),
        });
    }
    if let Some(reason) = reason {
        response.on_disabled_hover_text(reason);
    }
    if let Some(source) = source {
        let relative = crate::theme::relative_source(
            compared.scope.workspace_root(),
            std::path::Path::new(&source.file),
        );
        ui.add(egui::Label::new(format!("{relative} · 第 {} 行", source.line)).truncate())
            .on_hover_text(format!("{} · 第 {} 行", source.file, source.line));
    } else {
        ui.label(crate::theme::muted("没有可确认来源；不按同名或标签猜测"));
    }
    clicked
}

impl WorldeditApp {
    pub(super) fn jump_to_comparison_source(
        &mut self,
        ctx: &egui::Context,
        request: &ComparisonSourceRequest,
    ) {
        let hit = (|| {
            let compared = self
                .comparison
                .result
                .as_ref()
                .ok_or("比较结果已不存在，请重新比较")?;
            if compared.id != request.result_id || !contains_source(compared, &request.source) {
                return Err("证据已不属于当前比较结果，请重新选择动作".to_owned());
            }
            self.play_source_guard(&compared.scope)?;
            let hit = self.play_source_hit(&compared.scope, &request.source)?;
            self.project
                .verify_source_navigation(&hit.path, self.project.document(&hit.path)?)?;
            Ok(hit)
        })();
        match hit {
            Ok(hit) => {
                match self.go_author_source_position(ctx, &hit, true) {
                    Ok(()) => {
                        crate::app::search::mark_pending_selection_programmatic(ctx);
                        self.comparison.notice = None;
                        self.message = Some("已定位本次实际证据的语句头；Alt+Left 返回同一路线对照、选中项与阅读位置".into());
                    }
                    Err(reason) => self.comparison.notice = Some(reason),
                }
            }
            Err(reason) => self.comparison.notice = Some(reason),
        }
    }
}
