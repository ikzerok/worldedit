//! 自动 egui 与控制器回归，不冒充原生键盘、读屏或物理 IME 测试。
use super::*;
use crate::app::Tab;
use std::sync::atomic::{AtomicUsize, Ordering};
const SOURCE: &str = "let score = 1\nrule ready() -> bool = score > 0\nrule nested() -> bool = ready() and ready()\nevent start\n  choice \"继续\" if nested()\n    set score = score + 1\n    -> END\n";
fn fixture() -> (egui::Context, WorldeditApp) {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let ctx = egui::Context::default();
    let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
    let root = std::env::temp_dir().join(format!(
        "object-context-ui-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    app.project = worldline_core::project::Project::new(&root);
    app.active_file = app.project.entry.clone();
    app.project
        .documents
        .retain(|path, _| path == &app.active_file);
    app.project
        .set_text(&app.active_file, SOURCE.into())
        .unwrap();
    app.project
        .create_authoring_document(
            &app.project.root.join(".world/project.json"),
            br#"{"schema_version":1,"language_version":"1.13","required_features":[]}"#.to_vec(),
        )
        .unwrap();
    app.project.save().unwrap();
    app.reset_views();
    app.recompile();
    app.tab = Tab::Catalog;
    app.personal.pending_restore = false;
    assert!(
        !app.snapshot.as_ref().unwrap().result.has_errors(),
        "{:?}",
        app.diagnostics()
    );
    (ctx, app)
}
fn text(shape: &egui::epaint::Shape, out: &mut String) {
    match shape {
        egui::epaint::Shape::Text(value) => {
            out.push_str(value.galley.text());
            out.push('\n');
        }
        egui::epaint::Shape::Vec(shapes) => {
            for shape in shapes {
                text(shape, out);
            }
        }
        _ => {}
    }
}
#[test]
fn general_rule_context_is_cached_read_only_and_visible_at_supported_widths() {
    let (ctx, mut app) = fixture();
    let target = TargetRef::new("rule", "ready");
    let baseline = app.project.content_baseline();
    let first = app.current_object_context(&target).unwrap();
    let second = app.current_object_context(&target).unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    assert_eq!(first.returned, 3);
    assert!(first.complete);
    for width in [1040.0, 1188.0] {
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 660.0),
                )),
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default()
                    .show(ctx, |ui| app.reading_content(ui, target.clone()));
            },
        );
        let mut painted = String::new();
        for shape in &output.shapes {
            text(&shape.shape, &mut painted);
        }
        assert!(painted.contains("使用处与相关上下文"), "{painted}");
        assert!(painted.contains("静态调用和读写"), "{painted}");
        assert!(painted.contains("规则调用"), "{painted}");
    }
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}
#[test]
fn source_jump_and_author_back_restore_the_object_without_changing_content() {
    let (ctx, mut app) = fixture();
    let target = TargetRef::new("rule", "ready");
    app.open_reading(target.clone());
    let baseline = app.project.content_baseline();
    let result = app.current_object_context(&target).unwrap();
    let row = result
        .records
        .iter()
        .find(|row| row.kind == Kind::RuleCall)
        .unwrap();
    assert!(app.jump_object_context_source(&result, row));
    assert_eq!(app.tab, Tab::Edit);
    assert_eq!(
        app.jump,
        Some((row.source.line, row.source.column.unwrap()))
    );
    assert!(app.reading_target.is_none());
    app.author_back(&ctx);
    assert_eq!(app.tab, Tab::Catalog);
    assert_eq!(app.reading_target, Some(target));
    assert_eq!(app.project.content_baseline(), baseline);
    assert!(app.history.is_empty());
}
#[test]
fn stale_invalid_or_ime_sources_are_refused_and_cache_invalidates() {
    let (_, mut app) = fixture();
    let target = TargetRef::new("rule", "ready");
    let first = app.current_object_context(&target).unwrap();
    let row = first
        .records
        .iter()
        .find(|row| row.kind == Kind::RuleCall)
        .unwrap();
    app.ime_composing = true;
    assert!(!app.jump_object_context_source(&first, row));
    app.ime_composing = false;
    app.project
        .set_text(&app.active_file.clone(), format!("// moved\n{SOURCE}"))
        .unwrap();
    assert!(!app.jump_object_context_source(&first, row));
    app.recompile();
    let second = app.current_object_context(&target).unwrap();
    assert!(!Arc::ptr_eq(&first, &second));
    assert!(!app.jump_object_context_source(&first, row));
    app.project
        .set_text(
            &app.active_file.clone(),
            SOURCE.replace("score > 0", "missing > 0"),
        )
        .unwrap();
    app.recompile();
    let invalid = app.current_object_context(&target).unwrap();
    assert!(!invalid.complete);
    assert!(invalid.reasons.contains(&Limit::InvalidSource));
    assert!(!app.jump_object_context_source(&invalid, &invalid.records[0]));
    assert!(app.personal.history.is_empty());
}

#[test]
fn unrefreshed_external_source_change_refuses_navigation_without_overwriting_either_version() {
    let (_, mut app) = fixture();
    let target = TargetRef::new("rule", "ready");
    app.open_reading(target.clone());
    let result = app.current_object_context(&target).unwrap();
    let row = result
        .records
        .iter()
        .find(|row| row.kind == Kind::RuleCall)
        .unwrap();
    let baseline = app.project.content_baseline();
    let external = format!("// external inserted a line\n{SOURCE}");
    std::fs::write(&app.active_file, &external).unwrap();
    assert!(!app.jump_object_context_source(&result, row));
    assert_eq!(app.tab, Tab::Catalog);
    assert_eq!(app.reading_target, Some(target));
    assert!(app.jump.is_none());
    assert!(app.personal.history.is_empty());
    assert_eq!(app.project.content_baseline(), baseline);
    assert_eq!(app.project.document(&app.active_file).unwrap(), SOURCE);
    assert_eq!(std::fs::read_to_string(&app.active_file).unwrap(), external);
}
