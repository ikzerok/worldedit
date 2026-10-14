//! Full-App transient object window: real keys, same-pass owners and exact retained work.
use super::*;
use std::{collections::HashSet, path::PathBuf};
mod flows;
const CLOSE: &str = "关闭资料并返回";
const READER: &str = "资料阅读区 · ↑↓滚动";
const ORIGINAL: &str = "海雾中的港口渐渐亮起来。 prose native06 typed";
const SUFFIX: &str = " role return";
const ALIAS: &str = "例如：昵称、旧称或简称";
fn layer() -> egui::LayerId {
    egui::LayerId::new(egui::Order::Foreground, egui::Id::new("object-reading"))
}
fn fields(h: &Harness) -> BTreeMap<String, String> {
    h.app
        .manuscript
        .writing_view
        .retained_runtime_drafts(&h.app.project.root)
}
fn disk(h: &Harness) -> BTreeMap<PathBuf, (Vec<u8>, u32)> {
    worldline_core::file_access::workspace_files(&h.app.project.root)
        .unwrap()
        .into_iter()
        .map(|p| {
            #[cfg(unix)]
            let mode = {
                use std::os::unix::fs::PermissionsExt;
                std::fs::metadata(&p).unwrap().permissions().mode() & 0o777
            };
            #[cfg(not(unix))]
            let mode = 0;
            let bytes = std::fs::read(&p).unwrap();
            (p, (bytes, mode))
        })
        .collect()
}
fn preserved(h: &Harness) -> Value {
    // Every fixture starts with all four complete history vectors empty. Equality
    // therefore proves the full stacks remain empty, not just one chosen count.
    assert!(h.app.history.is_empty() && h.app.redo.is_empty());
    assert!(h.app.search_state.undo.is_empty() && h.app.search_state.redo.is_empty());
    let mut buffers: Vec<_> = h
        .app
        .manuscript
        .writing_buffers()
        .into_iter()
        .map(|b| {
            (
                b.path().to_owned(),
                b.identity().to_owned(),
                b.source().to_owned(),
                b.is_changed(),
            )
        })
        .collect();
    buffers.sort();
    json!({"project_and_history":h.state(),"buffers_full":buffers,"f_complete":fields(h),
        "node":format!("{:?}",h.app.history_state.current),"disk_bytes_mode":disk(h)})
}
fn dirty() -> (Harness, egui::Id) {
    let mut h = Harness::normal(&format!("  say traveler \"{ORIGINAL}\"\n"));
    h.toolbar("逐句对白");
    h.click("编辑此句");
    assert!(
        fields(&h).is_empty(),
        "clean Update is not a protected draft"
    );
    let out = h.settle();
    let r = h
        .focused_response
        .clone()
        .expect("Edit opens its real literal");
    assert!(r.has_focus() && egui::TextEdit::load_state(&h.ctx, r.id).is_some());
    assert!(r.rect.contains_rect(visible_label(&out, ORIGINAL).unwrap()));
    h.key(Key::End, Modifiers::NONE);
    for c in SUFFIX.chars() {
        h.frame(vec![Event::Text(c.to_string())]);
    }
    h.settle();
    assert_eq!(h.literal(), format!("{ORIGINAL}{SUFFIX}"));
    let f = fields(&h);
    assert_eq!(f.len(), 1);
    let request: Value = serde_json::from_str(f.values().next().unwrap()).unwrap();
    assert_eq!(request["operation"]["draft"]["speaker"]["id"], "traveler");
    assert!(request["operation"]["draft"]["direction"].is_null());
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(r.id));
    (h, r.id)
}
fn tap(h: &mut Harness, key: Key, modifiers: Modifiers) -> egui::FullOutput {
    let mut info = vec![];
    for pressed in [true, false] {
        let out = h.frame(vec![Event::Key {
            key,
            physical_key: Some(key),
            pressed,
            repeat: false,
            modifiers,
        }]);
        info.extend(out.platform_output.events);
    }
    let mut out = h.frame(vec![]);
    info.append(&mut out.platform_output.events);
    out.platform_output.events = info;
    out
}
#[track_caller]
fn focused(h: &Harness) -> egui::Response {
    let r = h
        .focused_response
        .clone()
        .expect("actual final-pass focused Response");
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(r.id));
    assert!(r.has_focus());
    r
}
fn finish(h: &mut Harness, mut out: egui::FullOutput) -> egui::FullOutput {
    let r = focused(h);
    if !r.interact_rect.contains_rect(r.rect) {
        assert_eq!(
            out.viewport_output[&egui::ViewportId::ROOT].repaint_delay,
            std::time::Duration::ZERO
        );
        let before = preserved(h);
        out = h.frame(vec![]);
        assert_eq!(
            focused(h).id,
            r.id,
            "one requested redraw preserves real owner"
        );
        assert_eq!(preserved(h), before);
    }
    out
}
fn named(h: &Harness, out: &egui::FullOutput, label: &str, text: bool) -> bool {
    let Some(r) = h.focused_response.as_ref() else {
        return false;
    };
    if h.ctx.memory(|m| m.focused()) != Some(r.id) {
        return false;
    }
    texts(out).iter().any(|(s, rect, _)| {
        s == label
            && if text {
                egui::TextEdit::load_state(&h.ctx, r.id).is_some()
                    && r.rect.expand(1.0).contains_rect(*rect)
            } else {
                control_owns_label(&h.ctx, r, *rect)
            }
    })
}
fn visible_owner(h: &Harness, out: &egui::FullOutput, label: &str, text: bool) -> egui::Id {
    let r = focused(h);
    assert!(r.interact_rect.contains_rect(r.rect), "{label}: {r:?}");
    assert!(
        texts(out).iter().any(|(s, rect, clip)| s == label
            && clip.contains_rect(*rect)
            && if text {
                r.rect.expand(1.0).contains_rect(*rect)
            } else {
                clip.contains_rect(r.rect) && control_owns_label(&h.ctx, &r, *rect)
            }),
        "{label}: owner={r:?}, paint={:?}",
        texts(out)
    );
    assert!(
        if text {
            text_frame(h, out, &r)
        } else {
            has_control_focus_paint(&h.ctx, out, r.rect)
        },
        "distinguishable local focus for {label}: {r:?}"
    );
    r.id
}
pub(super) fn text_frame(h: &Harness, out: &egui::FullOutput, r: &egui::Response) -> bool {
    let stroke = h.ctx.style().visuals.selection.stroke;
    fn visit(s: &egui::Shape, clip: Rect, target: Rect, stroke: egui::Stroke) -> bool {
        match s {
            egui::Shape::Rect(p) => {
                p.stroke == stroke
                    && clip.contains_rect(p.rect)
                    && (p.rect.min - target.min).length() <= 0.5
                    && (p.rect.max - target.max).length() <= 0.5
            }
            egui::Shape::Vec(v) => v.iter().any(|s| visit(s, clip, target, stroke)),
            _ => false,
        }
    }
    out.shapes.iter().any(|s| {
        visit(
            &s.shape,
            s.clip_rect,
            r.rect
                .expand(h.ctx.style().visuals.widgets.active.expansion),
            stroke,
        )
    })
}
fn seek(h: &mut Harness, label: &str, text: bool, reverse: bool) -> egui::Id {
    for _ in 0..180 {
        let out = tap(
            h,
            Key::Tab,
            if reverse {
                Modifiers::SHIFT
            } else {
                Modifiers::NONE
            },
        );
        if named(h, &out, label, text) {
            let out = finish(h, out);
            return visible_owner(h, &out, label, text);
        }
    }
    panic!("Tab did not reach {label}; final={:?}", h.focused_response);
}
fn open_details(h: &mut Harness) -> egui::Id {
    let before = preserved(h);
    let opener = seek(h, "人物资料", false, true);
    tap(h, Key::Enter, Modifiers::NONE);
    assert_eq!(
        h.app.reading_target,
        Some(TargetRef::new("character", "traveler"))
    );
    assert!(h.app.reading_return.is_none());
    assert_eq!(preserved(h), before);
    opener
}
fn assert_first(h: &mut Harness) -> egui::Id {
    let out = h.frame(vec![]);
    let r = focused(h);
    assert_eq!(r.layer_id, layer());
    visible_owner(h, &out, CLOSE, false)
}
#[track_caller]
fn returned(h: &mut Harness, opener: egui::Id, original: &Value) {
    assert!(h.app.reading_target.is_none());
    let out = h.frame(vec![]);
    let r = focused(h);
    assert_eq!(r.id, opener);
    assert_ne!(r.layer_id, layer());
    assert!(r.interact_rect.contains_rect(r.rect));
    assert!(has_control_focus_paint(&h.ctx, &out, r.rect));
    assert_eq!(preserved(h), *original);
}
fn click_point(h: &mut Harness, point: egui::Pos2) {
    for pressed in [true, false] {
        h.frame(vec![
            Event::PointerMoved(point),
            Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            },
        ]);
    }
    h.settle();
}

fn local_reading_content(h: &Harness, out: &egui::FullOutput) -> (Rect, Rect) {
    let r = focused(h);
    assert_eq!(r.layer_id, layer());
    let painted = texts(out);
    let domain = painted
        .iter()
        .find(|(text, rect, clip)| {
            text == READER && control_owns_label(&h.ctx, &r, *rect) && clip.contains_rect(*rect)
        })
        .map(|(_, _, clip)| *clip)
        .expect("same-pass named reader's real local content clip");
    let window = egui::AreaState::load(&h.ctx, layer().id).unwrap().rect();
    assert!(
        window.contains_rect(domain),
        "the reading domain belongs to the real object window"
    );
    let rect = painted
        .iter()
        .find(|(text, rect, clip)| {
            text == "按当前工程内容汇总；表单修改应用后会更新此页。"
                && *clip == domain
                && clip.contains_rect(*rect)
        })
        .map(|(_, rect, _)| *rect)
        .expect("exact visible object-content row within that domain");
    (rect, domain)
}
