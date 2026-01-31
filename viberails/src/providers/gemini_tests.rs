#![allow(clippy::unwrap_used)]

use serde_json::json;

use super::gemini::Gemini;

fn make_gemini(program: &str) -> Gemini {
    Gemini::new(program).unwrap()
}

#[test]
fn test_install_into_empty_json() {
    let gemini = make_gemini("/usr/bin/test-program");
    let mut json = json!({});

    gemini.install_into("BeforeTool", &mut json).unwrap();

    let hooks = &json["hooks"]["BeforeTool"];
    assert!(hooks.is_array());
    let hooks_arr = hooks.as_array().unwrap();
    assert_eq!(hooks_arr.len(), 1);
    assert_eq!(hooks_arr[0]["matcher"], "*");
    assert_eq!(
        hooks_arr[0]["hooks"][0]["command"],
        "/usr/bin/test-program gemini-callback"
    );
    assert_eq!(hooks_arr[0]["hooks"][0]["type"], "command");
    assert_eq!(hooks_arr[0]["hooks"][0]["name"], "viberails");
}

#[test]
fn test_install_into_existing_hooks_object() {
    let gemini = make_gemini("/usr/bin/test-program");
    let mut json = json!({
        "hooks": {}
    });

    gemini.install_into("BeforeTool", &mut json).unwrap();

    let hooks = &json["hooks"]["BeforeTool"];
    assert!(hooks.is_array());
    assert_eq!(hooks.as_array().unwrap().len(), 1);
}

#[test]
fn test_install_into_prepends_to_existing_wildcard_matcher() {
    let gemini = make_gemini("/usr/bin/test-program");
    let mut json = json!({
        "hooks": {
            "BeforeTool": [
                {
                    "matcher": "*",
                    "hooks": [
                        {"type": "command", "command": "/existing/program"}
                    ]
                }
            ]
        }
    });

    gemini.install_into("BeforeTool", &mut json).unwrap();

    let hooks = &json["hooks"]["BeforeTool"];
    let hooks_arr = hooks.as_array().unwrap();
    assert_eq!(hooks_arr.len(), 1);
    let inner_hooks = hooks_arr[0]["hooks"].as_array().unwrap();
    assert_eq!(inner_hooks.len(), 2);
    assert_eq!(
        inner_hooks[0]["command"],
        "/usr/bin/test-program gemini-callback"
    );
    assert_eq!(inner_hooks[1]["command"], "/existing/program");
}

#[test]
fn test_install_into_creates_wildcard_matcher() {
    let gemini = make_gemini("/usr/bin/test-program");
    let mut json = json!({
        "hooks": {
            "BeforeTool": [
                {
                    "matcher": "Bash",
                    "hooks": [
                        {"type": "command", "command": "/other/program"}
                    ]
                }
            ]
        }
    });

    gemini.install_into("BeforeTool", &mut json).unwrap();

    let hooks = &json["hooks"]["BeforeTool"];
    let hooks_arr = hooks.as_array().unwrap();
    assert_eq!(hooks_arr.len(), 2);
    // New wildcard matcher should be first
    assert_eq!(hooks_arr[0]["matcher"], "*");
    assert_eq!(hooks_arr[1]["matcher"], "Bash");
}

#[test]
fn test_install_into_skips_if_already_installed() {
    let gemini = make_gemini("/usr/bin/test-program");
    let mut json = json!({
        "hooks": {
            "BeforeTool": [
                {
                    "matcher": "*",
                    "hooks": [
                        {"type": "command", "command": "/usr/bin/test-program gemini-callback"}
                    ]
                }
            ]
        }
    });

    gemini.install_into("BeforeTool", &mut json).unwrap();

    let inner_hooks = json["hooks"]["BeforeTool"][0]["hooks"].as_array().unwrap();
    assert_eq!(inner_hooks.len(), 1);
}

#[test]
fn test_install_into_different_hook_types() {
    let gemini = make_gemini("/usr/bin/test-program");
    let mut json = json!({});

    gemini.install_into("BeforeTool", &mut json).unwrap();
    gemini.install_into("SessionStart", &mut json).unwrap();

    assert!(json["hooks"]["BeforeTool"].is_array());
    assert!(json["hooks"]["SessionStart"].is_array());
}

#[test]
fn test_uninstall_from_removes_our_hook() {
    let gemini = make_gemini("/usr/bin/test-program");
    let mut json = json!({
        "hooks": {
            "BeforeTool": [
                {
                    "matcher": "*",
                    "hooks": [
                        {"type": "command", "command": "/usr/bin/test-program gemini-callback"}
                    ]
                }
            ]
        }
    });

    gemini.uninstall_from("BeforeTool", &mut json);

    let inner_hooks = json["hooks"]["BeforeTool"][0]["hooks"].as_array().unwrap();
    assert_eq!(inner_hooks.len(), 0);
}

#[test]
fn test_uninstall_from_preserves_other_hooks() {
    let gemini = make_gemini("/usr/bin/test-program");
    let mut json = json!({
        "hooks": {
            "BeforeTool": [
                {
                    "matcher": "*",
                    "hooks": [
                        {"type": "command", "command": "/other/program"},
                        {"type": "command", "command": "/usr/bin/test-program gemini-callback"},
                        {"type": "command", "command": "/another/program"}
                    ]
                }
            ]
        }
    });

    gemini.uninstall_from("BeforeTool", &mut json);

    let inner_hooks = json["hooks"]["BeforeTool"][0]["hooks"].as_array().unwrap();
    assert_eq!(inner_hooks.len(), 2);
    assert_eq!(inner_hooks[0]["command"], "/other/program");
    assert_eq!(inner_hooks[1]["command"], "/another/program");
}

#[test]
fn test_uninstall_from_no_hooks_object() {
    let gemini = make_gemini("/usr/bin/test-program");
    let mut json = json!({});

    // Should not panic
    gemini.uninstall_from("BeforeTool", &mut json);
}

#[test]
fn test_uninstall_from_no_wildcard_matcher() {
    let gemini = make_gemini("/usr/bin/test-program");
    let mut json = json!({
        "hooks": {
            "BeforeTool": [
                {
                    "matcher": "Bash",
                    "hooks": [
                        {"type": "command", "command": "/other/program"}
                    ]
                }
            ]
        }
    });

    // Should not panic - only looks at wildcard matcher
    gemini.uninstall_from("BeforeTool", &mut json);

    // Other matcher should be unchanged
    let inner_hooks = json["hooks"]["BeforeTool"][0]["hooks"].as_array().unwrap();
    assert_eq!(inner_hooks.len(), 1);
}

// Discovery tests
#[test]
fn test_gemini_discovery_id() {
    use super::gemini::GeminiDiscovery;
    use crate::providers::ProviderDiscovery;

    let discovery = GeminiDiscovery;
    assert_eq!(discovery.id(), "gemini-cli");
}

#[test]
fn test_gemini_discovery_display_name() {
    use super::gemini::GeminiDiscovery;
    use crate::providers::ProviderDiscovery;

    let discovery = GeminiDiscovery;
    assert_eq!(discovery.display_name(), "Gemini CLI");
}

#[test]
fn test_gemini_discovery_supported_hooks() {
    use super::gemini::GeminiDiscovery;
    use crate::providers::ProviderDiscovery;

    let discovery = GeminiDiscovery;
    let hooks = discovery.supported_hooks();
    assert!(hooks.contains(&"BeforeTool"));
    assert!(hooks.contains(&"SessionStart"));
}
