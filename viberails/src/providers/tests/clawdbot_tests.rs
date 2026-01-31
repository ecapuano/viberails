#![allow(clippy::unwrap_used)]

use serde_json::json;

use crate::providers::clawdbot::Clawdbot;

fn make_clawdbot(program: &str) -> Clawdbot {
    Clawdbot::new(program).unwrap()
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
    let viberails = &entries["viberails"];
    assert!(viberails["enabled"].as_bool().unwrap());
    assert_eq!(
        viberails["command"],
        "/usr/bin/test-program clawdbot-callback"
    );
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
        json["hooks"]["internal"]["entries"]["viberails"]["enabled"]
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
                    "viberails": {
                        "enabled": true,
                        "command": "/usr/bin/test-program clawdbot-callback"
                    }
                }
            }
        }
    });

    clawdbot.install_into("hooks", &mut json).unwrap();

    // Should still have the same entry
    assert!(json["hooks"]["internal"]["entries"]["viberails"].is_object());
}

#[test]
fn test_install_into_updates_different_command() {
    let clawdbot = make_clawdbot("/usr/bin/test-program");
    let mut json = json!({
        "hooks": {
            "internal": {
                "enabled": false,
                "entries": {
                    "viberails": {
                        "enabled": false,
                        "command": "/old/path clawdbot-callback"
                    }
                }
            }
        }
    });

    clawdbot.install_into("hooks", &mut json).unwrap();

    // Should be updated with new command
    assert_eq!(
        json["hooks"]["internal"]["entries"]["viberails"]["command"],
        "/usr/bin/test-program clawdbot-callback"
    );
    assert!(
        json["hooks"]["internal"]["entries"]["viberails"]["enabled"]
            .as_bool()
            .unwrap()
    );
    // internal.enabled should also be set to true
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
fn test_uninstall_from_removes_viberails() {
    let clawdbot = make_clawdbot("/usr/bin/test-program");
    let mut json = json!({
        "hooks": {
            "internal": {
                "enabled": true,
                "entries": {
                    "viberails": {
                        "enabled": true,
                        "command": "/usr/bin/test-program clawdbot-callback"
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
            .get("viberails")
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
fn test_uninstall_from_no_viberails() {
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
