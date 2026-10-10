//! T -> one B -> W, using full-App controls and exact current-pass reading geometry.
use super::*;
use std::collections::BTreeSet;
use worldline_core::manuscript::{WritingAuthoringPlan, WritingBlockKind};
mod flows;
mod mesh;
const READER: &str = "关联预览阅读区 · ↑↓滚动";
const ORIGINAL: &str = "旅人缓缓走向灯塔。";
const TYPED: &str = "旅人缓缓走向灯塔。 T staged";

fn state(h: &Harness) -> Value {
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
    let disk: BTreeMap<_, _> = worldline_core::file_access::workspace_files(&h.app.project.root)
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
        .collect();
    let draft_nodes: Vec<_> = h
        .app
        .search_state
        .undo
        .iter()
        .map(|e| {
            (
                format!("{:?}", e.node),
                e.guard.content_baseline(),
                std::sync::Arc::as_ptr(&e.guard) as usize,
            )
        })
        .collect();
    json!({"project":h.state(),"buffers":buffers,"fields":h.fields(),"disk":disk,
        "current_node":format!("{:?}",h.app.history_state.current),"draft_nodes_and_guards":draft_nodes})
}
fn current_plan(h: &Harness) -> WritingAuthoringPlan {
    let s = h.app.manuscript.world_links.as_ref().expect("real W form");
    let plan = s
        .plan
        .clone()
        .unwrap_or_else(|| panic!("real Preview failed: {:?}", s.error));
    assert!(plan.can_apply && !plan.creates_object() && plan.migration.is_none());
    let derived = h
        .app
        .project
        .preview_writing_authoring(&h.app.manuscript.writing_buffers(), &s.request().unwrap())
        .unwrap();
    assert_eq!(plan.plan_digest, derived.plan_digest);
    assert_eq!(
        serde_json::to_value(plan.request()).unwrap(),
        serde_json::to_value(s.request().unwrap()).unwrap()
    );
    plan
}
fn setup(long: bool) -> (Harness, WritingAuthoringPlan, String) {
    let mut statement = format!("  {ORIGINAL}\n");
    for n in 0..if long { 300 } else { 12 } {
        statement.push_str(&format!(
            "  // W_ROW_{n:03} 完整同稿字节，保留中文与😀以及原始注释末尾。\n"
        ));
    }
    let mut h = Harness::normal(&statement);
    let applied = h
        .app
        .project
        .document(&h.app.active_file)
        .unwrap()
        .to_owned();
    // Normal typed Update really stages into B; W subsequently observes that exact B.
    h.toolbar("逐句对白");
    h.click("编辑此句");
    h.click(ORIGINAL);
    h.key(Key::A, Modifiers::COMMAND);
    h.frame(vec![Event::Text(TYPED.into())]);
    h.click("预览语句变更");
    let t = h.current_plan();
    let mut expected = h.app.manuscript.writing_buffers().pop().unwrap();
    h.app
        .project
        .stage_dialogue_edit(&mut expected, &t)
        .unwrap();
    h.tab_to("纳入正文草稿");
    h.key(Key::Enter, Modifiers::NONE);
    assert_eq!(
        h.app.manuscript.writing_buffers()[0].source(),
        expected.source()
    );
    assert_eq!(h.app.project.document(&h.app.active_file).unwrap(), applied);
    assert!(!h.app.manuscript.has_dialogue_input());
    assert_eq!(
        (
            h.app.history.len(),
            h.app.redo.len(),
            h.app.search_state.undo.len(),
            h.app.search_state.redo.len()
        ),
        (0, 0, 1, 0)
    );
    h.toolbar("逐句对白");
    select_name(&mut h);
    open_existing(&mut h);
    h.click("预览关联计划");
    let plan = current_plan(&h);
    assert_eq!(plan.source_path, expected.path());
    assert_eq!(plan.request().selection.expected_text, "旅人");
    assert!(plan
        .included_buffers
        .iter()
        .any(|b| b.path == expected.path() && b.changed));
    assert_eq!(plan.changes.len(), 1);
    assert_eq!(plan.changes[0].before.as_deref(), Some(applied.as_str()));
    assert!(plan.changes[0].after.contains("T staged"));
    assert!(plan.changes[0]
        .after
        .contains("[[character:traveler|旅人]]"));
    eprintln!(
        "W_READING_STAGE setup long={long} B={} before={} after={}",
        expected.source().len(),
        applied.len(),
        plan.changes[0].after.len()
    );
    (h, plan, applied)
}
fn select_name(h: &mut Harness) {
    let buffer = h.app.manuscript.writing_buffers().pop().unwrap();
    let projection = h
        .app
        .project
        .project_writing_buffer(&buffer, &TargetRef::new("event", "arrival"))
        .unwrap();
    let text = projection
        .blocks
        .iter()
        .find(|b| b.kind == WritingBlockKind::Prose && b.text.contains("旅人"))
        .unwrap()
        .text
        .clone();
    h.click(&text);
    h.key(Key::Home, Modifiers::COMMAND);
    let prefix = text[..text.find("旅人").unwrap()].chars().count();
    for _ in 0..prefix {
        h.key(Key::ArrowRight, Modifiers::NONE);
    }
    for _ in 0..2 {
        h.key(Key::ArrowRight, Modifiers::SHIFT);
    }
    let selected = crate::app::search::editor_selection(&h.ctx).expect("real source selection");
    assert_eq!(&selected.source[selected.range.clone()], "旅人");
    assert_eq!(selected.source, buffer.source());
    assert_eq!(selected.path, buffer.path());
}
fn open_existing(h: &mut Harness) {
    let out = h.settle();
    if visible_label(&out, "选词工具").is_none() {
        h.click("正文工具");
    }
    h.click("选词工具");
    h.click("关联世界资料…");
    let first = h.settle();
    let response = owner(h);
    assert_eq!(response.layer_id, super::super::world_links_layout::layer());
    assert!(
        texts(&first)
            .iter()
            .any(|(s, rect, clip)| s == "返回正文，保留输入"
                && control_owns_label(&h.ctx, &response, *rect)
                && clip.contains_rect(*rect)
                && clip.contains_rect(response.rect)),
        "the actual W entry starts at its visible safe return control: {response:?}"
    );
    assert!(has_control_focus_paint(&h.ctx, &first, response.rect));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let label = loop {
        h.frame(vec![]);
        let s = h
            .app
            .manuscript
            .world_links
            .as_ref()
            .expect("actual W toolbar entry");
        assert!(s.open);
        if let Some(Ok(page)) = &s.page.result {
            if let Some(o) = page
                .items
                .iter()
                .find(|o| o.target == TargetRef::new("character", "traveler"))
            {
                break crate::app::object_picker::candidate_caption(o, Some(&h.app.project.root));
            }
        }
        assert!(
            std::time::Instant::now() < deadline,
            "real catalog query result"
        );
        std::thread::yield_now();
    };
    h.click(&label);
    assert_eq!(
        h.app.manuscript.world_links.as_ref().unwrap().chosen,
        Some(TargetRef::new("character", "traveler"))
    );
}
fn owner(h: &Harness) -> egui::Response {
    let r = h
        .focused_response
        .clone()
        .expect("actual same-pass W owner");
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(r.id));
    assert!(r.has_focus());
    r
}
fn assert_local_cycle(h: &mut Harness) {
    let before = state(h);
    for reverse in [false, true] {
        let mut seen = BTreeSet::new();
        let mut repeated = false;
        for _ in 0..180 {
            h.key(
                Key::Tab,
                if reverse {
                    Modifiers::SHIFT
                } else {
                    Modifiers::NONE
                },
            );
            let response = owner(h);
            assert_eq!(
                response.layer_id,
                super::super::world_links_layout::layer(),
                "W's active local cycle cannot enter the long editable background"
            );
            assert_eq!(state(h), before);
            if !seen.insert(response.id.value()) {
                repeated = true;
                break;
            }
        }
        assert!(
            repeated,
            "actual W focus cycle must repeat; reverse={reverse}, owners={seen:?}"
        );
    }
}

