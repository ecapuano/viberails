#![allow(clippy::unwrap_used)]

use serde_json::json;

use crate::common::PROJECT_NAME;
use crate::providers::clawdbot::Clawdbot;

fn make_clawdbot(program: &str) -> Clawdbot {
    Clawdbot::with_custom_path(program)
}

#[test]
fn test_install_into_empty_json() {
    let clawdbot = make_clawdbot("/usr/bin/test-program");
    let mut json = json!({});

    clawdbot.install_into("hooks", &mut json).unwrap();

    let hooks = &json["hooks"];
    assert!(hooks.is_object());
    let internal = &hooks["internal"];
    assert!(internal["enabled"].as_bool().unwrap());
    let entries = &internal["entries"];
    assert!(entries.is_object());
    let entry = &entries[PROJECT_NAME];
    assert!(entry["enabled"].as_bool().unwrap());
    // Directory-based hooks don't have a "command" key
    assert!(entry.get("command").is_none());
}

#[test]
fn test_install_into_existing_hooks() {
    let clawdbot = make_clawdbot("/usr/bin/test-program");
    let mut json = json!({
        "hooks": {
            "internal": {
                "enabled": true,
                "entries": {
                    "other-hook": {
                        "enabled": true
                    }
                }
            }
        }
    });

    clawdbot.install_into("hooks", &mut json).unwrap();

    // Other hook should be preserved
    assert!(
        json["hooks"]["internal"]["entries"]["other-hook"]["enabled"]
            .as_bool()
            .unwrap()
    );
    // Our hook should be added
    assert!(
        json["hooks"]["internal"]["entries"][PROJECT_NAME]["enabled"]
            .as_bool()
            .unwrap()
    );
}

#[test]
fn test_install_into_skips_if_already_installed() {
    let clawdbot = make_clawdbot("/usr/bin/test-program");
    let mut json = json!({
        "hooks": {
            "internal": {
                "enabled": true,
                "entries": {
                    PROJECT_NAME: {
                        "enabled": true
                    }
                }
            }
        }
    });

    clawdbot.install_into("hooks", &mut json).unwrap();

    // Should still have the same entry
    assert!(json["hooks"]["internal"]["entries"][PROJECT_NAME].is_object());
}

#[test]
fn test_install_into_preserves_existing_entry() {
    let clawdbot = make_clawdbot("/usr/bin/test-program");
    let mut json = json!({
        "hooks": {
            "internal": {
                "enabled": false,
                "entries": {
                    PROJECT_NAME: {
                        "enabled": false
                    }
                }
            }
        }
    });

    clawdbot.install_into("hooks", &mut json).unwrap();

    // Entry should exist (won't overwrite existing)
    assert!(json["hooks"]["internal"]["entries"][PROJECT_NAME].is_object());
    // internal.enabled should be set to true
    assert!(json["hooks"]["internal"]["enabled"].as_bool().unwrap());
}

#[test]
fn test_install_into_preserves_other_config() {
    let clawdbot = make_clawdbot("/usr/bin/test-program");
    let mut json = json!({
        "agent": {
            "model": "claude-3"
        }
    });

    clawdbot.install_into("hooks", &mut json).unwrap();

    assert_eq!(json["agent"]["model"], "claude-3");
}

#[test]
fn test_uninstall_from_removes_entry() {
    let clawdbot = make_clawdbot("/usr/bin/test-program");
    let mut json = json!({
        "hooks": {
            "internal": {
                "enabled": true,
                "entries": {
                    PROJECT_NAME: {
                        "enabled": true
                    },
                    "other-hook": {
                        "enabled": true
                    }
                }
            }
        }
    });

    clawdbot.uninstall_from("hooks", &mut json);

    assert!(
        json["hooks"]["internal"]["entries"]
            .get(PROJECT_NAME)
            .is_none()
    );
    // Other hook should be preserved
    assert!(json["hooks"]["internal"]["entries"]["other-hook"].is_object());
}

