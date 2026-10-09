//! Full App::update consumes the real workbench link requests, including Alt+Left return.
use super::super::{exchange, jobs, plans, workbench_tests};
use super::*;
use egui::{Event, Pos2, Rect};
use worldline_core::localization::{LocalizationExchange, LocalizationPart};

mod guards;
mod harness;
mod previews;
use harness::*;

#[test]
fn localization_navigation_full_app_catalog_and_export_root_include_all_kinds_lf_crlf() {
    for crlf in [false, true] {
        let mut h = Harness::new(crlf);
        for (id, file, line, kind) in CASES {
            h.catalog(id);
            let entry = h.app.localization_ui.workbench.selected_entry().unwrap();
            assert_eq!(entry.id.as_deref(), Some(id));
            let source = entry.source.clone().unwrap();
            assert_eq!(
                (&*source.file, source.line, &*source.kind),
                (file, line, kind)
            );
            let before = h.unchanged();
            let label = format!("源文 {file}:{line}");
            for _ in 0..2 {
                h.click(&label);
                h.assert_source(&source, id);
                h.back();
                assert_eq!(h.unchanged(), before);
                assert_eq!(
                    h.app
                        .localization_ui
                        .workbench
                        .selected_entry()
                        .unwrap()
                        .id
                        .as_deref(),
                    Some(id)
                );
            }
        }
        h.click("高级 JSON 交换");
        h.click("预览导出");
        assert!(
            h.app
                .localization_ui
                .export_plan
                .as_ref()
                .unwrap()
                .can_export
        );
        for (id, file, line, kind) in CASES {
            let source = h
                .app
                .localization_ui
                .export_plan
                .as_ref()
                .unwrap()
                .exchange
                .entries
                .iter()
                .find(|entry| entry.id == id)
                .unwrap()
                .source
                .clone();
            let before = h.unchanged();
            h.click(&format!("定位来源 {file}:{line} · {kind}"));
            h.assert_source(&source, id);
            h.back();
            assert!(h.app.localization_ui.advanced);
            assert_eq!(h.unchanged(), before);
        }
    }
}

#[test]
fn localization_navigation_full_app_invalid_token_diagnostics_resolve_current_physical_units() {
    for crlf in [false, true] {
        let mut h = Harness::new(crlf);
        let mut exchange = h.exchange();
        for entry in &mut exchange.entries {
            assert!(entry
                .source_parts
                .iter()
                .any(|part| matches!(part, LocalizationPart::Placeholder { .. })));
            entry.translation_parts = Some(vec![LocalizationPart::Text {
                text: "丢掉受保护 token 的译文".into(),
            }]);
        }
        h.import(exchange);
        let plan = h.app.localization_ui.import_plan.as_ref().unwrap();
        assert!(!plan.can_apply);
        assert_eq!(
            plan.diagnostics
                .iter()
                .filter(|d| d.code == "INVALID_TOKEN")
                .count(),
            6
        );
        for (id, file, line, kind) in CASES {
            let source = h
                .app
                .localization_ui
                .import_plan
                .as_ref()
                .unwrap()
                .diagnostics
                .iter()
                .find(|d| d.code == "INVALID_TOKEN" && d.id.as_deref() == Some(id))
                .unwrap()
                .source
                .clone()
                .unwrap();
            assert_eq!(
                (&*source.file, source.line, &*source.kind),
                (file, line, kind)
            );
            let before = h.unchanged();
            for _ in 0..2 {
                h.click(&format!("定位诊断 {file}:{line} · {kind}"));
                h.assert_source(&source, id);
                h.back();
                assert_eq!(h.unchanged(), before);
            }
        }
    }
}

