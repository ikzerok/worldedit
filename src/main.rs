//! worldedit —— worldline 作者工作台(egui)。

mod app;
mod archive;
mod chrome;
#[cfg(feature = "eds11_prototype")]
mod eds11_prototype;
mod fonts;
mod highlight;
mod media;
#[cfg(any(target_arch = "wasm32", test))]
mod save_flow;
mod theme;
mod visual;
#[cfg(target_arch = "wasm32")]
mod web;

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    #[cfg(feature = "eds11_prototype")]
    if eds11_prototype::requested_native(std::env::args().skip(1)) {
        return eframe::run_native(
            "worldedit EDS-11 prototype",
            eds11_prototype::native_options(),
            Box::new(|cc| Ok(Box::new(eds11_prototype::Prototype::new(cc)))),
        );
    }
    let initial: Option<PathBuf> = std::env::args().nth(1).map(PathBuf::from).or_else(|| {
        rfd::FileDialog::new()
            .set_title("选择工作区目录，空目录将创建示例工程")
            .pick_folder()
    });
    let Some(path) = initial.as_ref() else {
        return Ok(());
    };
    if path.is_dir() && std::fs::read_dir(path).is_ok_and(|mut entries| entries.next().is_none()) {
        if let Err(error) = worldline_core::project::Project::new(path).save() {
            eprintln!("{error}");
            return Ok(());
        }
    }
    if let Err(error) = worldline_core::project::Project::open(path) {
        eprintln!("{error}");
        return Ok(());
    }
    let options = native_options();
    eframe::run_native(
        "worldedit",
        options,
        Box::new(|cc| Ok(Box::new(app::WorldeditApp::new(cc, initial)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    web::start();
}

#[cfg(not(target_arch = "wasm32"))]
fn native_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/worldedit-icon.png"))
                    .expect("内置 worldedit 图标应为有效 PNG"),
            )
            .with_inner_size([1280.0, 760.0])
            .with_min_inner_size([1040.0, 660.0])
            .with_decorations(false)
            .with_transparent(true)
            .with_title("worldedit · worldline 作者工作台"),
        persist_window: false,
        ..Default::default()
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
#[test]
fn native_favorite_storage_uses_the_user_profile_without_persisting_window_state() {
    let options = native_options();
    assert!(!options.persist_window);
    assert!(options.persistence_path.is_none());
    // 此 API 只有启用 eframe persistence 才存在，防止再次漏接原生存储。
    let storage = eframe::storage_dir("worldedit").expect("原生用户应有独立数据目录");
    assert!(storage.is_absolute());
}