#[test]
fn test_uninstall_from_no_hooks() {
    let clawdbot = make_clawdbot("/usr/bin/test-program");
    let mut json = json!({});

    // Should not panic
    clawdbot.uninstall_from("hooks", &mut json);
}

#[test]
fn test_uninstall_from_no_entries() {
    let clawdbot = make_clawdbot("/usr/bin/test-program");
    let mut json = json!({
        "hooks": {
            "internal": {
                "enabled": true
            }
        }
    });

    // Should not panic
    clawdbot.uninstall_from("hooks", &mut json);
}

#[test]
fn test_uninstall_from_no_entry() {
    let clawdbot = make_clawdbot("/usr/bin/test-program");
    let mut json = json!({
        "hooks": {
            "internal": {
                "enabled": true,
                "entries": {
                    "other-hook": {
                        "enabled": true
                    }
                }
            }
        }
    });

    // Should not panic
    clawdbot.uninstall_from("hooks", &mut json);

    // Other hook should be unchanged
    assert!(
        json["hooks"]["internal"]["entries"]["other-hook"]["enabled"]
            .as_bool()
            .unwrap()
    );
}

// Error handling tests
#[test]
fn test_install_into_fails_on_non_object_root() {
    let clawdbot = make_clawdbot("/usr/bin/test-program");
    let mut json = json!([]);

    let result = clawdbot.install_into("hooks", &mut json);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("JSON object"));
}

#[test]
fn test_install_into_fails_on_hooks_not_object() {
    let clawdbot = make_clawdbot("/usr/bin/test-program");
    let mut json = json!({
        "hooks": "not an object"
    });

    let result = clawdbot.install_into("hooks", &mut json);
    assert!(result.is_err());
}

// Generated content tests
#[test]
fn test_generate_hook_md_contains_required_fields() {
    use crate::providers::clawdbot::Clawdbot;

    let hook_md = Clawdbot::generate_hook_md();

    // Check YAML frontmatter markers
    assert!(hook_md.starts_with("---\n"));
    assert!(hook_md.contains("\n---\n"));

    // Check required fields
    assert!(hook_md.contains("name:"));
    assert!(hook_md.contains("description:"));
    assert!(hook_md.contains("events:"));
    assert!(hook_md.contains(crate::common::PROJECT_NAME));
}

#[test]
fn test_generate_handler_ts_contains_binary_path() {
    let clawdbot = make_clawdbot("/custom/path/to/viberails");

    let handler_ts = clawdbot.generate_handler_ts();

    // Check it contains the binary path
    assert!(handler_ts.contains("/custom/path/to/viberails"));
    // Check it has the callback command
    assert!(handler_ts.contains("clawdbot-callback"));
    // Check it exports a default handler
    assert!(handler_ts.contains("export default handler"));
    // Check it imports HookHandler type
    assert!(handler_ts.contains("HookHandler"));
}

// Discovery tests
#[test]
fn test_clawdbot_discovery_id() {
    use crate::providers::ProviderDiscovery;
    use crate::providers::clawdbot::ClawdbotDiscovery;

    let discovery = ClawdbotDiscovery;
    assert_eq!(discovery.id(), "clawdbot");
}

#[test]
fn test_clawdbot_discovery_display_name() {
    use crate::providers::ProviderDiscovery;
    use crate::providers::clawdbot::ClawdbotDiscovery;

    let discovery = ClawdbotDiscovery;
    assert_eq!(discovery.display_name(), "Clawdbot/OpenClaw");
}

#[test]
fn test_clawdbot_discovery_supported_hooks() {
    use crate::providers::ProviderDiscovery;
    use crate::providers::clawdbot::ClawdbotDiscovery;

    let discovery = ClawdbotDiscovery;
    let hooks = discovery.supported_hooks();
    assert!(hooks.contains(&"hooks"));
}
