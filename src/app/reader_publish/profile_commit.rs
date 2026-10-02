use super::super::WorldeditApp;

impl WorldeditApp {
    /// 配置不改语言内容。只在分析原本新鲜且提交前后源码/选项不变时复用分析。
    /// 计划始终由core原API完整重算；不接收后台任意工程或after字节。
    pub(super) fn apply_reader_profile_plan(&mut self) -> bool {
        let Some((input, plan)) = self.reader_publish.profile_plan.as_ref() else {
            return false;
        };
        #[cfg(all(test, not(debug_assertions)))]
        let measured = std::time::Instant::now();
        if input != &self.reader_profile_input() || !super::profile_job::plan_matches(input, plan) {
            self.reader_publish.status =
                Some("配置保存计划已过期；输入已保留，请重新核对。".into());
            return false;
        }
        #[cfg(all(test, not(debug_assertions)))]
        let scope_time = measured.elapsed();
        let sources = self.project.sources();
        let options = self.project.compile_options();
        let fresh = self.snapshot.as_ref().is_some_and(|snapshot| {
            snapshot.result.sources == sources && snapshot.result.options == options
        });
        if self.stale_form || !fresh {
            self.reader_publish.status =
                Some("当前分析已过期；配置输入和计划已保留，请先刷新工程分析后重试。".into());
            return false;
        }
        let before = self.project.clone();
        #[cfg(all(test, not(debug_assertions)))]
        let before_core = measured.elapsed();
        if let Err(error) = self.project.apply_save_reader_profile(plan) {
            self.io_error = Some(error.clone());
            self.reader_publish.status = Some(format!("配置未应用：{error}"));
            return false;
        }
        #[cfg(all(test, not(debug_assertions)))]
        let after_core = measured.elapsed();
        if self.project.sources() != sources || self.project.compile_options() != options {
            self.project = before;
            self.reader_publish.status =
                Some("配置应用意外改变语言内容，已回滚；输入与计划保留。".into());
            return false;
        }
        let profile = plan.profile.clone();
        self.remember(before);
        self.version += 1;
        self.map_revision.content_generation = self.map_revision.content_generation.wrapping_add(1);
        self.map_canvas.invalidate_rasters();
        self.io_error = None;
        self.message = Some("发布配置已应用；保存全部可写入工作区".into());
        if let Some(old) = self
            .reader_publish
            .profiles
            .iter_mut()
            .find(|old| old.id == profile.id)
        {
            *old = profile.clone();
        } else {
            self.reader_publish.profiles.push(profile.clone());
            self.reader_publish.profiles.sort_by(|a, b| a.id.cmp(&b.id));
        }
        self.reader_publish.profile_error = None;
        self.reader_publish.profile_plan = None;
        self.reader_publish.load_profile(profile);
        self.reader_publish.status = Some("配置已应用、可撤销；尚需保存全部。".into());
        #[cfg(all(test, not(debug_assertions)))]
        eprintln!(
            "PROFILE_APPLY_PHASE scope_us={} freshness_clone_us={} core_us={} history_ui_us={}",
            scope_time.as_micros(),
            (before_core - scope_time).as_micros(),
            (after_core - before_core).as_micros(),
            (measured.elapsed() - after_core).as_micros()
        );
        true
    }
}
