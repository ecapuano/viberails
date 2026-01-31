#![allow(clippy::unwrap_used)]

use serde_json::json;

use super::opencode::OpenCode;

fn make_opencode(program: &str) -> OpenCode {
    OpenCode::new(program).unwrap()
}

#[test]
fn test_install_into_empty_json() {
    let opencode = make_opencode("/usr/bin/test-program");
    let mut json = json!({});

    opencode.install_into("plugins", &mut json).unwrap();

    let plugins = &json["plugins"];
    assert!(plugins.is_object());
    let viberails = &plugins["viberails"];
    assert_eq!(viberails["enabled"], true);
    assert_eq!(
        viberails["command"],
        "/usr/bin/test-program opencode-callback"
    );
    assert_eq!(viberails["description"], "Viberails security hooks");
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
    assert!(json["plugins"]["other-plugin"]["enabled"].as_bool().unwrap());
    // Our plugin should be added
    assert!(json["plugins"]["viberails"]["enabled"].as_bool().unwrap());
}

#[test]
fn test_install_into_skips_if_already_installed() {
    let opencode = make_opencode("/usr/bin/test-program");
    let mut json = json!({
        "plugins": {
            "viberails": {
                "enabled": true,
                "command": "/usr/bin/test-program opencode-callback"
            }
        }
    });

    opencode.install_into("plugins", &mut json).unwrap();

    // Should still have only one viberails entry
    assert!(json["plugins"]["viberails"].is_object());
}

#[test]
fn test_install_into_updates_different_command() {
    let opencode = make_opencode("/usr/bin/test-program");
    let mut json = json!({
        "plugins": {
            "viberails": {
                "enabled": false,
                "command": "/old/path opencode-callback"
            }
        }
    });

    opencode.install_into("plugins", &mut json).unwrap();

    // Should be updated with new command
    assert_eq!(
        json["plugins"]["viberails"]["command"],
        "/usr/bin/test-program opencode-callback"
    );
    assert!(json["plugins"]["viberails"]["enabled"].as_bool().unwrap());
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
fn test_uninstall_from_removes_viberails() {
    let opencode = make_opencode("/usr/bin/test-program");
    let mut json = json!({
        "plugins": {
            "viberails": {
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

    assert!(json["plugins"].get("viberails").is_none());
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
fn test_uninstall_from_no_viberails() {
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
    assert!(json["plugins"]["other-plugin"]["enabled"].as_bool().unwrap());
}

// Discovery tests
#[test]
fn test_opencode_discovery_id() {
    use super::opencode::OpenCodeDiscovery;
    use crate::providers::ProviderDiscovery;

    let discovery = OpenCodeDiscovery;
    assert_eq!(discovery.id(), "opencode");
}

#[test]
fn test_opencode_discovery_display_name() {
    use super::opencode::OpenCodeDiscovery;
    use crate::providers::ProviderDiscovery;

    let discovery = OpenCodeDiscovery;
    assert_eq!(discovery.display_name(), "OpenCode");
}

#[test]
fn test_opencode_discovery_supported_hooks() {
    use super::opencode::OpenCodeDiscovery;
    use crate::providers::ProviderDiscovery;

    let discovery = OpenCodeDiscovery;
    let hooks = discovery.supported_hooks();
    assert!(hooks.contains(&"plugins"));
}
