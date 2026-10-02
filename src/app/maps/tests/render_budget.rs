use super::*;

fn bounded(budget: &RenderBudget) {
    let stats = budget.stats();
    assert!(stats.active <= MAX_ACTIVE);
    assert!(stats.cached + stats.reserved <= stats.limit);
}

#[test]
fn five_thousand_layers_admit_only_two_actual_jobs() {
    let budget = RenderBudget::with_limit(128 * 1024 * 1024);
    let mut leases = Vec::new();
    for layer in 0..5000 {
        match budget.start([100, 100]) {
            Ok(lease) => leases.push(lease),
            Err(AdmissionError::Waiting { .. }) => assert!(layer >= 2),
            Err(error) => panic!("unexpected admission: {error:?}"),
        }
        bounded(&budget);
    }
    assert_eq!(leases.len(), 2);
    assert_eq!(budget.stats().active, 2);
    drop(leases);
    assert_eq!(budget.stats().active, 0);
    assert_eq!(budget.stats().reserved, 0);
}

#[test]
fn completed_result_still_accounts_for_pixels_until_cache_or_drop() {
    let budget = RenderBudget::with_limit(24_000);
    let (permit, execution) = budget.start([50, 20]).unwrap();
    assert!(permit.cache(4000).is_err());
    drop(execution);
    assert_eq!(budget.stats().active, 0);
    assert_eq!(budget.stats().reserved, 12_000);
    let lease = permit.cache(4000).unwrap();
    assert_eq!(budget.stats().cached, 8000);
    assert_eq!(budget.stats().reserved, 0);
    assert!(permit.cache(4000).is_err());
    drop(permit);
    assert_eq!(budget.stats().cached, 8000);
    drop(lease);
    assert_eq!(budget.stats().cached, 0);
    bounded(&budget);
}

#[test]
fn waiting_and_permanent_workset_capacity_are_different() {
    let budget = RenderBudget::with_limit(24_000);
    let (first, running) = budget.start([50, 20]).unwrap();
    let (second, running2) = budget.start([50, 20]).unwrap();
    assert!(matches!(budget.start([50, 20]), Err(AdmissionError::Waiting { .. })));
    drop(running);
    drop(running2);
    let first_cache = first.cache(4000).unwrap();
    let second_cache = second.cache(4000).unwrap();
    assert!(matches!(budget.start([50, 20]), Err(AdmissionError::Capacity { .. })));
    drop(first_cache);
    let pending = budget.start([50, 20]).unwrap();
    bounded(&budget);
    drop(pending);
    drop(second_cache);
    assert_eq!(budget.stats().cached + budget.stats().reserved, 0);
}

#[test]
fn bad_dimensions_and_forged_results_cannot_bypass_reservation() {
    let budget = RenderBudget::with_limit(24_000);
    assert!(matches!(budget.start([0, 10]), Err(AdmissionError::InvalidDimensions)));
    assert!(matches!(budget.start([100, 100]), Err(AdmissionError::Oversized { .. })));
    assert!(budget.start([u32::MAX, u32::MAX]).is_err());
    let (permit, execution) = budget.start([50, 20]).unwrap();
    drop(execution);
    assert!(permit.cache(4004).is_err());
    assert!(permit.cache(3).is_err());
    bounded(&budget);
    drop(permit);
    assert_eq!(budget.stats().reserved, 0);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn closing_ui_does_not_release_a_still_running_native_thread() {
    use std::sync::mpsc;
    let budget = RenderBudget::with_limit(24_000);
    let (ui_permit, execution) = budget.start([50, 20]).unwrap();
    let (release, wait) = mpsc::channel::<()>();
    let thread = std::thread::spawn(move || {
        let _execution = execution;
        wait.recv().unwrap();
    });
    drop(ui_permit);
    assert_eq!(budget.stats().active, 1);
    assert_eq!(budget.stats().reserved, 12_000);
    let second = budget.start([50, 20]).unwrap();
    assert!(matches!(budget.start([50, 20]), Err(AdmissionError::Waiting { .. })));
    release.send(()).unwrap();
    thread.join().unwrap();
    assert_eq!(budget.stats().active, 1);
    drop(second);
    assert_eq!(budget.stats().active, 0);
    assert_eq!(budget.stats().reserved, 0);
}

#[test]
fn failure_close_and_repeat_do_not_leak_or_complete_another_job() {
    let budget = RenderBudget::with_limit(24_000);
    for index in 0..5000 {
        let (permit, execution) = budget.start([50, 20]).unwrap();
        if index % 2 == 0 {
            drop(permit);
            assert_eq!(budget.stats().active, 1);
            drop(execution);
        } else {
            drop(execution);
            let pixels_in_channel = permit.clone();
            drop(permit);
            assert_eq!(budget.stats().reserved, 12_000);
            drop(pixels_in_channel);
        }
        bounded(&budget);
        assert_eq!(budget.stats().cached + budget.stats().reserved, 0);
    }
}
