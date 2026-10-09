//! Real core failure/recovery through rendered filters; query errors never own operation status.
use super::focus_tests::{ctrl_a, draw};
use super::workbench_tests::{click, fixture, stage};
use super::*;
use egui::{Event, Key, Modifiers};

const QUERY_ERROR: &str = "来源前缀必须是工作区内相对路径";

struct Harness {
    ctx: egui::Context,
    project: Project,
    state: LocalizationUiState,
    baseline: String,
}

impl Harness {
    fn new() -> Self {
        let (project, mut state) = fixture(85);
        catalog::refresh(&project, &mut state, 1);
        jobs::settle(&project, &mut state, 1);
        stage(&mut state, "未提交译文😀");
        state.string_ids = "line0".into();
        state.exchange_json = "{完整未提交 JSON".into();
        state
            .workbench
            .id_inputs
            .insert("retained-other-unit".into(), "pending-id".into());
        let baseline = project.content_baseline();
        Self {
            ctx: egui::Context::default(),
            project,
            state,
            baseline,
        }
    }

    fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        draw(&self.ctx, &mut self.project, &mut self.state, events)
    }

    fn rendered(&mut self) -> String {
        self.frame(vec![])
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.text().to_owned()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn replace(&mut self, role: &str, value: &str) {
        self.frame(vec![]);
        self.frame(vec![]);
        let id = egui::Id::new((role, &self.project.root));
        let response = self.ctx.read_response(id).expect("rendered query field");
        assert!(response.enabled() && self.ctx.screen_rect().contains_rect(response.rect));
        let point = response.rect.center();
        for pressed in [true, false] {
            self.frame(vec![
                Event::PointerMoved(point),
                Event::PointerButton {
                    pos: point,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                },
            ]);
        }
        assert_eq!(self.ctx.memory(|memory| memory.focused()), Some(id));
        ctrl_a(&self.ctx, &mut self.project, &mut self.state);
        if value.is_empty() {
            // One frame deletes the selected prefix; do not pump the new query until requested.
            self.frame(vec![Event::Key {
                key: Key::Backspace,
                physical_key: Some(Key::Backspace),
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }]);
        } else {
            self.frame(vec![Event::Text(value.into())]);
        }
        let actual = match role {
            "localization-source-filter" => &self.state.workbench.source_prefix,
            "localization-search" => &self.state.workbench.search,
            _ => unreachable!(),
        };
        assert_eq!(actual, value);
    }

    fn settle(&mut self) {
        self.frame(vec![]);
        jobs::settle(&self.project, &mut self.state, 1);
        self.frame(vec![]);
        assert!(!self.state.jobs.pending());
    }

    fn fail_query(&mut self) {
        self.replace("localization-source-filter", "../outside");
        self.settle();
        assert_eq!(
            self.state.workbench.query_error.as_deref(),
            Some(QUERY_ERROR)
        );
        assert!(self.rendered().contains(QUERY_ERROR));
    }

    fn fail_import(&mut self) -> String {
        preview_import(&self.project, &mut self.state);
        jobs::settle(&self.project, &mut self.state, 1);
        assert!(self.state.import_plan.is_none());
        let error = self
            .state
            .status
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap_err()
            .clone();
        assert_ne!(error, QUERY_ERROR);
        assert!(self.rendered().contains(&error));
        error
    }

    fn inputs(&self) -> serde_json::Value {
        let drafts: Vec<_> = self
            .state
            .workbench
            .drafts
            .iter()
            .map(|(key, draft)| {
                (
                    key,
                    &draft.source_locale,
                    &draft.target_locale,
                    &draft.source_baseline,
                    &draft.edit,
                )
            })
            .collect();
        serde_json::json!({
            "drafts": drafts,
            "ids": self.state.workbench.id_inputs,
            "json": self.state.exchange_json,
            "selection": self.state.selection(),
            "source_locale": self.state.source_locale,
            "target_locale": self.state.target_locale
        })
    }

    fn assert_unchanged(&self, inputs: &serde_json::Value) {
        assert_eq!(&self.inputs(), inputs);
        assert!(self.state.has_unsubmitted_work());
        assert_eq!(self.project.content_baseline(), self.baseline);
        assert!(!self.project.root.exists());
    }
}