#[test]
fn localization_navigation_full_app_old_package_cannot_borrow_another_real_unit_source() {
    let mut h = Harness::new(false);
    h.click("高级 JSON 交换");
    h.click("预览导出");
    let mut exchange = h.exchange();
    let real = exchange
        .entries
        .iter()
        .find(|e| e.id == "root_body")
        .unwrap()
        .source
        .clone();
    let entry = exchange
        .entries
        .iter_mut()
        .find(|e| e.id == "body")
        .unwrap();
    let current = entry.source.clone();
    entry.source = real.clone();
    // Keep just this malformed package entry; the already accepted export still has the real unit.
    exchange.entries.retain(|entry| entry.id == "body");
    exchange.string_ids = vec!["body".into()];
    h.app.localization_ui.string_ids = "body".into();
    h.import(exchange);
    let plan = h.app.localization_ui.import_plan.as_ref().unwrap();
    assert!(!plan.can_apply);
    let diagnostic = plan
        .diagnostics
        .iter()
        .find(|d| d.code == "SOURCE_MISMATCH")
        .unwrap();
    assert_eq!(diagnostic.source.as_ref(), Some(&current));
    let before = h.unchanged();
    // The last matching link is the imported entry, not the earlier trusted export row.
    h.click_last(&format!(
        "定位来源 {}:{} · {}",
        real.file, real.line, real.kind
    ));
    assert_eq!(h.app.tab, Tab::Localization);
    assert!(h
        .app
        .message
        .as_deref()
        .unwrap()
        .contains("尚未通过核心核对"));
    assert_eq!(h.unchanged(), before);
    h.click(&format!(
        "定位诊断 {}:{} · {}",
        current.file, current.line, current.kind
    ));
    h.assert_source(&current, "body");
    h.back();
    assert_eq!(h.unchanged(), before);
}

#[test]
fn localization_navigation_full_app_valid_import_entries_and_stale_source_diagnostic() {
    let mut h = Harness::new(false);
    h.import(h.exchange());
    assert!(
        h.app
            .localization_ui
            .import_plan
            .as_ref()
            .unwrap()
            .can_apply
    );
    for (id, file, line, kind) in CASES {
        let source = h
            .app
            .localization_ui
            .import_exchange
            .as_ref()
            .unwrap()
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .unwrap()
            .source
            .clone();
        let before = h.unchanged();
        h.click(&format!("定位来源 {file}:{line} · {kind}"));
        h.assert_source(&source, id);
        h.back();
        assert_eq!(h.unchanged(), before);
    }
    let mut old = h.exchange();
    old.entries
        .iter_mut()
        .find(|entry| entry.id == "root_spoken")
        .unwrap()
        .source_revision
        .push_str("-old");
    h.import(old);
    let diagnostic = h
        .app
        .localization_ui
        .import_plan
        .as_ref()
        .unwrap()
        .diagnostics
        .iter()
        .find(|d| d.code == "STALE_SOURCE")
        .unwrap()
        .clone();
    let source = diagnostic.source.unwrap();
    let before = h.unchanged();
    h.click(&format!(
        "定位诊断 {}:{} · {}",
        source.file, source.line, source.kind
    ));
    h.assert_source(&source, "root_spoken");
    h.back();
    assert_eq!(h.unchanged(), before);
}

#[test]
fn localization_navigation_full_app_export_diagnostic_uses_current_core_source() {
    let mut h = Harness::new(true);
    let path = h.app.project.root.join("chapters/body.wl");
    let source = h
        .app
        .project
        .document(&path)
        .unwrap()
        .replace("#wl-localization:body", "#wl-localization:root_body");
    h.app.project.set_text(&path, source).unwrap();
    h.app.project.save().unwrap();
    h.app.recompile();
    h.app.localization_ui.string_ids = "root_body".into();
    h.click("高级 JSON 交换");
    h.click("预览导出");
    let plan = h.app.localization_ui.export_plan.as_ref().unwrap();
    assert!(!plan.can_export);
    let diagnostic = plan
        .diagnostics
        .iter()
        .find(|d| d.code == "DUPLICATE_ID")
        .unwrap();
    let source = diagnostic.source.clone().unwrap();
    let before = h.unchanged();
    h.click(&format!(
        "定位诊断 {}:{} · {}",
        source.file, source.line, source.kind
    ));
    h.assert_source(&source, "root_body");
    h.back();
    assert_eq!(h.unchanged(), before);
}
