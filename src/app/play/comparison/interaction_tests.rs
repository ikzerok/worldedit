//! 模式切换的真实 egui 指针事件必须在推进普通 Story 前停下。
use super::*;

#[test]
fn pointer_press_and_release_into_comparison_never_advance_pending_live_actions() {
    let (ctx, mut app) = setup();
    app.comparison.active = false;
    let size = egui::vec2(1188.0, 848.0);
    frame(&ctx, &mut app, size);
    let output = frame(&ctx, &mut app, size);
    let mut found = Vec::new();
    for shape in &output.shapes {
        labels(&shape.shape, &mut found);
    }
    let position = found
        .iter()
        .find(|(text, _)| text == "路线对照")
        .unwrap()
        .1
        .center();
    app.play
        .as_mut()
        .unwrap()
        .story
        .as_mut()
        .unwrap()
        .choose(0)
        .unwrap();
    let before = invariant(&app);
    for pressed in [true, false] {
        frame_events(
            &ctx,
            &mut app,
            size,
            vec![
                egui::Event::PointerMoved(position),
                egui::Event::PointerButton {
                    pos: position,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
        assert_eq!(
            invariant(&app),
            before,
            "mode gesture advanced live story on pressed={pressed}"
        );
    }
    assert!(app.comparison.active);
    frame(&ctx, &mut app, size);
    assert_eq!(invariant(&app), before);
    assert!(!app.play.as_ref().unwrap().paused);
    assert!(app.comparison.job.is_none());
    assert_eq!(app.comparison.generation, 0);
}
