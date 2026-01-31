#![allow(clippy::unwrap_used)]

use crate::providers::discovery::{DiscoveryResult, ProviderDiscovery, ProviderFactory};
use crate::providers::{HookEntry, LLmProviderTrait};

/// Mock provider for testing discovery system
struct MockProvider {
    name: &'static str,
}

impl LLmProviderTrait for MockProvider {
    fn name(&self) -> &'static str {
        self.name
    }

    fn install(&self, _hook_type: &str) -> anyhow::Result<()> {
        Ok(())
    }

    fn uninstall(&self, _hook_type: &str) -> anyhow::Result<()> {
        Ok(())
    }

    fn list(&self) -> anyhow::Result<Vec<HookEntry>> {
        Ok(vec![])
    }
}

/// Mock discovery that always detects
struct AlwaysDetectedDiscovery;

impl ProviderDiscovery for AlwaysDetectedDiscovery {
    fn id(&self) -> &'static str {
        "always-detected"
    }

    fn display_name(&self) -> &'static str {
        "Always Detected"
    }

    fn discover(&self) -> DiscoveryResult {
        DiscoveryResult {
            id: self.id(),
            display_name: self.display_name(),
            detected: true,
            detected_path: Some("/mock/path".into()),
            detection_hint: None,
        }
    }

    fn supported_hooks(&self) -> &'static [&'static str] {
        &["PreToolUse", "PostToolUse"]
    }
}

impl ProviderFactory for AlwaysDetectedDiscovery {
    fn create(&self) -> anyhow::Result<Box<dyn LLmProviderTrait>> {
        Ok(Box::new(MockProvider {
            name: "always-detected",
        }))
    }
}

/// Mock discovery that never detects
struct NeverDetectedDiscovery;

impl ProviderDiscovery for NeverDetectedDiscovery {
    fn id(&self) -> &'static str {
        "never-detected"
    }

    fn display_name(&self) -> &'static str {
        "Never Detected"
    }

    fn discover(&self) -> DiscoveryResult {
        DiscoveryResult {
            id: self.id(),
            display_name: self.display_name(),
            detected: false,
            detected_path: None,
            detection_hint: Some("Install from example.com".into()),
        }
    }

    fn supported_hooks(&self) -> &'static [&'static str] {
        &["CustomHook"]
    }
}

impl ProviderFactory for NeverDetectedDiscovery {
    fn create(&self) -> anyhow::Result<Box<dyn LLmProviderTrait>> {
        anyhow::bail!("Not detected, cannot create")
    }
}

// Tests for DiscoveryResult
#[test]
fn test_discovery_result_detected() {
    let discovery = AlwaysDetectedDiscovery;
    let result = discovery.discover();

    assert!(result.detected);
    assert!(result.detected_path.is_some());
    assert_eq!(result.id, "always-detected");
    assert_eq!(result.display_name, "Always Detected");
    assert!(result.detection_hint.is_none());
}

#[test]
fn test_discovery_result_not_detected() {
    let discovery = NeverDetectedDiscovery;
    let result = discovery.discover();

    assert!(!result.detected);
    assert!(result.detected_path.is_none());
    assert_eq!(result.id, "never-detected");
    assert_eq!(result.display_name, "Never Detected");
    assert!(result.detection_hint.is_some());
    assert_eq!(
        result.detection_hint.as_deref(),
        Some("Install from example.com")
    );
}

// Tests for ProviderDiscovery trait
#[test]
fn test_provider_discovery_id() {
    let discovery = AlwaysDetectedDiscovery;
    assert_eq!(discovery.id(), "always-detected");
}

#[test]
fn test_provider_discovery_display_name() {
    let discovery = AlwaysDetectedDiscovery;
    assert_eq!(discovery.display_name(), "Always Detected");
}

#[test]
fn test_provider_discovery_supported_hooks() {
    let discovery = AlwaysDetectedDiscovery;
    let hooks = discovery.supported_hooks();
    assert_eq!(hooks.len(), 2);
    assert!(hooks.contains(&"PreToolUse"));
    assert!(hooks.contains(&"PostToolUse"));
}

// Tests for ProviderFactory trait
#[test]
fn test_provider_factory_create_success() {
    let discovery = AlwaysDetectedDiscovery;
    let provider = discovery.create();

    assert!(provider.is_ok());
    let provider = provider.unwrap();
    assert_eq!(provider.name(), "always-detected");
}

#[test]
fn test_provider_factory_create_failure() {
    let discovery = NeverDetectedDiscovery;
    let provider = discovery.create();

    assert!(provider.is_err());
}

// Tests for ClaudeDiscovery (the real implementation)
#[test]
fn test_claude_discovery_id() {
    use crate::providers::claude::ClaudeDiscovery;

    let discovery = ClaudeDiscovery;
    assert_eq!(discovery.id(), "claude-code");
}

#[test]
fn test_claude_discovery_display_name() {
    use crate::providers::claude::ClaudeDiscovery;

    let discovery = ClaudeDiscovery;
    assert_eq!(discovery.display_name(), "Claude Code");
}

#[test]
fn test_claude_discovery_supported_hooks() {
    use crate::providers::claude::ClaudeDiscovery;

    let discovery = ClaudeDiscovery;
    let hooks = discovery.supported_hooks();

    assert!(hooks.contains(&"PreToolUse"));
    assert!(hooks.contains(&"UserPromptSubmit"));
}

#[test]
fn test_claude_discovery_returns_valid_result() {
    use crate::providers::claude::ClaudeDiscovery;

    let discovery = ClaudeDiscovery;
    let result = discovery.discover();

    // The result should always have valid id and display_name
    assert_eq!(result.id, "claude-code");
    assert_eq!(result.display_name, "Claude Code");

    // detection_hint should always be present for uninstalled tools
    if !result.detected {
        assert!(result.detection_hint.is_some());
    }
}

#[test]
fn test_claude_discovery_create_provider() {
    use crate::providers::claude::ClaudeDiscovery;

    let discovery = ClaudeDiscovery;
    let result = discovery.create();

    // Should succeed (just creates the struct, doesn't require file to exist)
    assert!(result.is_ok());
}