fn seek(h: &mut Harness, label: &str) -> egui::FullOutput {
    for _ in 0..180 {
        let mut out = h.key(Key::Tab, Modifiers::NONE);
        let Some(r) = h.focused_response.clone() else {
            continue;
        };
        if r.layer_id.id != egui::Id::new("manuscript-world-links") {
            continue;
        }
        if !texts(&out)
            .iter()
            .any(|(s, rect, _)| s == label && control_owns_label(&h.ctx, &r, *rect))
        {
            continue;
        }
        if !r.interact_rect.contains_rect(r.rect) {
            assert_eq!(
                out.viewport_output[&egui::ViewportId::ROOT].repaint_delay,
                std::time::Duration::ZERO
            );
            let before = state(h);
            out = h.frame(vec![]);
            assert_eq!(owner(h).id, r.id);
            assert_eq!(state(h), before);
        }
        let r = owner(h);
        assert!(r.interact_rect.contains_rect(r.rect), "{label}: {r:?}");
        assert!(
            texts(&out).iter().any(|(s, rect, clip)| s == label
                && control_owns_label(&h.ctx, &r, *rect)
                && clip.contains_rect(*rect)
                && clip.contains_rect(r.rect)),
            "{label}: owner={r:?} paint={:?}",
            texts(&out)
        );
        assert!(
            has_control_focus_paint(&h.ctx, &out, r.rect),
            "W focus visible for {label}: {r:?}"
        );
        return out;
    }
    panic!(
        "W Tab did not reach {label}; owner={:?}; state={}",
        h.focused_response,
        h.app
            .manuscript
            .world_links
            .as_ref()
            .map(|s| format!("open={} error={:?}", s.open, s.error))
            .unwrap_or_default()
    );
}
fn enter(h: &mut Harness, label: &str) {
    seek(h, label);
    h.key(Key::Enter, Modifiers::NONE);
}
fn expand(h: &mut Harness) {
    enter(h, "world.wl");
    enter(h, "应用前（已应用工程）");
    enter(h, "关联后（完整候选）");
}
fn read_pages(h: &mut Harness, before: &str, after: &str) {
    for (side, whole) in [("before", before), ("after", after)] {
        let other = if side == "before" {
            "关联后（完整候选）"
        } else {
            "应用前（已应用工程）"
        };
        enter(h, other); // Close the other expanded byte block through its real header.
        let pages = whole.len().div_ceil(16 * 1024);
        let mut reconstructed = String::new();
        for page in 0..pages {
            if page > 0 {
                enter(h, "后页");
            }
            let mut start = (page * 16 * 1024).min(whole.len());
            let mut end = ((page + 1) * 16 * 1024).min(whole.len());
            while start > 0 && !whole.is_char_boundary(start) {
                start -= 1;
            }
            while end > start && !whole.is_char_boundary(end) {
                end -= 1;
            }
            let exact = &whole[start..end];
            read_page(h, exact, &format!("{side}:{page}"));
            reconstructed.push_str(exact);
            eprintln!(
                "W_READING_STAGE read side={side} page={page}/{pages} bytes={}",
                exact.len()
            );
        }
        assert_eq!(reconstructed.as_bytes(), whole.as_bytes());
        for _ in 1..pages {
            enter(h, "前页");
        }
        // Re-open the other block so the next side starts from the same two-header state.
        enter(h, other);
    }
}
fn read_page(h: &mut Harness, exact: &str, stage: &str) {
    let mut out = seek(h, READER);
    let id = owner(h).id;
    let domain = texts(&out)
        .into_iter()
        .find(|(s, rect, clip)| {
            s == READER && control_owns_label(&h.ctx, &owner(h), *rect) && clip.contains_rect(*rect)
        })
        .unwrap()
        .2;
    assert!(
        has_viewport_focus_paint(&h.ctx, &out, domain),
        "real local W viewport frame"
    );
    let chars: Vec<_> = exact.chars().collect();
    let expected: BTreeSet<_> = chars
        .iter()
        .enumerate()
        .filter_map(|(i, c)| (!c.is_whitespace()).then_some(i))
        .collect();
    let before = state(h);
    let mut seen = BTreeSet::new();
    let mut last = Vec::new();
    for _ in 0..1600 {
        last = mesh::glyphs(&h.ctx, &out, exact, domain);
        for (index, c, visible, _, _) in &last {
            assert_eq!(chars.get(*index), Some(c));
            if *visible && !c.is_whitespace() {
                seen.insert(*index);
            }
        }
        if seen == expected {
            break;
        }
        out = h.key(Key::ArrowDown, Modifiers::NONE);
        assert_eq!(owner(h).id, id);
        assert!(has_viewport_focus_paint(&h.ctx, &out, domain));
    }
    assert_eq!(
        seen, expected,
        "{stage}: every actual glyph quad must fully enter local clip; last={last:?}; owner={:?}",
        h.focused_response
    );
    assert_eq!(state(h), before);
    // Real Up returns the first non-whitespace glyph to the viewport; no scroll state injection.
    let first = *expected.first().unwrap();
    for _ in 0..1600 {
        if mesh::glyphs(&h.ctx, &out, exact, domain)
            .iter()
            .any(|(i, _, visible, _, _)| *i == first && *visible)
        {
            break;
        }
        out = h.key(Key::ArrowUp, Modifiers::NONE);
        assert_eq!(owner(h).id, id);
    }
    assert!(mesh::glyphs(&h.ctx, &out, exact, domain)
        .iter()
        .any(|(i, _, v, _, _)| *i == first && *v));
    assert_eq!(state(h), before);
}

fn seek_query(h: &mut Harness) -> egui::Id {
    let id = egui::Id::new("world-link-query");
    for _ in 0..180 {
        let mut out = h.key(Key::Tab, Modifiers::NONE);
        if h.ctx.memory(|m| m.focused()) != Some(id) {
            continue;
        }
        let r = owner(h);
        assert_eq!(r.id, id);
        if !r.interact_rect.contains_rect(r.rect) {
            assert_eq!(
                out.viewport_output[&egui::ViewportId::ROOT].repaint_delay,
                std::time::Duration::ZERO
            );
            out = h.frame(vec![]);
            assert_eq!(owner(h).id, id);
        }
        let r = owner(h);
        assert!(r.interact_rect.contains_rect(r.rect));
        let query = &h.app.manuscript.world_links.as_ref().unwrap().query;
        assert!(texts(&out).iter().any(|(s, rect, clip)| s == query
            && r.rect.expand(1.0).contains_rect(*rect)
            && clip.contains_rect(*rect)));
        assert!(super::reading_window::text_frame(h, &out, &r));
        return id;
    }
    panic!("real query Tab owner not reached: {:?}", h.focused_response);
}
