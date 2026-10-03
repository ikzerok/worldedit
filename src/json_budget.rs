//! 原生后台与Web worker共用JSON输出预算；不依赖仅Web注册的协议模块。
use serde::Serialize;

const MAX_JSON_BYTES: usize = 32 * 1024 * 1024;

/// 按实际JSON字节计数，提前中止，不为检查预算复制整个输出字符串。
pub(crate) fn serialized_within(value: &impl Serialize, limit: usize) -> bool {
    struct Budget {
        used: usize,
        limit: usize,
    }
    impl std::io::Write for Budget {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            let next = self
                .used
                .checked_add(bytes.len())
                .filter(|next| *next <= self.limit)
                .ok_or_else(|| std::io::Error::other("JSON budget exceeded"))?;
            self.used = next;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    serde_json::to_writer(Budget { used: 0, limit }, value).is_ok()
}

/// 原生和Web问题任务采用同一完整输出预算；借用包装避免复制报告。
pub(crate) fn check_problem_report_output(
    report: &worldline_core::problems::ProblemsReport,
) -> Result<(), String> {
    #[derive(Serialize)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    enum BorrowedOutput<'a> {
        ProblemsReport {
            report: &'a worldline_core::problems::ProblemsReport,
        },
    }
    if !serialized_within(report, report.limits.max_report_bytes)
        || !serialized_within(&BorrowedOutput::ProblemsReport { report }, MAX_JSON_BYTES)
    {
        return Err("问题报告完整后台输出超过JSON预算".into());
    }
    Ok(())
}
