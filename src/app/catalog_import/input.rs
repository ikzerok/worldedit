use super::*;
use worldline_core::catalog_import::MAX_CSV_BYTES;

impl ImportState {
    pub(super) fn choose_file(&mut self, ctx: &egui::Context) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            use std::io::Read;
            let Some(path) = rfd::FileDialog::new()
                .set_title("选择世界资料 CSV 快照")
                .add_filter("UTF-8 CSV", &["csv"])
                .pick_file()
            else {
                return;
            };
            let result = (|| {
                let mut bytes = Vec::new();
                std::fs::File::open(&path)
                    .map_err(|e| e.to_string())?
                    .take(MAX_CSV_BYTES as u64 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|e| e.to_string())?;
                self.offer_file(path.display().to_string(), bytes, ctx)
            })();
            if let Err(error) = result {
                self.set_failure(error);
            }
        }
        #[cfg(target_arch = "wasm32")]
        crate::web::select_files(ctx, false, ".csv", crate::web::FileAction::CatalogImport);
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn load_browser_files(&mut self, files: crate::web::Files, ctx: &egui::Context) {
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
        self.source_name = name;
        self.csv = csv;
        self.start_parse(ctx);
    }
}
