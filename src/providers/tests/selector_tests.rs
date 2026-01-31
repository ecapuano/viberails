#![allow(clippy::unwrap_used, clippy::useless_vec)]

use crate::providers::discovery::DiscoveryResult;

// Helper function to create a mock detected discovery result
fn make_detected_result(id: &'static str, name: &'static str) -> DiscoveryResult {
    DiscoveryResult {
        id,
        display_name: name,
        detected: true,
        detected_path: Some("/mock/path".into()),
        detection_hint: None,
        hooks_installed: false,
    }
}

// Helper function to create a mock undetected discovery result
fn make_undetected_result(id: &'static str, name: &'static str) -> DiscoveryResult {
    DiscoveryResult {
        id,
        display_name: name,
        detected: false,
        detected_path: None,
        detection_hint: Some(format!("Install {name} from example.com")),
        hooks_installed: false,
    }
}

// Tests for SelectionResult
#[test]
fn test_selection_result_empty() {
    use crate::providers::selector::SelectionResult;

    let result = SelectionResult {
        selected_ids: vec![],
    };

    assert!(result.selected_ids.is_empty());
}

#[test]
fn test_selection_result_single_selection() {
    use crate::providers::selector::SelectionResult;

    let result = SelectionResult {
        selected_ids: vec!["claude-code"],
    };

    assert_eq!(result.selected_ids.len(), 1);
    assert!(result.selected_ids.contains(&"claude-code"));
}

#[test]
fn test_selection_result_multiple_selections() {
    use crate::providers::selector::SelectionResult;

    let result = SelectionResult {
        selected_ids: vec!["claude-code", "cursor", "windsurf"],
    };

    assert_eq!(result.selected_ids.len(), 3);
    assert!(result.selected_ids.contains(&"claude-code"));
    assert!(result.selected_ids.contains(&"cursor"));
    assert!(result.selected_ids.contains(&"windsurf"));
}

// Tests for DiscoveryResult display behavior
#[test]
fn test_detected_discovery_has_no_hint() {
    let result = make_detected_result("test", "Test Provider");

    assert!(result.detected);
    assert!(result.detection_hint.is_none());
}

#[test]
fn test_undetected_discovery_has_hint() {
    let result = make_undetected_result("test", "Test Provider");

    assert!(!result.detected);
    assert!(result.detection_hint.is_some());
    assert!(result.detection_hint.unwrap().contains("Test Provider"));
}

// Tests for filtering detected providers
#[test]
fn test_filter_detected_providers() {
    let discoveries = vec![
        make_detected_result("detected1", "Detected 1"),
        make_undetected_result("undetected1", "Undetected 1"),
        make_detected_result("detected2", "Detected 2"),
        make_undetected_result("undetected2", "Undetected 2"),
    ];

    let detected: Vec<_> = discoveries.iter().filter(|d| d.detected).collect();

    assert_eq!(detected.len(), 2);
    assert!(detected.iter().all(|d| d.detected));
}

#[test]
fn test_filter_undetected_providers() {
    let discoveries = vec![
        make_detected_result("detected1", "Detected 1"),
        make_undetected_result("undetected1", "Undetected 1"),
        make_detected_result("detected2", "Detected 2"),
        make_undetected_result("undetected2", "Undetected 2"),
    ];

    let undetected: Vec<_> = discoveries.iter().filter(|d| !d.detected).collect();

    assert_eq!(undetected.len(), 2);
    assert!(undetected.iter().all(|d| !d.detected));
}

#[test]
fn test_all_providers_undetected() {
    let discoveries = vec![
        make_undetected_result("undetected1", "Undetected 1"),
        make_undetected_result("undetected2", "Undetected 2"),
    ];

    let any_detected = discoveries.iter().any(|d| d.detected);
    assert!(!any_detected);
}

#[test]
fn test_all_providers_detected() {
    let discoveries = vec![
        make_detected_result("detected1", "Detected 1"),
        make_detected_result("detected2", "Detected 2"),
    ];

    let all_detected = discoveries.iter().all(|d| d.detected);
    assert!(all_detected);
}

// Tests for extracting IDs from selections
#[test]
fn test_extract_ids_from_detected() {
    let discoveries = vec![
        make_detected_result("tool-a", "Tool A"),
        make_detected_result("tool-b", "Tool B"),
        make_undetected_result("tool-c", "Tool C"),
    ];

    let detected_ids: Vec<_> = discoveries
        .iter()
        .filter(|d| d.detected)
        .map(|d| d.id)
        .collect();

    assert_eq!(detected_ids, vec!["tool-a", "tool-b"]);
}
