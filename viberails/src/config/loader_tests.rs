use std::io::Write;

use tempfile::NamedTempFile;

use crate::config::loader::LcOrg;

use super::loader::{Config, UserConfig};

#[test]
fn test_user_config_default() {
    let config = UserConfig::default();

    assert!(config.fail_open);
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
}

#[test]
fn test_config_load_existing_valid() {
    let json = r#"{
        "user": {
            "fail_open": false
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
    assert_eq!(config.install_id, "test-install-id-123");
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
