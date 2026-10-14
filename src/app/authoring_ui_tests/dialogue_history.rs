//! v0.34 有限跨事务链。合成 egui + 真实专用磁盘，不代表原生窗口/物理 IME。
use super::*;
use std::{collections::BTreeMap, fs, path::PathBuf};
use worldline_core::manuscript::WritingBuffer;

mod fixtures;
mod guards;
#[cfg(not(target_arch = "wasm32"))]
mod persistence;
mod sidecars;
mod world_links;
use fixtures::*;

#[test]
fn dialogue_history_source_and_typed_share_one_buffer_across_chapters() {
    // H01/H09/N01/N02：源码先改，再 typed；返回源码继续改，最后一次应用。
    let (ctx, mut app, _cleanup) = fixture();
    let path = app.project.entry.clone();
    let original = app.project.sources();
    let disk_before = disk(&app);
    click(&ctx, &mut app, 13, "源码");
    let source = buffer(&app, &path)
        .source()
        .replace("原来的正式对白", "源码先改的对白");
    replace_source(&ctx, &mut app, &source);
    click(&ctx, &mut app, 13, "写作");
    open_body(&ctx, &mut app);
    // 返回写作会聚焦首句的干净表单；先真实关闭，避免通用按钮点到其他语句。
    assert!(!app.manuscript.has_dialogue_input());
    let source_buffer = buffer(&app, &path);
    let before_cancel = project_files(&app.project);
    let history_before_cancel = counts(&app);
    click(&ctx, &mut app, 13, "取消此句输入");
    assert!(!app.manuscript.has_dialogue_input());
    assert_eq!(buffer(&app, &path).identity(), source_buffer.identity());
    assert_eq!(project_files(&app.project), before_cancel);
    assert_eq!(disk(&app), disk_before);
    assert_eq!(counts(&app), history_before_cancel);
    stage(&ctx, &mut app, "源码先改的对白", "typed 再改😀");
    let staged = buffer(&app, &path);
    assert_eq!(app.project.sources(), original);
    assert_eq!(disk(&app), disk_before);
    click(&ctx, &mut app, 13, "离港");
    click(&ctx, &mut app, 13, "抵达");
    assert_eq!(buffer(&app, &path).identity(), staged.identity());
    assert_eq!(
        app.manuscript
            .writing_buffers()
            .iter()
            .filter(|b| b.path() == path)
            .count(),
        1
    );
    click(&ctx, &mut app, 13, "源码");
    let source = format!("{}// 源码后加的保留注释\n", staged.source());
    replace_source(&ctx, &mut app, &source);
    let applied_buffer = buffer(&app, &path);
    click(&ctx, &mut app, 13, "应用源码草稿（可含诊断）");
    assert_eq!(app.project.document(&path).unwrap(), source);
    assert_eq!(counts(&app), (1, 0, 1, 0));
    app.edit_undo(false);
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    assert_eq!(app.project.sources(), original);
    assert_buffer(&app, &applied_buffer, true);
    app.edit_undo(true);
    assert_eq!(app.project.document(&path).unwrap(), source);
    assert_eq!(disk(&app), disk_before);
    save_reopen(&mut app);
}

#[test]
fn dialogue_history_manuscript_reorder_handoff_preserves_typed_draft_and_order() {
    // H10：真实编排 Apply 的 sidecar 边与 typed 草稿边/Project正文边共存。
    let (ctx, mut app, _cleanup) = fixture();
    let original = project_files(&app.project);
    let disk_before = disk(&app);
    let fingerprint = app.snapshot.as_ref().unwrap().result.analysis.fingerprint;
    stage(&ctx, &mut app, "原来的正式对白", "编排期间的台词😀");
    let staged = buffer(&app, &app.project.entry);
    click(&ctx, &mut app, 13, "编排与来源");
    click(&ctx, &mut app, 13, "下移");
    assert_eq!(project_files(&app.project), original);
    click(&ctx, &mut app, 13, "应用书稿");
    assert_eq!(
        app.project
            .manuscript_index("novel")
            .unwrap()
            .page(0, 10)
            .chapters[0]
            .id,
        "departure"
    );
    assert_eq!(
        app.snapshot.as_ref().unwrap().result.analysis.fingerprint,
        fingerprint
    );
    assert_eq!(counts(&app), (1, 0, 1, 0));
    assert_buffer(&app, &staged, true);
    assert_eq!(
        app.project.document(&app.project.entry).unwrap().as_bytes(),
        original[&app.project.entry]
    );
    click(&ctx, &mut app, 13, "编排与来源");
    apply_body(&ctx, &mut app);
    let applied = project_files(&app.project);
    for _ in 0..2 {
        app.edit_undo(false);
        assert!(app.io_error.is_none(), "{:?}", app.io_error);
        assert_buffer(&app, &staged, true);
    }
    assert_eq!(counts(&app), (0, 2, 1, 0));
    assert_eq!(project_files(&app.project), original);
    assert_eq!(
        app.project
            .manuscript_index("novel")
            .unwrap()
            .page(0, 10)
            .chapters[0]
            .id,
        "opening"
    );
    for _ in 0..2 {
        app.edit_undo(true);
        assert!(app.io_error.is_none(), "{:?}", app.io_error);
    }
    assert_eq!(project_files(&app.project), applied);
    assert_eq!(disk(&app), disk_before);
    save_reopen(&mut app);
}
