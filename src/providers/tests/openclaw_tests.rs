#![allow(clippy::unwrap_used)]

use serde_json::json;

use crate::common::PROJECT_NAME;
use crate::providers::openclaw::OpenClaw;

fn make_openclaw(program: &str) -> OpenClaw {
    OpenClaw::with_custom_path(program)
}

#[test]
fn test_install_into_empty_json() {
    let openclaw = make_openclaw("/usr/bin/test-program");
    let mut json = json!({});

    openclaw.install_into("plugin", &mut json).unwrap();

    let plugins = &json["plugins"];
    assert!(plugins.is_object());
    let entries = &plugins["entries"];
    assert!(entries.is_object());
    let entry = &entries[PROJECT_NAME];
    assert!(entry["enabled"].as_bool().unwrap());
}

#[test]
fn test_install_into_existing_plugins() {
    let openclaw = make_openclaw("/usr/bin/test-program");
    let mut json = json!({
        "plugins": {
            "entries": {
                "other-plugin": {
                    "enabled": true
                }
            }
        }
    });

    openclaw.install_into("plugin", &mut json).unwrap();

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
    let openclaw = make_openclaw("/usr/bin/test-program");
    let mut json = json!({
        "plugins": {
            "entries": {
                PROJECT_NAME: {
                    "enabled": true
                }
            }
        }
    });

    openclaw.install_into("plugin", &mut json).unwrap();

    // Should still have the same entry
    assert!(json["plugins"]["entries"][PROJECT_NAME].is_object());
}

#[test]
fn test_install_into_preserves_other_config() {
    let openclaw = make_openclaw("/usr/bin/test-program");
    let mut json = json!({
        "agent": {
            "model": "claude-3"
        }
    });

    openclaw.install_into("plugin", &mut json).unwrap();

    assert_eq!(json["agent"]["model"], "claude-3");
}

#[test]
fn test_uninstall_from_removes_plugin_entry() {
    let openclaw = make_openclaw("/usr/bin/test-program");
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

    openclaw.uninstall_from("plugin", &mut json);

    assert!(json["plugins"]["entries"].get(PROJECT_NAME).is_none());
    // Other plugin should be preserved
    assert!(json["plugins"]["entries"]["other-plugin"].is_object());
}

#[test]
fn test_uninstall_from_empty_json() {
    let openclaw = make_openclaw("/usr/bin/test-program");
    let mut json = json!({});

    // Should not panic
    openclaw.uninstall_from("plugin", &mut json);
}

#[test]
fn test_uninstall_from_no_entries() {
    let openclaw = make_openclaw("/usr/bin/test-program");
    let mut json = json!({
        "plugins": {}
    });

    // Should not panic
    openclaw.uninstall_from("plugin", &mut json);
}

#[test]
fn test_uninstall_from_no_entry() {
    let openclaw = make_openclaw("/usr/bin/test-program");
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
    openclaw.uninstall_from("plugin", &mut json);

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
    let openclaw = make_openclaw("/usr/bin/test-program");
    let mut json = json!([]);

    let result = openclaw.install_into("plugin", &mut json);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("JSON object"));
}

#[test]
fn test_install_into_fails_on_plugins_not_object() {
    let openclaw = make_openclaw("/usr/bin/test-program");
    let mut json = json!({
        "plugins": "not an object"
    });

    let result = openclaw.install_into("plugin", &mut json);
    assert!(result.is_err());
}

#[test]
fn test_install_into_fails_on_entries_not_object() {
    let openclaw = make_openclaw("/usr/bin/test-program");
    let mut json = json!({
        "plugins": {
            "entries": "not an object"
        }
    });

    let result = openclaw.install_into("plugin", &mut json);
    assert!(result.is_err());
}

// Generated content tests
#[test]
fn test_generate_plugin_manifest_contains_required_fields() {
    let manifest = OpenClaw::generate_plugin_manifest();

    // Parse as JSON
    let json: serde_json::Value = serde_json::from_str(&manifest).unwrap();

    // Check required fields per OpenClaw plugin spec
    assert!(json.get("id").is_some());
    assert!(json.get("name").is_some());
    assert!(json.get("version").is_some());
    assert!(json.get("description").is_some());
    assert!(json.get("main").is_some());
    assert!(json.get("configSchema").is_some(), "configSchema is required by OpenClaw");

    // Check PROJECT_NAME is used
    assert_eq!(json["id"].as_str().unwrap(), PROJECT_NAME);
    assert_eq!(json["main"].as_str().unwrap(), "index.ts");

    // Verify configSchema structure
    let config_schema = &json["configSchema"];
    assert_eq!(config_schema["type"].as_str().unwrap(), "object");
}

#[test]
fn test_generate_plugin_index_contains_binary_path() {
    let openclaw = make_openclaw("/custom/path/to/viberails");

    let index_ts = openclaw.generate_plugin_index();

    // Check it contains the binary path
    assert!(index_ts.contains("/custom/path/to/viberails"));
    // Check it has the callback command (now openclaw-callback, not clawdbot-callback)
    assert!(index_ts.contains("openclaw-callback"));
    // Check it exports a default register function
    assert!(index_ts.contains("export default function register"));
    // Check it registers before_tool_call hook
    assert!(index_ts.contains("before_tool_call"));
    // Check it uses registerHook
    assert!(index_ts.contains("registerHook"));
}

