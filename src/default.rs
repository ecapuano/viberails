use std::sync::LazyLock;

use anyhow::anyhow;
use rust_embed::Embed;
use serde_json::Value;

#[derive(Embed)]
#[folder = "resources/"]
#[include = "consts.json"]
struct Assets;

#[allow(clippy::expect_used)]
static DEFAULTS: LazyLock<Value> = LazyLock::new(|| {
    let file = Assets::get("consts.json").expect("consts.json embedded");
    serde_json::from_slice(&file.data).expect("valid consts.json")
});

#[allow(clippy::expect_used)]
pub fn get_embedded_default(name: &'static str) -> String {
    DEFAULTS
        .get(name)
        .and_then(Value::as_str)
        .map(String::from)
        .ok_or_else(|| anyhow!("missing default: {name}"))
        .expect("embedded default should exist")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_required_defaults_exist() {
        // These are used at runtime and must exist
        let required_keys = [
            "firebase_api_key",
            "firebase_signup_url",
            "join_team_command",
            "join_team_command_windows",
            "upgrade_url",
            "github_client_id",
        ];

        for key in required_keys {
            let value = get_embedded_default(key);
            assert!(!value.is_empty(), "{key} should not be empty");
        }
    }

    #[test]
    fn test_join_team_command_windows_has_url_placeholder() {
        let cmd = get_embedded_default("join_team_command_windows");
        assert!(
            cmd.contains("{URL}"),
            "Windows join command should contain {{URL}} placeholder"
        );
    }
}
