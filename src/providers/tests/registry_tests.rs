#![allow(clippy::unwrap_used)]

use crate::providers::registry::ProviderRegistry;

#[test]
fn test_registry_new_creates_default_registry() {
    let registry = ProviderRegistry::new();

    // Should have at least one provider (Claude Code)
    let discoveries = registry.discover_all();
    assert!(!discoveries.is_empty());
}

#[test]
fn test_registry_default_same_as_new() {
    let registry1 = ProviderRegistry::new();
    let registry2 = ProviderRegistry::default();

    // Both should have the same providers
    let discoveries1 = registry1.discover_all();
    let discoveries2 = registry2.discover_all();

    assert_eq!(discoveries1.len(), discoveries2.len());
    for (d1, d2) in discoveries1.iter().zip(discoveries2.iter()) {
        assert_eq!(d1.id, d2.id);
    }
}

#[test]
fn test_registry_discover_all_returns_results() {
    let registry = ProviderRegistry::new();
    let discoveries = registry.discover_all();

    for discovery in &discoveries {
        // Each discovery should have valid id and display_name
        assert!(!discovery.id.is_empty());
        assert!(!discovery.display_name.is_empty());

        // If not detected, should have a hint
        if !discovery.detected {
            assert!(discovery.detection_hint.is_some());
        }
    }
}

#[test]
fn test_registry_get_existing_provider() {
    let registry = ProviderRegistry::new();

    // Claude Code should be registered
    let factory = registry.get("claude-code");
    assert!(factory.is_some());

    let factory = factory.unwrap();
    assert_eq!(factory.id(), "claude-code");
    assert_eq!(factory.display_name(), "Claude Code");
}

#[test]
fn test_registry_get_nonexistent_provider() {
    let registry = ProviderRegistry::new();

    let factory = registry.get("nonexistent-provider");
    assert!(factory.is_none());
}

#[test]
fn test_registry_all_returns_iterator() {
    let registry = ProviderRegistry::new();

    let mut count = 0;
    for factory in registry.all() {
        count += 1;
        // Each factory should have valid id
        assert!(!factory.id().is_empty());
    }

    // Should have at least one provider
    assert!(count > 0);
}

#[test]
fn test_registry_contains_claude_code() {
    let registry = ProviderRegistry::new();

    let has_claude = registry.all().any(|f| f.id() == "claude-code");
    assert!(has_claude);
}

#[test]
fn test_registry_provider_can_create_instance() {
    let registry = ProviderRegistry::new();

    let factory = registry.get("claude-code").unwrap();
    let result = factory.create();

    // Should be able to create a provider instance
    assert!(result.is_ok());
}

#[test]
fn test_registry_provider_supported_hooks() {
    let registry = ProviderRegistry::new();

    let factory = registry.get("claude-code").unwrap();
    let hooks = factory.supported_hooks();

    // Claude Code should support at least PreToolUse
    assert!(hooks.contains(&"PreToolUse"));
}

#[test]
fn test_registry_discover_all_matches_all_iterator() {
    let registry = ProviderRegistry::new();

    let discoveries = registry.discover_all();
    let all_count = registry.all().count();

    // Should have same number of providers
    assert_eq!(discoveries.len(), all_count);
}

#[test]
fn test_registry_excludes_openclaw() {
    // OpenClaw is intentionally excluded from the registry until it adds proper hook support.
    // The implementation is preserved in openclaw.rs for future use.
    // See: https://github.com/refractionPOINT/project-west-coast/issues/XXX (if applicable)
    let registry = ProviderRegistry::new();

    let has_openclaw = registry.all().any(|f| f.id() == "openclaw");
    assert!(
        !has_openclaw,
        "OpenClaw should NOT be in the registry - it was intentionally disabled"
    );

    // Also verify via get()
    assert!(
        registry.get("openclaw").is_none(),
        "OpenClaw should not be retrievable from registry"
    );
}
