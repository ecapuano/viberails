#![allow(clippy::unwrap_used)]

use toml::Table;

use crate::providers::codex::Codex;

fn make_codex(program: &str) -> Codex {
    Codex::with_custom_path(program).unwrap()
}

fn parse_toml(s: &str) -> Table {
    s.parse().unwrap()
}

#[test]
fn test_install_into_empty_toml() {
    let codex = make_codex("/usr/bin/test-program");
    let mut toml = parse_toml("");

    codex.install_into("notify", &mut toml).unwrap();

    let notify = toml.get("notify").unwrap().as_array().unwrap();
    assert_eq!(notify.len(), 1);
    assert_eq!(
        notify[0].as_str().unwrap(),
        "/usr/bin/test-program codex-callback"
    );
}

#[test]
fn test_install_into_existing_config() {
    let codex = make_codex("/usr/bin/test-program");
    let mut toml = parse_toml(
        r#"
model = "gpt-4"
temperature = 0.7
"#,
    );

    codex.install_into("notify", &mut toml).unwrap();

    // Existing config should be preserved
    assert_eq!(toml.get("model").unwrap().as_str().unwrap(), "gpt-4");
    assert_eq!(toml.get("temperature").unwrap().as_float().unwrap(), 0.7);

    // Notify should be added
    let notify = toml.get("notify").unwrap().as_array().unwrap();
    assert_eq!(notify.len(), 1);
}

#[test]
fn test_install_into_skips_if_already_installed() {
    let codex = make_codex("/usr/bin/test-program");
    let mut toml = parse_toml(
        r#"
notify = ["/usr/bin/test-program codex-callback"]
"#,
    );

    codex.install_into("notify", &mut toml).unwrap();

    let notify = toml.get("notify").unwrap().as_array().unwrap();
    assert_eq!(notify.len(), 1);
}

#[test]
fn test_install_into_replaces_different_notify() {
    let codex = make_codex("/usr/bin/test-program");
    let mut toml = parse_toml(
        r#"
notify = ["/other/program", "arg1"]
"#,
    );

    codex.install_into("notify", &mut toml).unwrap();

    let notify = toml.get("notify").unwrap().as_array().unwrap();
    assert_eq!(notify.len(), 1);
    assert_eq!(
        notify[0].as_str().unwrap(),
        "/usr/bin/test-program codex-callback"
    );
}

#[test]
fn test_uninstall_from_removes_our_notify() {
    let codex = make_codex("/usr/bin/test-program");
    let mut toml = parse_toml(
        r#"
notify = ["/usr/bin/test-program codex-callback"]
model = "gpt-4"
"#,
    );

    codex.uninstall_from("notify", &mut toml);

    assert!(toml.get("notify").is_none());
    // Other config should be preserved
    assert_eq!(toml.get("model").unwrap().as_str().unwrap(), "gpt-4");
}

#[test]
fn test_uninstall_from_preserves_different_notify() {
    let codex = make_codex("/usr/bin/test-program");
    let mut toml = parse_toml(
        r#"
notify = ["/other/program"]
"#,
    );

    codex.uninstall_from("notify", &mut toml);

    // Different notify should be preserved
    let notify = toml.get("notify").unwrap().as_array().unwrap();
    assert_eq!(notify[0].as_str().unwrap(), "/other/program");
}

#[test]
fn test_uninstall_from_no_notify() {
    let codex = make_codex("/usr/bin/test-program");
    let mut toml = parse_toml(
        r#"
model = "gpt-4"
"#,
    );

    // Should not panic
    codex.uninstall_from("notify", &mut toml);
}

#[test]
fn test_uninstall_from_empty_toml() {
    let codex = make_codex("/usr/bin/test-program");
    let mut toml = parse_toml("");

    // Should not panic
    codex.uninstall_from("notify", &mut toml);
}

// Discovery tests
#[test]
fn test_codex_discovery_id() {
    use crate::providers::ProviderDiscovery;
    use crate::providers::codex::CodexDiscovery;

    let discovery = CodexDiscovery;
    assert_eq!(discovery.id(), "codex");
}

#[test]
fn test_codex_discovery_display_name() {
    use crate::providers::ProviderDiscovery;
    use crate::providers::codex::CodexDiscovery;

    let discovery = CodexDiscovery;
    assert_eq!(discovery.display_name(), "OpenAI Codex CLI");
}

#[test]
fn test_codex_discovery_supported_hooks() {
    use crate::providers::ProviderDiscovery;
    use crate::providers::codex::CodexDiscovery;

    let discovery = CodexDiscovery;
    let hooks = discovery.supported_hooks();
    assert!(hooks.contains(&"notify"));
}
