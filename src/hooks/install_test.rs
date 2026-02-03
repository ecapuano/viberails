use super::install::should_delete_binary;

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
