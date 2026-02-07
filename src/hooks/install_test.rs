use super::install::{parse_provider_selection, should_delete_binary};
use crate::providers::DiscoveryResult;

// =========================================================================
// should_delete_binary tests
// =========================================================================

#[test]
fn test_should_delete_binary_all_uninstalled() {
    // All 2 installed providers were selected for uninstall
    assert!(should_delete_binary(2, 2));
}

#[test]
fn test_should_delete_binary_more_selected_than_installed() {
    // Edge case: selected more than installed (shouldn't happen but handle gracefully)
    assert!(should_delete_binary(3, 2));
}

#[test]
fn test_should_delete_binary_partial_uninstall() {
    // Only 1 of 2 installed providers selected - should NOT delete binary
    assert!(!should_delete_binary(1, 2));
}

#[test]
fn test_should_delete_binary_no_selection() {
    // No providers selected for uninstall
    assert!(!should_delete_binary(0, 2));
}

#[test]
fn test_should_delete_binary_single_provider() {
    // Only 1 provider installed and it was selected
    assert!(should_delete_binary(1, 1));
}

#[test]
fn test_should_delete_binary_no_providers_installed() {
    // Edge case: no providers were installed (shouldn't reach uninstall but handle it)
    // 0 selected >= 0 installed = true
    assert!(should_delete_binary(0, 0));
}

#[test]
fn test_should_delete_binary_many_providers_partial() {
    // 5 providers installed, only 3 selected for uninstall
    assert!(!should_delete_binary(3, 5));
}

#[test]
fn test_should_delete_binary_many_providers_all() {
    // 5 providers installed, all 5 selected for uninstall
    assert!(should_delete_binary(5, 5));
}

// =========================================================================
// Bug regression tests
// =========================================================================

#[test]
fn test_bug_regression_count_before_not_after() {
    // This test documents the bug that was fixed:
    // Previously, installed_count was checked AFTER uninstall, so it would be 0
    // making all_uninstalled always true.
    //
    // With the fix, we capture installed_count BEFORE uninstall.
    // Example scenario:
    // - 2 providers installed (Claude Code, Codex)
    // - User selects only 1 for uninstall (Codex)
    // - installed_count_before = 2
    // - selected_count = 1
    // - should_delete_binary(1, 2) = false (binary preserved)
    //
    // BUG behavior (checking after):
    // - After uninstalling Codex, only Claude Code has hooks
    // - installed_count_after = 1 (but actually 0 in buggy code due to stale check)
    // - should_delete_binary(1, 0) = true (WRONG - binary deleted!)

    // Correct behavior: partial uninstall should NOT delete binary
    let installed_before = 2;
    let selected_for_uninstall = 1;
    assert!(
        !should_delete_binary(selected_for_uninstall, installed_before),
        "Partial uninstall should NOT delete binary"
    );

    // Full uninstall SHOULD delete binary
    let selected_all = 2;
    assert!(
        should_delete_binary(selected_all, installed_before),
        "Full uninstall should delete binary"
    );
}

// =========================================================================
// parse_provider_selection tests
// =========================================================================

fn make_discovery(id: &'static str, detected: bool, hooks_installed: bool) -> DiscoveryResult {
    DiscoveryResult {
        id,
        display_name: id,
        detected,
        detected_path: None,
        detection_hint: None,
        hooks_installed,
    }
}

fn sample_discoveries() -> Vec<DiscoveryResult> {
    vec![
        make_discovery("claude-code", true, true),
        make_discovery("cursor", true, false),
        make_discovery("gemini-cli", false, false),
    ]
}

#[test]
fn test_parse_all_install_returns_detected_providers() {
    let discoveries = sample_discoveries();
    let result = parse_provider_selection(&discoveries, "all", false).unwrap();
    // "all" for install should return only detected providers
    assert_eq!(result, vec!["claude-code", "cursor"]);
}

#[test]
fn test_parse_all_uninstall_returns_installed_providers() {
    let discoveries = sample_discoveries();
    let result = parse_provider_selection(&discoveries, "all", true).unwrap();
    // "all" for uninstall should return only providers with hooks installed
    assert_eq!(result, vec!["claude-code"]);
}

#[test]
fn test_parse_all_case_insensitive() {
    let discoveries = sample_discoveries();
    let result = parse_provider_selection(&discoveries, "ALL", false).unwrap();
    assert_eq!(result, vec!["claude-code", "cursor"]);

    let result = parse_provider_selection(&discoveries, "All", false).unwrap();
    assert_eq!(result, vec!["claude-code", "cursor"]);
}

#[test]
fn test_parse_all_with_whitespace() {
    let discoveries = sample_discoveries();
    let result = parse_provider_selection(&discoveries, "  all  ", false).unwrap();
    assert_eq!(result, vec!["claude-code", "cursor"]);
}

#[test]
fn test_parse_all_install_no_detected_tools() {
    let discoveries = vec![make_discovery("gemini-cli", false, false)];
    let err = parse_provider_selection(&discoveries, "all", false).unwrap_err();
    assert!(err.to_string().contains("No supported AI coding tools detected"));
}

#[test]
fn test_parse_all_uninstall_no_hooks_installed() {
    let discoveries = vec![make_discovery("claude-code", true, false)];
    let err = parse_provider_selection(&discoveries, "all", true).unwrap_err();
    assert!(err.to_string().contains("No providers have hooks installed"));
}

#[test]
fn test_parse_specific_detected_provider() {
    let discoveries = sample_discoveries();
    let result = parse_provider_selection(&discoveries, "claude-code", false).unwrap();
    assert_eq!(result, vec!["claude-code"]);
}

#[test]
fn test_parse_multiple_comma_separated() {
    let discoveries = sample_discoveries();
    let result = parse_provider_selection(&discoveries, "claude-code,cursor", false).unwrap();
    assert_eq!(result, vec!["claude-code", "cursor"]);
}

#[test]
fn test_parse_comma_separated_with_spaces() {
    let discoveries = sample_discoveries();
    let result =
        parse_provider_selection(&discoveries, "claude-code , cursor", false).unwrap();
    assert_eq!(result, vec!["claude-code", "cursor"]);
}

#[test]
fn test_parse_unknown_provider_lists_valid_ids() {
    let discoveries = sample_discoveries();
    let err = parse_provider_selection(&discoveries, "nonexistent", false).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Unknown provider ID: 'nonexistent'"));
    assert!(msg.contains("Valid IDs:"));
    assert!(msg.contains("claude-code"));
    assert!(msg.contains("cursor"));
    assert!(msg.contains("gemini-cli"));
}

#[test]
fn test_parse_empty_string_errors() {
    let discoveries = sample_discoveries();
    let err = parse_provider_selection(&discoveries, "", false).unwrap_err();
    assert!(err.to_string().contains("No provider IDs specified"));
}

#[test]
fn test_parse_only_commas_errors() {
    let discoveries = sample_discoveries();
    let err = parse_provider_selection(&discoveries, ",,,", false).unwrap_err();
    assert!(err.to_string().contains("No provider IDs specified"));
}

#[test]
fn test_parse_undetected_provider_for_install_errors() {
    let discoveries = sample_discoveries();
    // gemini-cli is not detected
    let err = parse_provider_selection(&discoveries, "gemini-cli", false).unwrap_err();
    assert!(err.to_string().contains("not detected"));
}

#[test]
fn test_parse_provider_without_hooks_for_uninstall_errors() {
    let discoveries = sample_discoveries();
    // cursor has no hooks installed
    let err = parse_provider_selection(&discoveries, "cursor", true).unwrap_err();
    assert!(err.to_string().contains("does not have hooks installed"));
}