#[test]
fn test_generate_plugin_index_uses_spawn_sync() {
    let openclaw = make_openclaw("/usr/bin/viberails");

    let index_ts = openclaw.generate_plugin_index();

    // Should use spawnSync for synchronous hook execution
    assert!(index_ts.contains("spawnSync"));
}

#[test]
fn test_generate_plugin_index_uses_correct_event_format() {
    let openclaw = make_openclaw("/usr/bin/viberails");

    let index_ts = openclaw.generate_plugin_index();

    // Should use the correct OpenClaw event format (PR #6264)
    assert!(index_ts.contains("toolName"));
    assert!(index_ts.contains("BeforeToolCallEvent"));
    // Should use the correct response format
    assert!(index_ts.contains("block: true"));
    assert!(index_ts.contains("blockReason"));
}

// Discovery tests
#[test]
fn test_openclaw_discovery_id() {
    use crate::providers::ProviderDiscovery;
    use crate::providers::openclaw::OpenClawDiscovery;

    let discovery = OpenClawDiscovery;
    assert_eq!(discovery.id(), "openclaw");
}

#[test]
fn test_openclaw_discovery_display_name() {
    use crate::providers::ProviderDiscovery;
    use crate::providers::openclaw::OpenClawDiscovery;

    let discovery = OpenClawDiscovery;
    assert_eq!(discovery.display_name(), "OpenClaw");
}

#[test]
fn test_openclaw_discovery_supported_hooks() {
    use crate::providers::ProviderDiscovery;
    use crate::providers::openclaw::OpenClawDiscovery;

    let discovery = OpenClawDiscovery;
    let hooks = discovery.supported_hooks();
    assert!(hooks.contains(&"plugin"));
}

// is_tool_use detection tests (using LLmProviderTrait)
#[test]
fn test_is_tool_use_detects_openclaw_format() {
    use crate::providers::LLmProviderTrait;

    let openclaw = make_openclaw("/usr/bin/test");

    // OpenClaw before_tool_call format uses "toolName" key (PR #6264)
    let event = json!({
        "toolName": "exec",
        "params": {
            "command": "ls -la"
        }
    });

    assert!(openclaw.is_tool_use(&event));
}

#[test]
fn test_is_tool_use_detects_claude_code_format() {
    use crate::providers::LLmProviderTrait;

    let openclaw = make_openclaw("/usr/bin/test");

    // Claude Code format uses tool_name/tool_input
    let event = json!({
        "tool_name": "bash",
        "tool_input": {
            "command": "ls -la"
        },
        "tool_use_id": "toolu_123"
    });

    assert!(openclaw.is_tool_use(&event));
}

#[test]
fn test_is_tool_use_rejects_non_tool_event() {
    use crate::providers::LLmProviderTrait;

    let openclaw = make_openclaw("/usr/bin/test");

    // Regular message without tool keys
    let event = json!({
        "type": "message",
        "content": "Hello world"
    });

    assert!(!openclaw.is_tool_use(&event));
}

#[test]
fn test_is_tool_use_rejects_params_only() {
    use crate::providers::LLmProviderTrait;

    let openclaw = make_openclaw("/usr/bin/test");

    // Event with just "params" shouldn't be detected as tool use
    // (too generic, could be any kind of event)
    let event = json!({
        "type": "config",
        "params": {
            "setting": "value"
        }
    });

    assert!(!openclaw.is_tool_use(&event));
}

// JavaScript string escaping tests
#[test]
fn test_escape_js_string_basic_path() {
    let openclaw = make_openclaw("/usr/bin/viberails");
    let index_ts = openclaw.generate_plugin_index();

    // Basic path should be unchanged
    assert!(index_ts.contains("/usr/bin/viberails"));
}

#[test]
fn test_escape_js_string_path_with_spaces() {
    let openclaw = make_openclaw("/home/user name/bin/viberails");
    let index_ts = openclaw.generate_plugin_index();

    // Path with spaces should work (spaces don't need escaping in JS strings)
    assert!(index_ts.contains("/home/user name/bin/viberails"));
}

#[test]
fn test_escape_js_string_path_with_quotes() {
    let openclaw = make_openclaw("/home/user\"test/bin/viberails");
    let index_ts = openclaw.generate_plugin_index();

    // Quotes should be escaped
    assert!(index_ts.contains("/home/user\\\"test/bin/viberails"));
}

#[test]
fn test_escape_js_string_path_with_backslash() {
    let openclaw = make_openclaw("C:\\Users\\test\\viberails");
    let index_ts = openclaw.generate_plugin_index();

    // Backslashes should be escaped
    assert!(index_ts.contains("C:\\\\Users\\\\test\\\\viberails"));
}

#[test]
fn test_escape_js_string_path_with_dollar() {
    let openclaw = make_openclaw("/home/$USER/bin/viberails");
    let index_ts = openclaw.generate_plugin_index();

    // Dollar signs should be escaped to prevent template injection
    assert!(index_ts.contains("/home/\\$USER/bin/viberails"));
}

#[test]
fn test_escape_js_string_path_with_backtick() {
    let openclaw = make_openclaw("/home/user`test/bin/viberails");
    let index_ts = openclaw.generate_plugin_index();

    // Backticks should be escaped
    assert!(index_ts.contains("/home/user\\`test/bin/viberails"));
}
