use super::*;

#[test]
fn localization_navigation_full_app_edit_diagnostic_returns_to_same_preview_and_cancel_keeps_draft()
{
    let mut h = Harness::new(false);
    h.catalog("body");
    workbench_tests::stage(&mut h.app.localization_ui, "故意缺少 token 的译文");
    h.click("预览 1 项译文");
    let Some(plans::Preview::Edits { plan, .. }) = &h.app.localization_ui.workbench.preview else {
        panic!("accepted edit preview")
    };
    assert!(!plan.can_apply);
    let diagnostic = plan
        .diagnostics
        .iter()
        .find(|d| d.code == "INVALID_TOKEN")
        .unwrap();
    let request = Request::diagnostic(Container::Edits(plan), diagnostic).unwrap();
    let source = request.source.clone();
    let before = h.unchanged();
    h.click("定位诊断 chapters/body.wl:11 · text");
    h.assert_source(&source, "body");
    h.back();
    assert!(matches!(
        h.app.localization_ui.workbench.preview,
        Some(plans::Preview::Edits { .. })
    ));
    assert_eq!(h.unchanged(), before);
    h.click("取消预览，保留输入");
    assert!(h.app.localization_ui.workbench.preview.is_none());
    assert_eq!(h.unchanged(), before);
    h.reject(request, "预览已关闭");
}

#[test]
fn localization_navigation_full_app_id_diagnostic_checks_assignment_identity_and_cancellation() {
    let mut h = Harness::new(true);
    h.catalog("root_spoken");
    let entry = h
        .app
        .localization_ui
        .workbench
        .selected_entry()
        .unwrap()
        .clone();
    h.app
        .localization_ui
        .workbench
        .id_inputs
        .insert(entry.unit_key.clone(), "body".into());
    h.click("稳定身份与来源");
    h.click("预览此 ID 修改");
    let Some(plans::Preview::Id { plan, .. }) = &h.app.localization_ui.workbench.preview else {
        panic!("accepted ID preview")
    };
    assert!(!plan.can_apply);
    let diagnostic = plan
        .diagnostics
        .iter()
        .find(|d| d.code == "DUPLICATE_ID")
        .unwrap();
    let request = Request::diagnostic(Container::Id(plan), diagnostic).unwrap();
    let source = request.source.clone();
    let before = h.unchanged();
    h.click("定位诊断 world.wl:5 · say");
    h.assert_source(&source, "root_spoken");
    h.back();
    assert_eq!(h.unchanged(), before);
    assert_eq!(
        h.app
            .localization_ui
            .workbench
            .id_inputs
            .get(&entry.unit_key)
            .unwrap(),
        "body"
    );
    h.click("取消预览，保留输入");
    h.reject(request, "预览已关闭");
    assert_eq!(
        h.app
            .localization_ui
            .workbench
            .id_inputs
            .get(&entry.unit_key)
            .unwrap(),
        "body"
    );
}

#[test]
fn localization_navigation_stale_id_assignment_diagnostic_cannot_resolve_another_current_unit() {
    let mut h = Harness::new(false);
    h.catalog("root_body");
    let entry = h
        .app
        .localization_ui
        .workbench
        .selected_entry()
        .unwrap()
        .clone();
    h.app
        .localization_ui
        .workbench
        .id_inputs
        .insert(entry.unit_key.clone(), "new_name".into());
    plans::preview_id(&h.app.project, &mut h.app.localization_ui, &entry);
    h.settle();
    let Some(plans::Preview::Id { mut draft, key, .. }) =
        h.app.localization_ui.workbench.preview.take()
    else {
        panic!("ID preview")
    };
    // Feed the existing worker a stale identity at a different real unit; core echoes assignment.source.
    draft.assignments[0].source.line = 5;
    draft.assignments[0].source.kind = "say".into();
    h.app.localization_ui.jobs.submit(
        crate::localization_job::Task::IdPreview { draft },
        jobs::Intent::Id { key },
    );
    h.settle();
    let Some(plans::Preview::Id { plan, .. }) = &h.app.localization_ui.workbench.preview else {
        panic!("stale ID preview")
    };
    let diagnostic = plan
        .diagnostics
        .iter()
        .find(|d| d.code == "STALE_SOURCE")
        .unwrap();
    let request = Request::diagnostic(Container::Id(plan), diagnostic).unwrap();
    assert_eq!(request.source.line, 5);
    h.reject(request, "精确身份");
}

#[test]
fn localization_navigation_cancel_pending_result_retains_only_previously_accepted_page() {
    let mut h = Harness::new(false);
    h.catalog("root_body");
    let request = h.request();
    let before = h.unchanged();
    exchange::preview_export(&h.app.project, &mut h.app.localization_ui);
    h.app.localization_ui.jobs.cancel();
    h.settle();
    assert!(h.app.localization_ui.export_plan.is_none());
    assert!(h.app.localization_ui.jobs.accepted_matches(
        AcceptedKind::Catalog,
        &h.app.project.root,
        h.app.version
    ));
    let source = request.source.clone();
    // A cancellation must not erase the provenance of the unchanged, still displayed catalog.
    h.click("源文 world.wl:4");
    h.assert_source(&source, "root_body");
    h.back();
    assert_eq!(h.unchanged(), before);
}
