//! 版本闸门独立于诊断语义：取消/失败不会变成空报告，旧任务不能接管新版本。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ReportTicket {
    pub generation: u64,
    pub version: u64,
    pub baseline: String,
    pub observation: String,
}

#[derive(Default)]
pub(super) struct ReportSchedule {
    seen_version: Option<u64>,
    due_after: f64,
    generation: u64,
    attempted: bool,
    active: Option<ReportTicket>,
}

impl ReportSchedule {
    /// 返回 true 时调用方必须取消已有任务；停止输入后才允许启动最新版本。
    pub(super) fn observe(&mut self, version: u64, now: f64) -> bool {
        if self.seen_version == Some(version) {
            return false;
        }
        self.invalidate(version, now);
        true
    }

    pub(super) fn invalidate(&mut self, version: u64, now: f64) {
        self.seen_version = Some(version);
        self.due_after = now + 0.25;
        self.attempted = false;
        self.active = None;
        self.generation = self.generation.wrapping_add(1);
    }

    pub(super) fn ready(&self, now: f64) -> bool {
        self.seen_version.is_some()
            && !self.attempted
            && self.active.is_none()
            && now >= self.due_after
    }

    pub(super) fn begin(&mut self, baseline: String, observation: String) -> Option<ReportTicket> {
        let version = self.seen_version?;
        if self.active.is_some() || self.attempted {
            return None;
        }
        self.attempted = true;
        self.generation = self.generation.wrapping_add(1);
        let ticket = ReportTicket {
            generation: self.generation,
            version,
            baseline,
            observation,
        };
        self.active = Some(ticket.clone());
        Some(ticket)
    }

    pub(super) fn accepts(
        &self,
        ticket: &ReportTicket,
        version: u64,
        baseline: &str,
        observation: &str,
    ) -> bool {
        self.active.as_ref() == Some(ticket)
            && self.seen_version == Some(version)
            && ticket.version == version
            && ticket.baseline == baseline
            && ticket.observation == observation
    }

    pub(super) fn finish(&mut self) {
        self.active = None;
    }

    pub(super) fn cancel(&mut self) {
        self.active = None;
        self.attempted = true;
        self.generation = self.generation.wrapping_add(1);
    }

    /// 明确重试与自动触发分离：相同版本失败后不在每帧重启。
    pub(super) fn retry(&mut self, now: f64) {
        self.cancel();
        self.attempted = false;
        self.due_after = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_is_debounced_and_a_version_has_one_automatic_active_job() {
        let mut schedule = ReportSchedule::default();
        assert!(!schedule.ready(10.));
        assert!(schedule.observe(1, 10.));
        assert!(!schedule.ready(10.24));
        assert!(schedule.ready(10.25));
        let first = schedule.begin("one".into(), "observed".into()).unwrap();
        assert!(schedule.begin("one".into(), "observed".into()).is_none());
        assert!(!schedule.observe(1, 11.));
        assert!(schedule.accepts(&first, 1, "one", "observed"));
        assert!(!schedule.accepts(&first, 1, "changed", "observed"));
        assert!(schedule.observe(2, 11.));
        assert!(!schedule.accepts(&first, 2, "two", "observed"));
        assert!(!schedule.ready(11.2));
        assert!(schedule.ready(11.25));
    }

    #[test]
    fn cancel_or_failure_requires_explicit_retry_and_rejects_late_results() {
        let mut schedule = ReportSchedule::default();
        schedule.observe(7, 0.);
        let first = schedule.begin("same".into(), "observed".into()).unwrap();
        schedule.cancel();
        assert!(!schedule.ready(99.));
        assert!(!schedule.accepts(&first, 7, "same", "observed"));
        schedule.retry(99.);
        assert!(schedule.ready(99.));
        let second = schedule.begin("same".into(), "observed".into()).unwrap();
        assert_ne!(first.generation, second.generation);
        assert!(!schedule.accepts(&first, 7, "same", "observed"));
        assert!(schedule.accepts(&second, 7, "same", "observed"));
        schedule.finish();
        assert!(!schedule.ready(100.));
        assert!(!schedule.accepts(&second, 7, "same", "observed"));
    }
}
