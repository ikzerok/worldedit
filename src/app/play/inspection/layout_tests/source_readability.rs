//! 回源后的真实源码裁剪；不以行列标题或离屏 galley 存在代替可读正文。
use super::*;

#[test]
fn inspector_source_return_at_800_by_600_two_hundred_percent_keeps_target_readable() {
    let (ctx, mut app) = setup();
    let source = format!(
        "{}let note = 10\nevent start\n  choice \"继续\"\n    -> END\n",
        "// 检查器回源保持原文\n".repeat(7)
    );
    app.project
        .set_text(&app.active_file.clone(), source.clone())
        .unwrap();
    app.recompile();
    app.start_play();
    app.play
        .as_mut()
        .unwrap()
        .story
        .as_mut()
        .unwrap()
        .continue_story()
        .unwrap();
    app.personal.pending_restore = false;
    app.personal.settings.appearance = AppearancePreferences {
        palette: PaletteId::Vellum,
        theme: ThemeMode::Light,
        style: StylePreset::Manuscript,
        density: Density::Spacious,
        ui_scale: 2.0,
        reduce_motion: true,
        ..Default::default()
    };
    app.replay_debugger.inspection.query.text = "note".into();
    app.replay_debugger.inspection.show(&ctx);
    let mut h = Harness {
        ctx,
        app,
        size: Vec2::new(400.0, 300.0),
    };
    h.settle();
    assert_eq!(h.ctx.pixels_per_point(), 2.0);
    assert_eq!(
        h.ctx.screen_rect().size() * h.ctx.pixels_per_point(),
        Vec2::new(800.0, 600.0)
    );
    let before = unchanged(&h.app);
    let undo = h.app.history.len();
    h.click("定位变量声明");
    assert_eq!(h.app.tab, Tab::Edit);
    for focus in [false, true] {
        h.app.personal.settings.focus = focus;
        let output = h.settle();
        let id = egui::Id::new(("source", &h.app.active_file));
        let range = egui::TextEdit::load_state(&h.ctx, id)
            .unwrap()
            .cursor
            .char_range()
            .unwrap();
        let coordinates =
            worldline_core::source_coordinates::SourceCoordinates::new(&source).unwrap();
        let position = coordinates
            .position_at_character(&source, range.primary.index)
            .unwrap();
        assert_eq!((position.line, position.column), (8, 14));
        let (clip, target, declaration, complete_rows) = output
            .shapes
            .iter()
            .find_map(|shape| match &shape.shape {
                egui::Shape::Text(text) if text.galley.text() == source => {
                    let clip = shape.clip_rect.intersect(h.ctx.screen_rect());
                    let target = text
                        .galley
                        .pos_from_cursor(range.primary)
                        .translate(text.pos.to_vec2());
                    let declaration = text.galley.rows[7].rect().translate(text.pos.to_vec2());
                    let complete_rows = text
                        .galley
                        .rows
                        .iter()
                        .filter(|row| {
                            let row = row.rect().translate(text.pos.to_vec2());
                            row.top() >= clip.top() && row.bottom() <= clip.bottom()
                        })
                        .count();
                    Some((clip, target, declaration, complete_rows))
                }
                _ => None,
            })
            .expect("回源必须绘制同一文件的真实源码 galley");
        assert!(
            clip.contains_rect(target),
            "目标第 8 行第 14 列须完整可见：focus={focus}, clip={clip:?}, target={target:?}"
        );
        assert!(
            clip.contains_rect(declaration),
            "第 8 行短声明须整行可读：focus={focus}, clip={clip:?}, declaration={declaration:?}"
        );
        assert!(
            complete_rows >= 1,
            "必须有完整源码行：focus={focus}, clip={clip:?}"
        );
        assert!(clip.height() >= target.height(), "源码视口不能只留碎片行");
        assert_eq!(unchanged(&h.app), before);
        assert_eq!(h.app.history.len(), undo);
    }
    h.app.author_back(&h.ctx);
    h.settle();
    assert_eq!(h.app.tab, Tab::Play);
    assert!(h.app.replay_debugger.inspection.open);
    assert_eq!(h.app.replay_debugger.inspection.query.text, "note");
    assert_eq!(unchanged(&h.app), before);
}
