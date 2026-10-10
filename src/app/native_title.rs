//! 原生窗口身份只取已打开工程的位置，不读取正文、查询 core 或持久化状态。
use super::WorldeditApp;
use std::path::{Path, PathBuf};

const MAX_NAME_CHARS: usize = 48;

#[derive(Clone, Default)]
struct TitleState {
    root: Option<PathBuf>,
    title: Option<String>,
}

impl TitleState {
    fn refresh(&mut self, root: Option<&Path>) -> Option<String> {
        if self.title.is_some() && self.root.as_deref() == root {
            return None;
        }
        let title = window_title(root);
        self.root = root.map(Path::to_path_buf);
        if self.title.as_deref() == Some(title.as_str()) {
            return None;
        }
        self.title = Some(title.clone());
        Some(title)
    }
}

impl WorldeditApp {
    pub(super) fn sync_native_window_title(&self, ctx: &egui::Context) {
        // 未选择工程的 App 也有临时 root；沿用真实的打开/另存状态。
        let root = self.saved_location.then_some(self.project.root.as_path());
        let title = ctx.data_mut(|data| {
            data.get_temp_mut_or_default::<TitleState>(egui::Id::new(
                "worldedit.native-window-title",
            ))
            .refresh(root)
        });
        if let Some(title) = title {
            ctx.send_viewport_cmd_to(egui::ViewportId::ROOT, egui::ViewportCommand::Title(title));
        }
    }
}

fn window_title(root: Option<&Path>) -> String {
    let project = root.map_or_else(
        || "未打开工程".to_owned(),
        |root| format!("{} [{:012x}]", project_name(root), project_marker(root)),
    );
    format!("worldedit v{} · {project}", env!("CARGO_PKG_VERSION"))
}

fn project_name(root: &Path) -> String {
    // 与工作台现有目录短名称一致；不能把绝对路径或父目录放进系统标题。
    let name = root.file_name().unwrap_or_default().to_string_lossy();
    let mut chars = name.trim().chars().map(|ch| {
        if ch.is_control() || matches!(ch, '\u{2028}'..='\u{202e}' | '\u{2066}'..='\u{2069}') {
            ' '
        } else {
            ch
        }
    });
    let mut display: String = chars.by_ref().take(MAX_NAME_CHARS).collect();
    if chars.next().is_some() {
        display.push('…');
    }
    if display.trim().is_empty() {
        "未命名工程".to_owned()
    } else {
        display
    }
}

fn project_marker(root: &Path) -> u64 {
    // 固定 FNV-1a 与 48 位显示；不用进程随机种子或 Rust 默认哈希契约。
    // 同平台原始路径编码重复打开稳定，坏 UTF-8 不先有损替换。
    // 只是可辨的短身份，不是加密、匿名化或权限判断依据。
    root.as_os_str()
        .as_encoded_bytes()
        .iter()
        .fold(0xcbf29ce484222325u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        })
        & 0xffff_ffff_ffff
}

#[cfg(test)]
mod tests {
    use super::*;
    use worldline_core::project::Project;

    fn app() -> (egui::Context, WorldeditApp) {
        let ctx = egui::Context::default();
        let creation = eframe::CreationContext::_new_kittest(ctx.clone());
        let app = WorldeditApp::new(&creation, None);
        (ctx, app)
    }

    fn titles(output: &egui::FullOutput) -> Vec<String> {
        output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .iter()
            .filter_map(|command| match command {
                egui::ViewportCommand::Title(title) => Some(title.clone()),
                _ => None,
            })
            .collect()
    }

    fn title_frame(ctx: &egui::Context, app: &WorldeditApp) -> egui::FullOutput {
        ctx.run(egui::RawInput::default(), |ctx| {
            app.sync_native_window_title(ctx);
            app.sync_native_window_title(ctx);
        })
    }

    struct Workspace(PathBuf);

