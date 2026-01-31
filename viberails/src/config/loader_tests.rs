use std::io::Write;

use tempfile::NamedTempFile;

use crate::config::loader::LcOrg;

use super::loader::{Config, UserConfig, parse_team_url};

#[test]
fn test_user_config_default() {
    let config = UserConfig::default();

    assert!(config.fail_open);
    assert!(config.audit_tool_use);
    assert!(config.audit_prompts);
}

#[test]
fn test_user_config_builder() {
    let config = UserConfig::builder().fail_open(false).build();

    assert!(!config.fail_open);
}

#[test]
fn test_user_config_serialization() {
    let config = UserConfig::default();
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: UserConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(config.fail_open, deserialized.fail_open);
    assert_eq!(config.audit_tool_use, deserialized.audit_tool_use);
    assert_eq!(config.audit_prompts, deserialized.audit_prompts);
}

#[test]
fn test_user_config_audit_fields_serialization() {
    let config = UserConfig {
        fail_open: true,
        audit_tool_use: false,
        audit_prompts: false,
    };
    let json = serde_json::to_string(&config).unwrap();

    assert!(json.contains("\"audit_tool_use\":false"));
    assert!(json.contains("\"audit_prompts\":false"));

    let deserialized: UserConfig = serde_json::from_str(&json).unwrap();
    assert!(!deserialized.audit_tool_use);
    assert!(!deserialized.audit_prompts);
}

#[test]
fn test_config_load_existing_valid() {
    let json = r#"{
        "user": {
            "fail_open": false,
            "audit_tool_use": true,
            "audit_prompts": false
        },
        "install_id": "test-install-id-123",
        "org": {
            "oid": "",
            "jwt": "",
            "name": "",
            "url": ""
        }
    }"#;

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(json.as_bytes()).unwrap();

    let config = Config::load_existing(temp_file.path()).unwrap();

    assert!(!config.user.fail_open);
    assert!(config.user.audit_tool_use);
    assert!(!config.user.audit_prompts);
    assert_eq!(config.install_id, "test-install-id-123");
}

#[test]
fn test_config_load_existing_backwards_compatible() {
    // Old config format without audit_tool_use and audit_prompts fields
    // Should default to true for both
    let json = r#"{
        "user": {
            "fail_open": false
        },
        "install_id": "test-install-id-123",
        "org": {
            "oid": "",
            "name": "",
            "url": ""
        }
    }"#;

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(json.as_bytes()).unwrap();

    let config = Config::load_existing(temp_file.path()).unwrap();

    assert!(!config.user.fail_open);
    // New fields should default to true for backwards compatibility
    assert!(config.user.audit_tool_use);
    assert!(config.user.audit_prompts);
}

#[test]
fn test_config_load_existing_invalid_json() {
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(b"not valid json").unwrap();

    let result = Config::load_existing(temp_file.path());

    assert!(result.is_err());
}

#[test]
fn test_config_load_existing_missing_fields() {
    let json = r#"{"user": {}}"#;

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(json.as_bytes()).unwrap();

    let result = Config::load_existing(temp_file.path());

    assert!(result.is_err());
}

#[test]
fn test_config_load_existing_nonexistent_file() {
    let result = Config::load_existing(std::path::Path::new("/nonexistent/path/config.json"));
    assert!(result.is_err());
}

#[test]
fn test_config_serialization_roundtrip() {
    let config = Config {
        user: UserConfig::builder().fail_open(true).build(),
        install_id: "roundtrip-test-id".to_string(),
        org: LcOrg::default(),
    };

    let json = serde_json::to_string_pretty(&config).unwrap();
    let deserialized: Config = serde_json::from_str(&json).unwrap();

    assert_eq!(config.user.fail_open, deserialized.user.fail_open);
    assert_eq!(config.install_id, deserialized.install_id);
}

// Tests for parse_team_url

#[test]
fn test_parse_team_url_valid() {
    let url = "https://hooks.limacharlie.io/abc123/viberails/secret-token";
    let oid = parse_team_url(url).unwrap();
    assert_eq!(oid, "abc123");
}

#[test]
fn test_parse_team_url_valid_with_trailing_slash() {
    let url = "https://hooks.limacharlie.io/org-id-456/adapter/secret/";
    let oid = parse_team_url(url).unwrap();
    assert_eq!(oid, "org-id-456");
}

#[test]
fn test_parse_team_url_rejects_http() {
    let url = "http://hooks.limacharlie.io/abc123/viberails/secret";
    let result = parse_team_url(url);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("HTTPS"));
}

#[test]
fn test_parse_team_url_rejects_missing_segments() {
    let url = "https://hooks.limacharlie.io/abc123/viberails";
    let result = parse_team_url(url);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid team URL format"));
}

#[test]
fn test_parse_team_url_rejects_empty_oid() {
    let url = "https://hooks.limacharlie.io//viberails/secret";
    let result = parse_team_url(url);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("empty"));
}

#[test]
fn test_parse_team_url_rejects_no_path() {
    let url = "https://hooks.limacharlie.io";
    let result = parse_team_url(url);
    assert!(result.is_err());
}

#[test]
fn test_parse_team_url_rejects_invalid_url() {
    let url = "not-a-valid-url";
    let result = parse_team_url(url);
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid URL"));
}

#[test]
fn test_parse_team_url_rejects_no_host() {
    let url = "https:///abc123/viberails/secret";
    let result = parse_team_url(url);
    assert!(result.is_err());
}
