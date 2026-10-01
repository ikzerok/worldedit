use worldline_core::timeline::{TemporalEvent, TemporalOrderScope, Timeline, TimelineStatus};

pub(super) fn summary(timeline: &Timeline) -> &'static str {
    if timeline.status == TimelineStatus::Partial {
        "时间线不完整：存在编译错误，层级未知；请查看诊断"
    } else if timeline.order_scope == TemporalOrderScope::RootPeriod {
        "同一时间根内按约束分列 · 独立根不可比 · 同层不代表同时"
    } else {
        "直接时段内按约束分列 · 同层不代表同时"
    }
}

pub(super) fn caption(timeline: &Timeline, event: Option<&TemporalEvent>) -> String {
    let Some(event) = event else {
        return "未归组 · 时间未知".into();
    };
    match timeline.event_rank(event) {
        Some(rank) => format!(
            "{} · 层 {rank}",
            event.order_scope.as_deref().unwrap_or("未知范围")
        ),
        None => "不完整 · 层级未知".into(),
    }
}

pub(super) fn details(timeline: &Timeline, event: Option<&TemporalEvent>) -> String {
    let Some(event) = event else {
        return "未指定有效时段；不推测日期或顺序".into();
    };
    let scope = event.order_scope.as_deref().unwrap_or("未知");
    let root = event.root.as_deref().unwrap_or("未知");
    let order = timeline
        .event_rank(event)
        .map_or_else(|| "未知".into(), |r| r.to_string());
    let direct = if event.status == TimelineStatus::Complete {
        event.rank.to_string()
    } else {
        "未知".into()
    };
    format!("直接时段：{}\n时间根：{root}\n比较范围：{scope}\n显示层级：{order}\n直接时段内部层级：{direct}\n不同层级本身不证明先后；双击定位源码", event.period)
}

#[cfg(test)]
mod tests {
    use super::*;
    use worldline_core::{compile_source_with_options, CompileOptions};

    const SOURCE: &str = "period year\nperiod summer within year\nperiod autumn within year\nevent opening during summer\n  开幕\nevent closing during autumn follows opening\n  闭幕\n";

    #[test]
    fn root_columns_preserve_native_bands_and_source_details() {
        let result = compile_source_with_options("seasons.wl", SOURCE, CompileOptions::v1_13());
        assert!(!result.has_errors(), "{:?}", result.diagnostics);
        let timeline = &result.analysis.timeline;
        assert_eq!(timeline.events[1].period, "autumn");
        assert_eq!(timeline.event_rank(&timeline.events[1]), Some(1));
        assert_eq!(timeline.events[1].rank, 0);
        assert_eq!(caption(timeline, Some(&timeline.events[1])), "year · 层 1");
        let text = details(timeline, Some(&timeline.events[1]));
        assert!(text.contains("直接时段：autumn") && text.contains("时间根：year"));
        assert!(summary(timeline).contains("独立根不可比"));
    }

    #[test]
    fn invalid_and_empty_graphs_do_not_claim_trusted_layers() {
        for source in [
            SOURCE.replace("follows opening", "follows missing"),
            "period\n".into(),
        ] {
            let result = compile_source_with_options("bad.wl", &source, CompileOptions::v1_13());
            assert!(result.has_errors());
            let timeline = &result.analysis.timeline;
            assert!(summary(timeline).contains("不完整"));
            for event in &timeline.events {
                assert!(caption(timeline, Some(event)).contains("层级未知"));
                assert_eq!(timeline.event_rank(event), None);
            }
        }
    }
}