    impl Workspace {
        fn new() -> Self {
            use std::sync::atomic::{AtomicUsize, Ordering};
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "worldedit-native-title-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            std::fs::create_dir_all(&root).unwrap();
            Self(root)
        }

        fn project(&self, relative: &str) -> PathBuf {
            let root = self.0.join(relative);
            std::fs::create_dir_all(&root).unwrap();
            Project::new(&root).save().unwrap();
            root
        }
    }

    impl Drop for Workspace {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn exact_version_and_unopened_state_do_not_depend_on_a_nonempty_draft_root() {
        assert_eq!(window_title(None), "worldedit v0.34.0 · 未打开工程");
        let (ctx, app) = app();
        assert!(!app.saved_location);
        assert!(!app.project.root.as_os_str().is_empty());
        assert_eq!(titles(&title_frame(&ctx, &app)), [window_title(None)]);
        assert!(titles(&title_frame(&ctx, &app)).is_empty());
        assert!(!window_title(Some(Path::new("/"))).contains("未打开工程"));
    }

    #[test]
    fn same_named_roots_have_stable_distinct_markers_without_parent_paths() {
        let a = Path::new("/private/first/同名作品");
        let b = Path::new("/private/second/同名作品");
        let title = window_title(Some(a));
        assert_eq!(title, window_title(Some(a)));
        assert_ne!(title, window_title(Some(b)));
        assert!(title.starts_with("worldedit v0.34.0 · 同名作品 ["));
        assert!(!title.contains("private"));
        assert!(!title.contains("first"));
        assert!(!title.contains('/'));
        assert_eq!(project_marker(Path::new("hello")), 0xd84680aabd0b);
    }

