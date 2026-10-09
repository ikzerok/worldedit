//! Isolated, cancellable localization preparation shared by native and Web Worker hosts.
use serde::{Deserialize, Serialize};
use worldline_core::{localization::*, project::Project};

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Task {
    Catalog {
        query: LocalizationCatalogQuery,
    },
    EditPreview {
        draft: LocalizationEditDraft,
    },
    IdPreview {
        draft: LocalizationIdDraft,
    },
    ImportPreview {
        selection: LocalizationSelection,
        json: String,
    },
    ExportPreview {
        selection: LocalizationSelection,
    },
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Output {
    Catalog {
        page: LocalizationCatalogPage,
    },
    EditPreview {
        plan: LocalizationImportPlan,
    },
    IdPreview {
        plan: LocalizationIdPlan,
    },
    ImportPreview {
        exchange: LocalizationExchange,
        plan: LocalizationImportPlan,
    },
    ExportPreview {
        plan: LocalizationExportPlan,
    },
}
impl Task {
    pub(crate) fn run(&self, project: &Project) -> Result<Output, String> {
        match self {
            Self::Catalog { query } => project
                .query_localization_catalog(query)
                .map(|page| Output::Catalog { page })
                .map_err(|e| e.to_string()),
            Self::EditPreview { draft } => project
                .preview_localization_edit(draft)
                .map(|plan| Output::EditPreview { plan })
                .map_err(|e| e.to_string()),
            Self::IdPreview { draft } => project
                .preview_localization_ids(draft)
                .map(|plan| Output::IdPreview { plan })
                .map_err(|e| e.to_string()),
            Self::ImportPreview { selection, json } => {
                if json.len() > MAX_LOCALIZATION_JSON_BYTES {
                    return Err("本地化交换文件超过 8 MiB 预算，未解析".into());
                }
                let exchange = LocalizationExchange::from_json_bytes(json.as_bytes())?;
                let plan = project
                    .preview_localization_import_candidate(selection, &exchange)
                    .map_err(|e| e.to_string())?;
                Ok(Output::ImportPreview { exchange, plan })
            }
            Self::ExportPreview { selection } => project
                .preview_localization_export(selection)
                .map(|plan| Output::ExportPreview { plan }),
        }
    }
    pub(crate) fn accepts(&self, output: &Output, baseline: &str) -> Result<(), String> {
        if !self.matches_output(output)
            || !crate::json_budget::serialized_within(output, 32 * 1024 * 1024)
        {
            return Err("本地化后台类型或结果预算不匹配".into());
        }
        let valid = match (self, output) {
            (Self::Catalog { query }, Output::Catalog { page }) => {
                page.schema_version == 1
                    && page.content_baseline == baseline
                    && page.entries.len() <= query.limit
                    && page.entries.len() <= MAX_LOCALIZATION_PAGE_SIZE
                    && page.total <= page.all_total
                    && page.all_total <= MAX_LOCALIZATION_UNITS * 2
                    && page.target_locale == query.target_locale
                    && query
                        .expected_content_baseline
                        .as_ref()
                        .is_none_or(|b| b == &page.content_baseline)
                    && query
                        .expected_source_baseline
                        .as_ref()
                        .is_none_or(|b| b == &page.source_baseline)
            }
            (Self::EditPreview { draft }, Output::EditPreview { plan }) => {
                plan.content_baseline == baseline
                    && plan.affected_ids.len() <= MAX_LOCALIZATION_BATCH
                    && (!plan.can_apply
                        || (plan.target_locale == draft.target_locale
                            && plan.source_baseline == draft.source_baseline))
            }
            (Self::IdPreview { draft }, Output::IdPreview { plan }) => {
                plan.content_baseline == baseline
                    && plan.changes.len() <= MAX_LOCALIZATION_BATCH
                    && (!plan.can_apply || plan.source_baseline == draft.source_baseline)
            }
            (Self::ImportPreview { selection, .. }, Output::ImportPreview { plan, exchange }) => {
                plan.content_baseline == baseline
                    && exchange.entries.len() <= MAX_LOCALIZATION_BATCH
                    && plan.affected_ids.len() <= MAX_LOCALIZATION_BATCH
                    && (!plan.can_apply
                        || (exchange.target_locale == selection.target_locale
                            && exchange.source_locale == selection.source_locale
                            && plan.target_locale == selection.target_locale))
            }
            (Self::ExportPreview { selection }, Output::ExportPreview { plan }) => {
                plan.content_baseline == baseline
                    && plan.exchange.entries.len() <= MAX_LOCALIZATION_BATCH
                    && (!plan.can_export
                        || (plan.exchange.target_locale == selection.target_locale
                            && plan.exchange.source_locale == selection.source_locale))
            }
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err("本地化后台来源、范围或请求身份不匹配".into())
        }
    }
    pub(crate) fn matches_output(&self, output: &Output) -> bool {
        matches!(
            (self, output),
            (Self::Catalog { .. }, Output::Catalog { .. })
                | (Self::EditPreview { .. }, Output::EditPreview { .. })
                | (Self::IdPreview { .. }, Output::IdPreview { .. })
                | (Self::ImportPreview { .. }, Output::ImportPreview { .. })
                | (Self::ExportPreview { .. }, Output::ExportPreview { .. })
        )
    }
}

pub(crate) const BUSY: &str = "上一份本地化核对仍在退出；已保留最新请求，等待释放后继续";
#[cfg(not(target_arch = "wasm32"))]
static ACTIVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
#[cfg(not(target_arch = "wasm32"))]
struct Permit;
#[cfg(not(target_arch = "wasm32"))]
impl Drop for Permit {
    fn drop(&mut self) {
        ACTIVE.store(false, std::sync::atomic::Ordering::Release);
    }
}

pub(crate) struct Job {
    pub generation: u64,
    pub version: u64,
    pub root: std::path::PathBuf,
    pub task: Task,
    #[cfg(not(target_arch = "wasm32"))]
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    #[cfg(not(target_arch = "wasm32"))]
    receiver: std::sync::mpsc::Receiver<Result<Output, String>>,
    #[cfg(target_arch = "wasm32")]
    worker: crate::worker_host::WorkerJob,
}
impl Job {
    pub(crate) fn available() -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            !ACTIVE.load(std::sync::atomic::Ordering::Acquire)
        }
        #[cfg(target_arch = "wasm32")]
        {
            true
        }
    }
    pub(crate) fn start(
        project: &Project,
        task: Task,
        generation: u64,
        version: u64,
        ctx: &egui::Context,
    ) -> Result<Self, String> {
        project
            .check_localization_budget()
            .map_err(|error| error.to_string())?;
        #[cfg(not(target_arch = "wasm32"))]
        {
            use std::sync::{
                atomic::{AtomicBool, Ordering},
                mpsc, Arc,
            };
            ACTIVE
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .map_err(|_| BUSY.to_owned())?;
            let permit = Permit;
            let root = project.root.clone();
            let snapshot = project.clone();
            let cancel = Arc::new(AtomicBool::new(false));
            let control = cancel.clone();
            let submitted = task.clone();
            let context = ctx.clone();
            let (sender, receiver) = mpsc::channel();
            std::thread::Builder::new()
                .name("localization-prepare".into())
                .spawn(move || {
                    let _permit = permit;
                    if !control.load(Ordering::Acquire) {
                        let result = submitted.run(&snapshot).and_then(|output| {
                            submitted.accepts(&output, &snapshot.content_baseline())?;
                            Ok(output)
                        });
                        if !control.load(Ordering::Acquire) {
                            let _ = sender.send(result);
                        }
                    }
                    drop(snapshot);
                    context.request_repaint();
                })
                .map_err(|e| format!("无法启动本地化核对：{e}"))?;
            Ok(Self {
                generation,
                version,
                root,
                task,
                cancel,
                receiver,
            })
        }
        #[cfg(target_arch = "wasm32")]
        {
            use crate::worker_protocol::{
                WorkRequest, WorkTask, MAX_BYTES, MAX_FILES, SCHEMA_VERSION,
            };
            let state = project.snapshot_state()?;
            let retained: Vec<_> = state
                .documents
                .iter()
                .filter_map(|d| d.retained_bytes.as_ref())
                .collect();
            let bytes = retained
                .iter()
                .try_fold(0usize, |n, b| n.checked_add(b.len()))
                .ok_or("墓碑预算溢出")?;
            let files = project.snapshot_files_limited(
                MAX_FILES
                    .checked_sub(retained.len())
                    .ok_or("墓碑数量超限")?,
                MAX_BYTES.checked_sub(bytes).ok_or("墓碑字节超限")?,
            )?;
            let request = WorkRequest {
                schema_version: SCHEMA_VERSION,
                job_id: format!("localization-{version}-{generation}"),
                generation,
                baseline: project.content_baseline(),
                entry: project
                    .entry
                    .strip_prefix(&project.root)
                    .map_err(|_| "入口越界")?
                    .into(),
                snapshot_state: Some(state),
                task: WorkTask::Localization { task: task.clone() },
            };
            let worker = crate::worker_host::WorkerJob::start(request, &files, ctx)?;
            Ok(Self {
                generation,
                version,
                root: project.root.clone(),
                task,
                worker,
            })
        }
    }
    pub(crate) fn poll(&mut self) -> Option<Result<Output, String>> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            match self.receiver.try_recv() {
                Ok(result) => Some(result),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    Some(Err("本地化核对后台已结束，请重试".into()))
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            use crate::worker_protocol::{WorkEvent, WorkOutput};
            match self.worker.take_event()? {
                WorkEvent::Progress { .. } => None,
                WorkEvent::Error(error) => Some(Err(error)),
                WorkEvent::Done { output, binaries } if binaries.is_empty() => match *output {
                    WorkOutput::Localization { result } if self.task.matches_output(&result) => {
                        Some(Ok(result))
                    }
                    _ => Some(Err("本地化后台结果类型不匹配".into())),
                },
                _ => Some(Err("本地化后台包含意外负载".into())),
            }
        }
    }
    pub(crate) fn cancel(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        self.cancel
            .store(true, std::sync::atomic::Ordering::Release);
        #[cfg(target_arch = "wasm32")]
        self.worker.cancel();
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel();
    }
}
