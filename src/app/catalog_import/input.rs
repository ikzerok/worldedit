use super::*;
use worldline_core::catalog_import::MAX_CSV_BYTES;

impl ImportState {
    pub(super) fn choose_file(&mut self, ctx: &egui::Context) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let Some(path) = rfd::FileDialog::new()
                .set_title("选择世界资料 CSV 快照")
                .add_filter("UTF-8 CSV", &["csv"])
                .pick_file()
            else {
                return;
            };
            self.load_native_file(&path, ctx);
        }
        #[cfg(target_arch = "wasm32")]
        crate::web::select_files(ctx, false, ".csv", crate::web::FileAction::CatalogImport);
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn load_native_path(&mut self, ctx: &egui::Context) {
        let path = PathBuf::from(&self.native_path);
        self.load_native_file(&path, ctx);
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn load_native_file(&mut self, path: &std::path::Path, ctx: &egui::Context) {
        let result = read_native_csv(path)
            .and_then(|bytes| self.offer_file(path.display().to_string(), bytes, ctx));
        if let Err(error) = result {
            self.set_failure(error);
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(in crate::app) fn load_browser_files(
        &mut self,
        files: crate::web::Files,
        ctx: &egui::Context,
    ) {
        if files.len() != 1 {
            self.set_failure("请选择单个 UTF-8 CSV 快照".into());
            return;
        }
        if let Some((name, bytes)) = files.into_iter().next() {
            if let Err(error) = self.offer_file(name.display().to_string(), bytes, ctx) {
                self.set_failure(error);
            }
        }
    }

    pub(super) fn offer_file(
        &mut self,
        name: String,
        bytes: Vec<u8>,
        ctx: &egui::Context,
    ) -> Result<(), String> {
        if bytes.len() > MAX_CSV_BYTES {
            return Err("CSV 超过 2 MiB 上限；现有输入已保留".into());
        }
        let csv = String::from_utf8(bytes)
            .map_err(|_| "CSV 必须为 UTF-8；不会猜测编码，现有输入已保留")?;
        if !self.source_name.is_empty() || self.table.is_some() {
            self.acknowledged = false;
            self.replacement = Some((name, csv));
        } else {
            self.accept_file(name, csv, ctx);
        }
        Ok(())
    }

    pub(super) fn accept_file(&mut self, name: String, csv: String, ctx: &egui::Context) {
        self.discard();
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.native_path = name.clone();
        }
        self.source_name = name;
        self.csv = csv;
        self.start_parse(ctx);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn read_native_csv(path: &std::path::Path) -> Result<Vec<u8>, String> {
    use std::io::Read;
    if !path.is_absolute() {
        return Err("请填写完整的 CSV 文件路径；不会搜索目录或猜测文件位置".into());
    }
    let metadata = std::fs::symlink_metadata(path).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => "指定 CSV 文件不存在；路径与现有快照已保留".into(),
        std::io::ErrorKind::PermissionDenied => {
            "没有读取此 CSV 路径的权限；未尝试其他路径，输入已保留".into()
        }
        _ => format!("无法检查指定 CSV 文件：{error}；输入已保留"),
    })?;
    if !metadata.file_type().is_file() {
        return Err("请选择普通 CSV 文件；不会读取目录、链接或设备，输入已保留".into());
    }
    if metadata.len() > MAX_CSV_BYTES as u64 {
        return Err("CSV 超过 2 MiB 上限；未读取内容，现有快照已保留".into());
    }
    let mut bytes = Vec::new();
    let file = std::fs::File::open(path)
        .map_err(|error| format!("不能读取指定 CSV 文件（请检查读取权限）：{error}；输入已保留"))?;
    if !file
        .metadata()
        .map_err(|error| error.to_string())?
        .is_file()
    {
        return Err("指定路径已不再是普通文件；输入已保留".into());
    }
    file.take(MAX_CSV_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("读取 CSV 失败：{error}；输入已保留"))?;
    if bytes.len() > MAX_CSV_BYTES {
        return Err("CSV 超过 2 MiB 上限；现有快照已保留".into());
    }
    Ok(bytes)
}