    #[test]
    fn unicode_names_are_bounded_and_title_controls_are_not_passed_through() {
        assert_eq!(project_name(Path::new("/作品/海港🌊夜话")), "海港🌊夜话");
        let long = "雪".repeat(MAX_NAME_CHARS + 8);
        let short = project_name(Path::new(&long));
        assert_eq!(short, format!("{}…", "雪".repeat(MAX_NAME_CHARS)));
        let unsafe_name = project_name(Path::new("/作品/海港\n\t\u{202e}夜话"));
        assert_eq!(unsafe_name, "海港   夜话");
        assert!(!unsafe_name.chars().any(char::is_control));
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_path_bytes_keep_distinct_markers_even_when_names_display_identically() {
        use std::os::unix::ffi::OsStringExt;
        let a = PathBuf::from(std::ffi::OsString::from_vec(
            b"/private/story-\xfe".to_vec(),
        ));
        let b = PathBuf::from(std::ffi::OsString::from_vec(
            b"/private/story-\xff".to_vec(),
        ));
        assert_eq!(project_name(&a), project_name(&b));
        assert_ne!(window_title(Some(&a)), window_title(Some(&b)));
        assert_eq!(window_title(Some(&a)), window_title(Some(&a)));
        // 这里只验证传入标题的路径；不扩展 core 现有的工程路径支持。
    }

    #[test]
    fn real_open_reopen_switch_failed_open_and_return_emit_only_changed_titles() {
        let workspace = Workspace::new();
        let a = workspace.project("first/同名作品");
        let b = workspace.project("second/同名作品");
        let (ctx, mut app) = app();
        app.load_project(a.clone());
        assert!(app.saved_location);
        let first = window_title(Some(&app.project.root));
        assert_eq!(
            titles(&title_frame(&ctx, &app)),
            std::slice::from_ref(&first)
        );
        assert!(titles(&title_frame(&ctx, &app)).is_empty());
        app.load_project(a.join("world.wl"));
        assert!(titles(&title_frame(&ctx, &app)).is_empty());
        app.load_project(b);
        let second = window_title(Some(&app.project.root));
        assert_ne!(first, second);
        assert_eq!(titles(&title_frame(&ctx, &app)), [second]);
        app.load_project(workspace.0.join("missing/world.wl"));
        assert!(app.io_error.is_some());
        assert!(titles(&title_frame(&ctx, &app)).is_empty());
        app.load_project(a);
        assert_eq!(
            titles(&title_frame(&ctx, &app)),
            std::slice::from_ref(&first)
        );
        let other_context = egui::Context::default();
        assert_eq!(titles(&title_frame(&other_context, &app)), [first]);
        app.saved_location = false;
        assert_eq!(titles(&title_frame(&ctx, &app)), [window_title(None)]);
    }

    #[test]
    fn content_drafts_active_file_and_appearance_do_not_refresh_identity() {
        let workspace = Workspace::new();
        let root = workspace.project("故事");
        let (ctx, mut app) = app();
        app.load_project(root.clone());
        assert_eq!(titles(&title_frame(&ctx, &app)).len(), 1);
        let entry = app.project.entry.clone();
        let disk_before = std::fs::read(&entry).unwrap();
        app.project
            .set_text(&entry, "event start\n  未保存正文。\n  -> END\n".into())
            .unwrap();
        app.recompile();
        app.new_event(None);
        app.event_editor.as_mut().unwrap().draft.body = "未提交草稿。\n-> END".into();
        app.active_file = root.join("未提交来源.wl");
        app.personal.settings.appearance.theme = crate::theme::ThemeMode::Dark;
        let _theme = crate::theme::configure_appearance(&ctx, app.personal.appearance());
        let sources = app.project.sources();
        let baseline = app.project.content_baseline();
        let dirty = app.project.is_dirty();
        let draft = app.event_editor.as_ref().unwrap().draft.body.clone();
        let personal = serde_json::to_value(&app.personal).unwrap();
        let theme = crate::theme::resolved(&ctx);
        let style = ctx.style();
        let version = app.version;
        let output = title_frame(&ctx, &app);
        assert!(titles(&output).is_empty());
        assert!(output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .is_empty());
        // 冷缓存真正发出 Title 时，同样不能改数据、草稿或主题。
        ctx.data_mut(|data| {
            data.remove::<TitleState>(egui::Id::new("worldedit.native-window-title"));
        });
        let output = title_frame(&ctx, &app);
        assert_eq!(titles(&output), [window_title(Some(&app.project.root))]);
        assert!(output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .iter()
            .all(|command| matches!(command, egui::ViewportCommand::Title(_))));
        assert_eq!(app.project.sources(), sources);
        assert_eq!(app.project.content_baseline(), baseline);
        assert_eq!(app.project.is_dirty(), dirty);
        assert_eq!(app.event_editor.as_ref().unwrap().draft.body, draft);
        assert_eq!(serde_json::to_value(&app.personal).unwrap(), personal);
        assert_eq!(crate::theme::resolved(&ctx), theme);
        assert_eq!(ctx.style(), style);
        assert_eq!(app.version, version);
        assert_eq!(std::fs::read(entry).unwrap(), disk_before);
    }

    #[test]
    fn same_frame_switch_and_full_app_update_synchronize_the_root_viewport() {
        use eframe::App;
        let workspace = Workspace::new();
        let root = workspace.project("海港🌊夜话");
        let (ctx, mut app) = app();
        let output = ctx.run(egui::RawInput::default(), |ctx| {
            app.sync_native_window_title(ctx);
            app.load_project(root.clone());
            app.sync_native_window_title(ctx);
        });
        assert_eq!(
            titles(&output),
            [window_title(None), window_title(Some(&app.project.root))]
        );
        let (new_context, mut reopened) = self::app();
        reopened.load_project(root);
        let output = new_context.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1188.0, 848.0),
                )),
                ..Default::default()
            },
            |ctx| reopened.update(ctx, &mut eframe::Frame::_new_kittest()),
        );
        assert_eq!(titles(&output), [window_title(Some(&app.project.root))]);
        assert_eq!(output.viewport_output.len(), 1);
    }
}
