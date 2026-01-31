#![allow(clippy::unwrap_used)]

use serde_json::json;

use crate::providers::cursor::Cursor;

fn make_cursor(program: &str) -> Cursor {
    Cursor::with_custom_path(program).unwrap()
}

#[test]
fn test_install_into_empty_json() {
    let cursor = make_cursor("/usr/bin/test-program");
    let mut json = json!({"version": 1, "hooks": {}});

    cursor.install_into("preToolUse", &mut json).unwrap();

    let hooks = &json["hooks"]["preToolUse"];
    assert!(hooks.is_array());
    let hooks_arr = hooks.as_array().unwrap();
    assert_eq!(hooks_arr.len(), 1);
    assert_eq!(hooks_arr[0]["type"], "command");
    assert_eq!(
        hooks_arr[0]["command"],
        "/usr/bin/test-program cursor-callback"
    );
    assert_eq!(hooks_arr[0]["matcher"], "*");
}

#[test]
fn test_install_into_creates_hooks_object() {
    let cursor = make_cursor("/usr/bin/test-program");
    let mut json = json!({"version": 1});

    cursor.install_into("preToolUse", &mut json).unwrap();

    assert!(json.get("hooks").is_some());
    assert!(json["hooks"]["preToolUse"].is_array());
}

#[test]
fn test_install_into_creates_version() {
    let cursor = make_cursor("/usr/bin/test-program");
    let mut json = json!({});

    cursor.install_into("preToolUse", &mut json).unwrap();

    assert_eq!(json["version"], 1);
}

#[test]
fn test_install_into_preserves_existing_hooks() {
    let cursor = make_cursor("/usr/bin/test-program");
    let mut json = json!({
        "version": 1,
        "hooks": {
            "preToolUse": [
                {
                    "type": "command",
                    "command": "/other/program",
                    "matcher": "Bash"
                }
            ]
        }
    });

    cursor.install_into("preToolUse", &mut json).unwrap();

    let hooks = json["hooks"]["preToolUse"].as_array().unwrap();
    assert_eq!(hooks.len(), 2);
    // Our hook should be first
    assert_eq!(hooks[0]["command"], "/usr/bin/test-program cursor-callback");
    assert_eq!(hooks[1]["command"], "/other/program");
}

#[test]
fn test_install_into_skips_if_already_installed() {
    let cursor = make_cursor("/usr/bin/test-program");
    let mut json = json!({
        "version": 1,
        "hooks": {
            "preToolUse": [
                {
                    "type": "command",
                    "command": "/usr/bin/test-program cursor-callback",
                    "matcher": "*"
                }
            ]
        }
    });

    cursor.install_into("preToolUse", &mut json).unwrap();

    let hooks = json["hooks"]["preToolUse"].as_array().unwrap();
    assert_eq!(hooks.len(), 1);
}

#[test]
fn test_install_into_different_hook_types() {
    let cursor = make_cursor("/usr/bin/test-program");
    let mut json = json!({"version": 1, "hooks": {}});

    cursor.install_into("preToolUse", &mut json).unwrap();
    cursor
        .install_into("beforeSubmitPrompt", &mut json)
        .unwrap();

    assert!(json["hooks"]["preToolUse"].is_array());
    assert!(json["hooks"]["beforeSubmitPrompt"].is_array());
}

#[test]
fn test_uninstall_from_removes_our_hook() {
    let cursor = make_cursor("/usr/bin/test-program");
    let mut json = json!({
        "version": 1,
        "hooks": {
            "preToolUse": [
                {
                    "type": "command",
                    "command": "/usr/bin/test-program cursor-callback",
                    "matcher": "*"
                }
            ]
        }
    });

    cursor.uninstall_from("preToolUse", &mut json);

    let hooks = json["hooks"]["preToolUse"].as_array().unwrap();
    assert_eq!(hooks.len(), 0);
}

#[test]
fn test_uninstall_from_preserves_other_hooks() {
    let cursor = make_cursor("/usr/bin/test-program");
    let mut json = json!({
        "version": 1,
        "hooks": {
            "preToolUse": [
                {
                    "type": "command",
                    "command": "/other/program",
                    "matcher": "Bash"
                },
                {
                    "type": "command",
                    "command": "/usr/bin/test-program cursor-callback",
                    "matcher": "*"
                }
            ]
        }
    });

    cursor.uninstall_from("preToolUse", &mut json);

    let hooks = json["hooks"]["preToolUse"].as_array().unwrap();
    assert_eq!(hooks.len(), 1);
    assert_eq!(hooks[0]["command"], "/other/program");
}

#[test]
fn test_uninstall_from_no_hooks_object() {
    let cursor = make_cursor("/usr/bin/test-program");
    let mut json = json!({"version": 1});

    // Should not panic
    cursor.uninstall_from("preToolUse", &mut json);
}

#[test]
fn test_uninstall_from_no_hook_type() {
    let cursor = make_cursor("/usr/bin/test-program");
    let mut json = json!({"version": 1, "hooks": {}});

    // Should not panic
    cursor.uninstall_from("preToolUse", &mut json);
}

// Discovery tests
#[test]
fn test_cursor_discovery_id() {
    use crate::providers::ProviderDiscovery;
    use crate::providers::cursor::CursorDiscovery;

    let discovery = CursorDiscovery;
    assert_eq!(discovery.id(), "cursor");
}

#[test]
fn test_cursor_discovery_display_name() {
    use crate::providers::ProviderDiscovery;
    use crate::providers::cursor::CursorDiscovery;

    let discovery = CursorDiscovery;
    assert_eq!(discovery.display_name(), "Cursor");
}

#[test]
fn test_cursor_discovery_supported_hooks() {
    use crate::providers::ProviderDiscovery;
    use crate::providers::cursor::CursorDiscovery;

    let discovery = CursorDiscovery;
    let hooks = discovery.supported_hooks();
    assert!(hooks.contains(&"preToolUse"));
    assert!(hooks.contains(&"beforeSubmitPrompt"));
}
