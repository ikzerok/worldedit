use super::*;
#[cfg(not(target_arch = "wasm32"))]
use worldline_core::catalog_import::parse_catalog_csv;
use worldline_core::project::Project;

pub(super) enum JobResult {
    Parsed(CatalogCsvTable),
    Preview(CatalogImportPlan),
}

pub(super) struct ImportJob {
    pub generation: u64,
    pub baseline: Option<String>,
    pub cancelled: bool,
    #[cfg(not(target_arch = "wasm32"))]
    receiver: std::sync::mpsc::Receiver<Result<JobResult, String>>,
    #[cfg(target_arch = "wasm32")]
    worker: crate::worker_host::WorkerJob,
}

impl ImportJob {
    pub(super) fn parse(csv: String, generation: u64, ctx: &egui::Context) -> Result<Self, String> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self::native(generation, None, ctx, move || {
                parse_catalog_csv(&csv)
                    .map(JobResult::Parsed)
                    .map_err(|error| format!("{}：{}", error.code, error.message))
            })
        }
        #[cfg(target_arch = "wasm32")]
        {
            use crate::worker_protocol::{WorkRequest, WorkTask, SCHEMA_VERSION};
            let request = WorkRequest {
                schema_version: SCHEMA_VERSION,
                job_id: format!("catalog-csv-{generation}"),
                generation,
                baseline: String::new(),
                entry: PathBuf::new(),
                snapshot_state: None,
                task: WorkTask::CatalogCsvParse { csv },
            };
            let worker = crate::worker_host::WorkerJob::start(request, &Default::default(), ctx)?;
            Ok(Self {
                generation,
                baseline: None,
                cancelled: false,
                worker,
            })
        }
    }

    pub(super) fn preview(
        project: &Project,
        request: CatalogImportRequest,
        generation: u64,
        ctx: &egui::Context,
    ) -> Result<Self, String> {
        let baseline = request.expected_baseline.clone();
        #[cfg(not(target_arch = "wasm32"))]
        {
            let project = project.clone();
            Self::native(generation, Some(baseline), ctx, move || {
                project
                    .preview_catalog_import(&request)
                    .map(JobResult::Preview)
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
                .try_fold(0usize, |n, bytes| n.checked_add(bytes.len()))
                .ok_or("墓碑预算溢出")?;
            let files = project.snapshot_files_limited(
                MAX_FILES
                    .checked_sub(retained.len())
                    .ok_or("墓碑数量超限")?,
                MAX_BYTES.checked_sub(bytes).ok_or("墓碑字节超限")?,
            )?;
            let work = WorkRequest {
                schema_version: SCHEMA_VERSION,
                job_id: format!("catalog-preview-{generation}"),
                generation,
                baseline: baseline.clone(),
                entry: project
                    .entry
                    .strip_prefix(&project.root)
                    .map_err(|_| "工程入口越界")?
                    .to_owned(),
                snapshot_state: Some(state),
                task: WorkTask::CatalogImportPreview { request },
            };
            let worker = crate::worker_host::WorkerJob::start(work, &files, ctx)?;
            Ok(Self {
                generation,
                baseline: Some(baseline),
                cancelled: false,
                worker,
            })
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn native(
        generation: u64,
        baseline: Option<String>,
        ctx: &egui::Context,
        work: impl FnOnce() -> Result<JobResult, String> + Send + 'static,
    ) -> Result<Self, String> {
        let ctx = ctx.clone();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("catalog-import".into())
            .spawn(move || {
                let _ = sender.send(work());
                ctx.request_repaint();
            })
            .map_err(|e| format!("无法启动资料导入检查：{e}"))?;
        Ok(Self {
            generation,
            baseline,
            cancelled: false,
            receiver,
        })
    }

    pub(super) fn cancel(&mut self) {
        self.cancelled = true;
        #[cfg(target_arch = "wasm32")]
        self.worker.cancel();
    }

    fn poll(&mut self) -> Option<Result<JobResult, String>> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            match self.receiver.try_recv() {
                Ok(result) => Some(result),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    Some(Err("导入后台任务已中断".into()))
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            use crate::worker_protocol::{WorkEvent, WorkOutput};
            if self.cancelled {
                return Some(Err("已取消".into()));
            }
            match self.worker.take_event()? {
                WorkEvent::Progress { .. } => None,
                WorkEvent::Error(error) => Some(Err(error)),
                WorkEvent::Done { output, binaries } if binaries.is_empty() => match *output {
                    WorkOutput::CatalogCsvParse { table } => Some(Ok(JobResult::Parsed(table))),
                    WorkOutput::CatalogImportPreview { plan } => Some(Ok(JobResult::Preview(plan))),
                    _ => Some(Err("资料导入后台结果类型不匹配".into())),
                },
                _ => Some(Err("资料导入后台结果包含意外负载".into())),
            }
        }
    }
}

impl ImportState {
    pub(super) fn start_parse(&mut self, ctx: &egui::Context) {
        if self.job.is_some() || self.source_name.is_empty() {
            return;
        }
        match ImportJob::parse(self.csv.clone(), self.generation, ctx) {
            Ok(job) => {
                self.job = Some(job);
                self.status = Some("正在后台读取 CSV 快照…".into());
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub(super) fn poll(&mut self, app: &WorldeditApp, ctx: &egui::Context) {
        let Some(job) = self.job.as_mut() else {
            return;
        };
        let Some(result) = job.poll() else {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
            return;
        };
        let job = self.job.take().expect("polled task exists");
        if job.cancelled
            || job.generation != self.generation
            || job
                .baseline
                .as_ref()
                .is_some_and(|baseline| baseline != &app.project.content_baseline())
        {
            self.acknowledged = false;
            self.stale = self.plan.is_some();
            self.status = Some("旧任务结果已丢弃；输入完整保留，请重新读取或刷新预览。".into());
            return;
        }
        match result {
            Ok(JobResult::Parsed(table)) => {
                self.columns = vec![None; table.headers.len()];
                self.table = Some(table);
                self.status = Some("快照已读取；请为每一列明确映射或忽略。".into());
                self.error = None;
            }
            Ok(JobResult::Preview(plan)) => {
                self.selected_row = 0;
                self.stale = false;
                self.submitted =
                    plan.error_count == 0 && plan.rows.iter().all(|r| r.operation == "unchanged");
                self.applied_baseline = self.submitted.then(|| app.project.content_baseline());
                self.plan = Some(plan);
                self.step = Step::Review;
                self.status = Some(
                    if self.submitted {
                        "同批资料没有变化；未修改工程，也未新增撤销记录。"
                    } else {
                        "预览就绪；审阅真实字段差异后再确认整批应用。"
                    }
                    .into(),
                );
                self.error = None;
            }
            Err(error) => {
                self.error = Some(error);
                self.acknowledged = false;
            }
        }
    }
}
