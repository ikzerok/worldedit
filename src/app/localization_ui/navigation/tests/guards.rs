use super::*;

#[test]
fn localization_navigation_rejects_same_line_different_entry_forgery_and_stale_diagnostics() {
    let mut h = Harness::new(false);
    h.catalog("root_body");
    let actual = h.request();
    for changed in [
        "id", "revision", "unit_key", "parts", "line", "kind", "outside",
    ] {
        let mut request = actual.clone();
        let Evidence::Catalog(entry) = &mut request.evidence else {
            unreachable!()
        };
        match changed {
            "id" => entry.id = Some("root_spoken".into()),
            "revision" => entry.source_revision = Some("not-this-entry".into()),
            "unit_key" => entry.unit_key.push_str("-other"),
            "parts" => {
                entry.source_parts = vec![LocalizationPart::Text {
                    text: "different same-line entry".into(),
                }]
            }
            "line" => {
                request.source.line = 5;
                entry.source = Some(request.source.clone());
            }
            "kind" => {
                request.source.kind = "say".into();
                entry.source = Some(request.source.clone());
            }
            "outside" => {
                request.source.file = "../outside.wl".into();
                entry.source = Some(request.source.clone());
            }
            _ => unreachable!(),
        }
        h.reject(request, "精确条目");
    }
    let mut exchange = h.exchange();
    exchange.entries[0].translation_parts = Some(vec![]);
    h.import(exchange);
    let plan = h.app.localization_ui.import_plan.as_ref().unwrap();
    let diagnostic = plan
        .diagnostics
        .iter()
        .find(|d| d.code == "INVALID_TOKEN")
        .unwrap();
    let request = Request::diagnostic(Container::Import(plan), diagnostic).unwrap();
    for field in ["id", "code", "message"] {
        let mut forged = request.clone();
        let Evidence::Diagnostic(diagnostic) = &mut forged.evidence else {
            unreachable!()
        };
        match field {
            "id" => diagnostic.id = Some("not-the-diagnosed-entry".into()),
            "code" => diagnostic.code.push_str("_FORGED"),
            "message" => diagnostic.message.push_str(" forged"),
            _ => unreachable!(),
        }
        h.reject(forged, "精确条目");
    }
    let mut replacement = h.exchange();
    replacement.entries.last_mut().unwrap().translation_parts = Some(vec![]);
    h.import(replacement);
    h.reject(request, "计划已更新");
}

#[test]
fn localization_navigation_read_only_guards_preserve_translation_and_execution_inputs() {
    for reason in [
        "draft",
        "ime",
        "palette_ime",
        "external",
        "baseline",
        "version",
        "compile",
    ] {
        let mut h = Harness::new(false);
        h.catalog("body");
        workbench_tests::stage(&mut h.app.localization_ui, "保留此译文输入");
        let request = h.request();
        let path = h.app.project.root.join("chapters/body.wl");
        let source = h.app.project.document(&path).unwrap().to_owned();
        let message = match reason {
            "draft" => {
                let mut buffer = h.app.project.open_source_writing_buffer(&path).unwrap();
                buffer.replace_source(format!("{source}// 尚未应用正文\n"));
                h.app.manuscript.restore_writing_buffers(&[buffer]);
                "未应用"
            }
            "ime" => {
                h.app.ime_composing = true;
                "输入法"
            }
            "palette_ime" => {
                h.app.command_palette.ime_frame = true;
                "输入法"
            }
            "external" => {
                std::fs::write(&path, format!("{source}// 外部变更\n")).unwrap();
                "来源"
            }
            "baseline" => {
                h.app
                    .project
                    .set_text(&path, format!("\n{source}"))
                    .unwrap();
                "基线已过期"
            }
            "version" => {
                h.app.version += 1;
                "当前工作区"
            }
            "compile" => {
                h.app
                    .snapshot
                    .as_mut()
                    .unwrap()
                    .result
                    .sources
                    .remove(&path);
                "编译来源"
            }
            _ => unreachable!(),
        };
        let buffers = h
            .app
            .manuscript
            .writing_buffers()
            .iter()
            .map(|b| (b.path().to_owned(), b.source().to_owned(), b.generation()))
            .collect::<Vec<_>>();
        for _ in 0..2 {
            h.reject(request.clone(), message);
        }
        assert_eq!(h.app.localization_ui.workbench.drafts.len(), 1);
        assert_eq!(
            h.app
                .manuscript
                .writing_buffers()
                .iter()
                .map(|b| (b.path().to_owned(), b.source().to_owned(), b.generation()))
                .collect::<Vec<_>>(),
            buffers
        );
    }
}

#[test]
fn localization_navigation_full_app_ime_click_is_visible_and_keeps_pending_translation() {
    let mut h = Harness::new(false);
    h.catalog("root_body");
    workbench_tests::stage(&mut h.app.localization_ui, "还没应用的翻译");
    let before = h.unchanged();
    h.app.ime_composing = true;
    h.click("源文 world.wl:4");
    assert_eq!(h.app.tab, Tab::Localization);
    assert!(h.app.message.as_deref().unwrap().contains("输入法"));
    assert_eq!(h.unchanged(), before);
    h.app.ime_composing = false;
    h.click("源文 world.wl:4");
    h.assert_source(
        &LocalizationSource {
            file: "world.wl".into(),
            line: 4,
            kind: "text".into(),
        },
        "root_body",
    );
    h.back();
    assert_eq!(h.unchanged(), before);
}

#[test]
fn localization_navigation_acceptance_stamp_does_not_relabel_retained_page_on_workspace_switch() {
    let mut h = Harness::new(false);
    h.catalog("root_body");
    let request = h.request();
    let mut other = Harness::new(false);
    assert_ne!(h.app.project.root, other.app.project.root);
    assert_eq!(
        h.app.project.content_baseline(),
        other.app.project.content_baseline()
    );
    std::mem::swap(&mut h.app.localization_ui, &mut other.app.localization_ui);
    let retained = other.app.localization_ui.workbench.page.clone();
    other.reject(request.clone(), "当前工作区");
    jobs::pump(
        &other.ctx,
        &other.app.project,
        &mut other.app.localization_ui,
        other.app.version,
    );
    assert_eq!(other.app.localization_ui.workbench.page, retained);
    for kind in [
        AcceptedKind::Catalog,
        AcceptedKind::Import,
        AcceptedKind::Export,
        AcceptedKind::Preview,
    ] {
        assert!(!other.app.localization_ui.jobs.accepted_matches(
            kind,
            &other.app.project.root,
            other.app.version
        ));
    }
    other.reject(request, "当前工作区");
}

#[test]
fn localization_navigation_unselected_extra_package_entry_is_rejected_without_writes() {
    let mut h = Harness::new(false);
    let mut exchange = h.exchange();
    let extra = exchange
        .entries
        .iter_mut()
        .find(|entry| entry.id == "body")
        .unwrap();
    extra.source.file = "../outside.wl".into();
    h.app.localization_ui.string_ids = "root_body".into();
    h.import(exchange);
    let state = &h.app.localization_ui;
    let plan = state.import_plan.as_ref().unwrap();
    assert!(!plan.can_apply);
    assert!(plan
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "UNKNOWN_ID"));
    assert_eq!(plan.affected_ids, ["root_body"]);
    let entry = state
        .import_exchange
        .as_ref()
        .unwrap()
        .entries
        .iter()
        .find(|entry| entry.id == "body")
        .unwrap();
    let request = Request::import_entry(plan, entry);
    h.reject(request, "尚未通过核心核对");
}
