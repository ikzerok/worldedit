//! 200% 下的单行入口、真实正文裁剪与现有动作；不改变旧几何/生命周期测试。
use super::*;
use crate::theme::{Density, PaletteId, StylePreset, ThemeMode};

fn compact_harness() -> Harness {
    let mut h = Harness::new(source());
    h.app.personal.settings.style = StylePreset::Manuscript;
    h.app.personal.settings.palette = PaletteId::Vellum;
    h.app.personal.settings.theme = ThemeMode::Light;
    h.app.personal.settings.density = Density::Spacious;
    h.app.personal.settings.ui_scale = 2.0;
    h.app.personal.settings.reduce_motion = true;
    h.size = egui::vec2(400.0, 300.0);
    h.settle();
    h
}

fn source_clip(output: &egui::FullOutput, source: &str) -> Rect {
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == source => Some(shape.clip_rect),
            _ => None,
        })
        .expect("唯一真实源码 galley 必须存在")
}

fn full_text(output: &egui::FullOutput, label: &str) -> Rect {
    output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                let rect = text.galley.rect.translate(text.pos.to_vec2());
                shape.clip_rect.contains_rect(rect).then_some(rect)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("{label} 必须完整留在实际clip内：{}", all_text(output)))
}

#[test]
fn compact_source_heading_keeps_body_clip_across_caret_context_and_ime_frames() {
    let mut h = compact_harness();
    let output = h.settle();
    let clip = source_clip(&output, source());
    assert!(
        clip.height() >= h.app.personal.settings.source_size * h.app.personal.settings.line_spacing
    );
    let tools = full_text(&output, "源码工具");
    assert_eq!(h.ctx.pixels_per_point(), 2.0);
    let at = source()[..source().find("中文🌊正文").unwrap()]
        .chars()
        .count();
    for index in [4, at, source().chars().count(), at + 1] {
        h.select(index, index);
        for _ in 0..2 {
            let output = h.frame(vec![]);
            assert_eq!(
                source_clip(&output, source()),
                clip,
                "caret帧不能改变正文视口"
            );
            assert_eq!(full_text(&output, "源码工具"), tools);
        }
    }
    h.app.ime_composing = true;
    let output = h.frame(vec![]);
    assert_eq!(source_clip(&output, source()), clip);
    assert_eq!(full_text(&output, "源码工具"), tools);
    assert_eq!(h.source(), source());
}

#[test]
fn compact_source_tools_keep_file_context_structure_position_and_comment_reachable() {
    let mut h = compact_harness();
    h.select(8, 2);
    let range = h.range();
    let baseline = h.app.project.content_baseline();
    let undo = h.app.history.len();
    let before = source_clip(&h.settle(), source());
    h.click("源码工具");
    let output = h.settle();
    for label in [
        "本文件结构  Ctrl+Shift+O",
        "返回作者位置",
        "为当前选区添加批注",
        "world.wl",
    ] {
        full_text(&output, label);
    }
    full_text(&output, "编辑位置 · 文件正文 / 空白（无所属声明）");
    h.click("本文件结构  Ctrl+Shift+O");
    h.settle();
    assert!(h.app.source_outline.open);
    h.press(Key::Escape, Modifiers::NONE);
    let output = h.settle();
    assert!(!h.app.source_outline.open);
    assert_eq!(h.range(), range);
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(h.source_id()));
    assert_eq!(source_clip(&output, source()), before);
    // 行列入口保持同一个功能，仍由真实按钮打开，取消不改作者选区。
    let position = worldline_core::source_coordinates::SourceCoordinates::new(h.source())
        .unwrap()
        .position_at_character(h.source(), range.primary.index)
        .unwrap();
    let caption = format!("{}:{}", position.line, position.column);
    full_text(&output, &caption);
    h.click(&caption);
    h.settle();
    assert!(h.app.source_jump.open);
    h.press(Key::Escape, Modifiers::NONE);
    h.settle();
    assert_eq!(h.range(), range);
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(h.source_id()));
    h.click("源码工具");
    h.click("为当前选区添加批注");
    h.settle();
    assert!(h.app.review.comment_editor.is_some());
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert_eq!(h.app.history.len(), undo);
    assert_eq!(h.source(), source());
}

#[test]
fn compact_source_tools_are_keyboard_reachable_without_changing_source() {
    let mut h = compact_harness();
    let baseline = h.app.project.content_baseline();
    let mut reached = false;
    for _ in 0..48 {
        h.press(Key::Tab, Modifiers::NONE);
        let output = h.settle();
        let menu = full_text(&output, "源码工具");
        if h.ctx
            .memory(|memory| memory.focused())
            .and_then(|id| h.ctx.read_response(id))
            .is_some_and(|response| response.rect.contains(menu.center()))
        {
            reached = true;
            break;
        }
    }
    assert!(reached, "Tab 必须能到达源码工具");
    h.press(Key::Enter, Modifiers::NONE);
    full_text(&h.settle(), "本文件结构  Ctrl+Shift+O");
    h.press(Key::Escape, Modifiers::NONE);
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert_eq!(h.source(), source());
}

#[test]
fn compact_source_tools_return_to_other_file_preserves_identity_selection_and_scroll() {
    let mut h = compact_harness();
    let original = h.app.active_file.clone();
    let other = h
        .app
        .project
        .add_file(std::path::Path::new("other.wl"))
        .unwrap();
    h.app
        .project
        .set_text(&other, "event other\n  另一份稿\n  -> END\n".into())
        .unwrap();
    h.app.recompile();
    let original_source = h.app.project.document(&original).unwrap().to_owned();
    h.select(100, 80);
    let selection = h.range();
    let scroll = h.app.personal.source_scroll;
    let origin = h.app.author_location(Some(&h.ctx));
    h.app.remember_author_location(origin);
    let baseline = h.app.project.content_baseline();
    h.app.active_file = other;
    h.settle();
    h.click("源码工具");
    h.click("返回作者位置");
    h.settle();
    assert_eq!(h.app.active_file, original);
    assert_eq!(h.range(), selection);
    for (expected, actual) in scroll.into_iter().zip(h.app.personal.source_scroll) {
        assert!(
            (expected - actual).abs() < 1.5,
            "跨文件返回滚动：{expected} / {actual}"
        );
    }
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert_eq!(h.source(), original_source);
}
