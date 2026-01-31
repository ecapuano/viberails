#![allow(clippy::unwrap_used)]

use serde_json::json;

use crate::common::PROJECT_NAME;
use crate::providers::opencode::OpenCode;

fn make_opencode(program: &str) -> OpenCode {
    OpenCode::with_custom_path(program).unwrap()
}

#[test]
fn test_install_into_empty_json() {
    let opencode = make_opencode("/usr/bin/test-program");
    let mut json = json!({});

    opencode.install_into("plugins", &mut json).unwrap();

    let plugins = &json["plugins"];
    assert!(plugins.is_object());
    let entry = &plugins[PROJECT_NAME];
    assert_eq!(entry["enabled"], true);
    assert_eq!(entry["command"], "/usr/bin/test-program opencode-callback");
    // Note: description comes from production code which capitalizes the name
    assert!(
        entry["description"]
            .as_str()
            .unwrap()
            .contains("security hooks")
    );
}

#[test]
fn test_install_into_existing_plugins() {
    let opencode = make_opencode("/usr/bin/test-program");
    let mut json = json!({
        "plugins": {
            "other-plugin": {
                "enabled": true,
                "command": "/other/plugin"
            }
        }
    });

    opencode.install_into("plugins", &mut json).unwrap();

    // Other plugin should be preserved
    assert!(
        json["plugins"]["other-plugin"]["enabled"]
            .as_bool()
            .unwrap()
    );
    // Our plugin should be added
    assert!(json["plugins"][PROJECT_NAME]["enabled"].as_bool().unwrap());
}

#[test]
fn test_install_into_skips_if_already_installed() {
    let opencode = make_opencode("/usr/bin/test-program");
    let mut json = json!({
        "plugins": {
            PROJECT_NAME: {
                "enabled": true,
                "command": "/usr/bin/test-program opencode-callback"
            }
        }
    });

    opencode.install_into("plugins", &mut json).unwrap();

    // Should still have only one entry
    assert!(json["plugins"][PROJECT_NAME].is_object());
}

#[test]
fn test_install_into_updates_different_command() {
    let opencode = make_opencode("/usr/bin/test-program");
    let mut json = json!({
        "plugins": {
            PROJECT_NAME: {
                "enabled": false,
                "command": "/old/path opencode-callback"
            }
        }
    });

    opencode.install_into("plugins", &mut json).unwrap();

    // Should be updated with new command
    assert_eq!(
        json["plugins"][PROJECT_NAME]["command"],
        "/usr/bin/test-program opencode-callback"
    );
    assert!(json["plugins"][PROJECT_NAME]["enabled"].as_bool().unwrap());
}

#[test]
fn test_install_into_preserves_other_config() {
    let opencode = make_opencode("/usr/bin/test-program");
    let mut json = json!({
        "model": "gpt-4",
        "temperature": 0.7
    });

    opencode.install_into("plugins", &mut json).unwrap();

    assert_eq!(json["model"], "gpt-4");
    assert_eq!(json["temperature"], 0.7);
}

#[test]
fn test_uninstall_from_removes_entry() {
    let opencode = make_opencode("/usr/bin/test-program");
    let mut json = json!({
        "plugins": {
            PROJECT_NAME: {
                "enabled": true,
                "command": "/usr/bin/test-program opencode-callback"
            },
            "other-plugin": {
                "enabled": true,
                "command": "/other/plugin"
            }
        }
    });

    opencode.uninstall_from("plugins", &mut json);

    assert!(json["plugins"].get(PROJECT_NAME).is_none());
    // Other plugin should be preserved
    assert!(json["plugins"]["other-plugin"].is_object());
}

#[test]
fn test_uninstall_from_no_plugins() {
    let opencode = make_opencode("/usr/bin/test-program");
    let mut json = json!({});

    // Should not panic
    opencode.uninstall_from("plugins", &mut json);
}

#[test]
fn test_uninstall_from_no_entry() {
    let opencode = make_opencode("/usr/bin/test-program");
    let mut json = json!({
        "plugins": {
            "other-plugin": {
                "enabled": true
            }
        }
    });

    // Should not panic
    opencode.uninstall_from("plugins", &mut json);

    // Other plugin should be unchanged
    assert!(
        json["plugins"]["other-plugin"]["enabled"]
            .as_bool()
            .unwrap()
    );
}

// Discovery tests
#[test]
fn test_opencode_discovery_id() {
    use crate::providers::ProviderDiscovery;
    use crate::providers::opencode::OpenCodeDiscovery;

    let discovery = OpenCodeDiscovery;
    assert_eq!(discovery.id(), "opencode");
}

#[test]
fn test_opencode_discovery_display_name() {
    use crate::providers::ProviderDiscovery;
    use crate::providers::opencode::OpenCodeDiscovery;

    let discovery = OpenCodeDiscovery;
    assert_eq!(discovery.display_name(), "OpenCode");
}

#[test]
fn test_opencode_discovery_supported_hooks() {
    use crate::providers::ProviderDiscovery;
    use crate::providers::opencode::OpenCodeDiscovery;

    let discovery = OpenCodeDiscovery;
    let hooks = discovery.supported_hooks();
    assert!(hooks.contains(&"plugins"));
}
