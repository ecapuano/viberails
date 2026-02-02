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

    clawdbot.install_into("plugin", &mut json).unwrap();

    let plugins = &json["plugins"];
    assert!(plugins.is_object());
    let entries = &plugins["entries"];
    assert!(entries.is_object());
    let entry = &entries[PROJECT_NAME];
    assert!(entry["enabled"].as_bool().unwrap());
}

#[test]
fn test_install_into_existing_plugins() {
    let clawdbot = make_clawdbot("/usr/bin/test-program");
    let mut json = json!({
        "plugins": {
            "entries": {
                "other-plugin": {
                    "enabled": true
                }
            }
        }
    });

    clawdbot.install_into("plugin", &mut json).unwrap();

    // Other plugin should be preserved
    assert!(
        json["plugins"]["entries"]["other-plugin"]["enabled"]
            .as_bool()
            .unwrap()
    );
    // Our plugin should be added
    assert!(
        json["plugins"]["entries"][PROJECT_NAME]["enabled"]
            .as_bool()
            .unwrap()
    );
}

#[test]
fn test_install_into_skips_if_already_installed() {
    let clawdbot = make_clawdbot("/usr/bin/test-program");
    let mut json = json!({
        "plugins": {
            "entries": {
                PROJECT_NAME: {
                    "enabled": true
                }
            }
        }
    });

    clawdbot.install_into("plugin", &mut json).unwrap();

    // Should still have the same entry
    assert!(json["plugins"]["entries"][PROJECT_NAME].is_object());
}

#[test]
fn test_install_into_preserves_other_config() {
    let clawdbot = make_clawdbot("/usr/bin/test-program");
    let mut json = json!({
        "agent": {
            "model": "claude-3"
        }
    });

    clawdbot.install_into("plugin", &mut json).unwrap();

    assert_eq!(json["agent"]["model"], "claude-3");
}

#[test]
fn test_uninstall_from_removes_plugin_entry() {
    let clawdbot = make_clawdbot("/usr/bin/test-program");
    let mut json = json!({
        "plugins": {
            "entries": {
                PROJECT_NAME: {
                    "enabled": true
                },
                "other-plugin": {
                    "enabled": true
                }
            }
        }
    });

    clawdbot.uninstall_from("plugin", &mut json);

    assert!(
        json["plugins"]["entries"]
            .get(PROJECT_NAME)
            .is_none()
    );
    // Other plugin should be preserved
    assert!(json["plugins"]["entries"]["other-plugin"].is_object());
}

#[test]
fn test_uninstall_from_empty_json() {
    let clawdbot = make_clawdbot("/usr/bin/test-program");
    let mut json = json!({});

    // Should not panic
    clawdbot.uninstall_from("plugin", &mut json);
}

#[test]
fn test_uninstall_from_no_entries() {
    let clawdbot = make_clawdbot("/usr/bin/test-program");
    let mut json = json!({
        "plugins": {}
    });

    // Should not panic
    clawdbot.uninstall_from("plugin", &mut json);
}

#[test]
fn test_uninstall_from_no_entry() {
    let clawdbot = make_clawdbot("/usr/bin/test-program");
    let mut json = json!({
        "plugins": {
            "entries": {
                "other-plugin": {
                    "enabled": true
                }
            }
        }
    });

    // Should not panic
    clawdbot.uninstall_from("plugin", &mut json);

    // Other plugin should be unchanged
    assert!(
        json["plugins"]["entries"]["other-plugin"]["enabled"]
            .as_bool()
            .unwrap()
    );
}

// Error handling tests
#[test]
fn test_install_into_fails_on_non_object_root() {
    let clawdbot = make_clawdbot("/usr/bin/test-program");
    let mut json = json!([]);

    let result = clawdbot.install_into("plugin", &mut json);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("JSON object"));
}

#[test]
fn test_install_into_fails_on_plugins_not_object() {
    let clawdbot = make_clawdbot("/usr/bin/test-program");
    let mut json = json!({
        "plugins": "not an object"
    });

    let result = clawdbot.install_into("plugin", &mut json);
    assert!(result.is_err());
}

// Generated content tests
#[test]
fn test_generate_plugin_manifest_contains_required_fields() {
    let manifest = Clawdbot::generate_plugin_manifest();

    // Parse as JSON
    let json: serde_json::Value = serde_json::from_str(&manifest).unwrap();

    // Check required fields
    assert!(json.get("id").is_some());
    assert!(json.get("name").is_some());
    assert!(json.get("version").is_some());
    assert!(json.get("description").is_some());
    assert!(json.get("main").is_some());

    // Check PROJECT_NAME is used
    assert_eq!(json["id"].as_str().unwrap(), PROJECT_NAME);
    assert_eq!(json["main"].as_str().unwrap(), "index.ts");
}

#[test]
fn test_generate_plugin_index_contains_binary_path() {
    let clawdbot = make_clawdbot("/custom/path/to/viberails");

    let index_ts = clawdbot.generate_plugin_index();

    // Check it contains the binary path
    assert!(index_ts.contains("/custom/path/to/viberails"));
    // Check it has the callback command
    assert!(index_ts.contains("clawdbot-callback"));
    // Check it exports a default register function
    assert!(index_ts.contains("export default function register"));
    // Check it registers before_tool_call hook
    assert!(index_ts.contains("before_tool_call"));
    // Check it handles both OpenClaw and Clawdbot API
    assert!(index_ts.contains("registerHook"));
    assert!(index_ts.contains("addHook"));
}

#[test]
fn test_generate_plugin_index_uses_spawn_sync() {
    let clawdbot = make_clawdbot("/usr/bin/viberails");

    let index_ts = clawdbot.generate_plugin_index();

    // Should use spawnSync for synchronous hook execution
    assert!(index_ts.contains("spawnSync"));
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
    assert!(hooks.contains(&"plugin"));
}

// is_tool_use detection tests (using LLmProviderTrait)
#[test]
fn test_is_tool_use_detects_openclaw_format() {
    use crate::providers::LLmProviderTrait;

    let clawdbot = make_clawdbot("/usr/bin/test");

    // OpenClaw/Clawdbot before_tool_call format uses "tool" key
    let event = json!({
        "tool": "exec",
        "parameters": {
            "command": "ls -la"
        }
    });

    assert!(clawdbot.is_tool_use(&event));
}

#[test]
fn test_is_tool_use_detects_claude_code_format() {
    use crate::providers::LLmProviderTrait;

    let clawdbot = make_clawdbot("/usr/bin/test");

    // Claude Code format uses tool_name/tool_input
    let event = json!({
        "tool_name": "bash",
        "tool_input": {
            "command": "ls -la"
        },
        "tool_use_id": "toolu_123"
    });

    assert!(clawdbot.is_tool_use(&event));
}

#[test]
fn test_is_tool_use_rejects_non_tool_event() {
    use crate::providers::LLmProviderTrait;

    let clawdbot = make_clawdbot("/usr/bin/test");

    // Regular message without tool keys
    let event = json!({
        "type": "message",
        "content": "Hello world"
    });

    assert!(!clawdbot.is_tool_use(&event));
}

#[test]
fn test_is_tool_use_rejects_parameters_only() {
    use crate::providers::LLmProviderTrait;

    let clawdbot = make_clawdbot("/usr/bin/test");

    // Event with just "parameters" shouldn't be detected as tool use
    // (too generic, could be any kind of event)
    let event = json!({
        "type": "config",
        "parameters": {
            "setting": "value"
        }
    });

    assert!(!clawdbot.is_tool_use(&event));
}
