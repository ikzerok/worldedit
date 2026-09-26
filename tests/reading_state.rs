#[path = "../src/app/reading_state.rs"]
mod reading_state;
use reading_state::ReadingPanels;
use worldline_core::TargetRef;

#[test]
fn pinned_navigation_and_back_are_independent_and_bounded() {
    let mut panels = ReadingPanels::default();
    let a = TargetRef::new("entity", "a");
    let b = TargetRef::new("entity", "b");
    let first = panels.pin(a.clone()).unwrap();
    let second = panels.pin(b.clone()).unwrap();
    assert!(panels.pin(a.clone()).is_none());
    panels.navigate(first, b.clone());
    assert_eq!(panels.get(second).unwrap().target, b);
    assert!(panels.get(second).unwrap().history.is_empty());
    panels.back(first);
    assert_eq!(panels.get(first).unwrap().target, a);
    for i in 0..100 {
        panels.navigate(first, TargetRef::new("entity", &format!("e{i}")));
    }
    assert_eq!(panels.get(first).unwrap().history.len(), 64);
    panels.close(first);
    assert!(panels.get(first).is_none());
    assert!(panels.pin(a).is_some());
    panels.clear();
    assert!(panels.ids().is_empty());
}