#[test]
fn localization_catalog_corrected_query_removes_resolved_error_and_returns_accurate_pages() {
    let mut h = Harness::new();
    let inputs = h.inputs();
    click(&h.ctx, &mut h.project, &mut h.state, "下一页");
    h.settle();
    assert!(h.rendered().contains("41–80 / 85"));
    h.fail_query();
    assert!(
        h.rendered().contains("41–80 / 85"),
        "failure retains the last stable page"
    );
    h.replace("localization-source-filter", "");
    h.settle();
    let page = h.state.workbench.page.as_ref().unwrap();
    assert_eq!(
        (page.total, page.all_total, page.entries.len()),
        (85, 85, 40)
    );
    assert!(h.state.workbench.query_error.is_none());
    let recovered = h.rendered();
    assert!(recovered.contains("1–40 / 85"));
    assert!(
        !recovered.contains(QUERY_ERROR),
        "accepted recovery still paints the old query error: {recovered}"
    );
    assert!(h.state.status.is_none());
    h.replace("localization-search", "line84");
    h.settle();
    let page = h.state.workbench.page.as_ref().unwrap();
    assert_eq!((page.total, page.all_total), (1, 85));
    assert_eq!(page.entries[0].id.as_deref(), Some("line84"));
    let final_page = h.rendered();
    assert!(final_page.contains("1–1 / 1") && !final_page.contains(QUERY_ERROR));
    h.assert_unchanged(&inputs);
}

#[test]
fn localization_catalog_failure_and_recovery_preserve_unrelated_import_error() {
    let mut h = Harness::new();
    let inputs = h.inputs();
    let import_error = h.fail_import();
    h.fail_query();
    assert_eq!(h.state.status.as_ref(), Some(&Err(import_error.clone())));
    h.replace("localization-source-filter", "");
    h.settle();
    assert_eq!(h.state.workbench.page.as_ref().unwrap().total, 85);
    assert!(h.state.workbench.query_error.is_none());
    assert_eq!(h.state.status.as_ref(), Some(&Err(import_error.clone())));
    let rendered = h.rendered();
    assert!(rendered.contains(&import_error) && !rendered.contains(QUERY_ERROR));
    h.assert_unchanged(&inputs);
}

#[test]
fn localization_catalog_pending_and_cancel_retain_last_query_and_operation_errors() {
    let mut h = Harness::new();
    let inputs = h.inputs();
    h.fail_query();
    let import_error = h.fail_import();
    h.replace("localization-source-filter", "");
    catalog::refresh(&h.project, &mut h.state, 1);
    assert!(h.state.jobs.catalog_pending());
    assert_eq!(h.state.workbench.query_error.as_deref(), Some(QUERY_ERROR));
    assert_eq!(h.state.status.as_ref(), Some(&Err(import_error.clone())));
    // Render the real cancellation control without pumping, so even a fast query stays queued.
    let ctx = egui::Context::default();
    let mut frame = |events| {
        ctx.run(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| jobs::status(ui, &mut h.state));
            },
        )
    };
    let output = frame(vec![]);
    let point = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == "取消本次核对" => {
                Some(text.pos + text.galley.size() * 0.5)
            }
            _ => None,
        })
        .expect("rendered cancel action");
    for pressed in [true, false] {
        frame(vec![
            Event::PointerMoved(point),
            Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            },
        ]);
    }
    assert!(!h.state.jobs.pending());
    assert!(h.state.jobs.notice.as_deref().unwrap().contains("已取消"));
    assert_eq!(h.state.workbench.query_error.as_deref(), Some(QUERY_ERROR));
    assert_eq!(h.state.status.as_ref(), Some(&Err(import_error.clone())));
    let rendered = h.rendered();
    assert!(rendered.contains(QUERY_ERROR) && rendered.contains(&import_error));
    h.assert_unchanged(&inputs);
}
