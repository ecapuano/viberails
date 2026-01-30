use std::sync::LazyLock;

use anyhow::{Result, anyhow};
use rust_embed::Embed;
use serde_json::Value;

#[derive(Embed)]
#[folder = "resources/"]
#[include = "consts.json"]
struct Assets;

static DEFAULTS: LazyLock<Value> = LazyLock::new(|| {
    let file = Assets::get("consts.json").expect("consts.json embedded");
    serde_json::from_slice(&file.data).expect("valid consts.json")
});

pub fn get_embedded_default(name: &'static str) -> Result<String> {
    DEFAULTS
        .get(name)
        .and_then(Value::as_str)
        .map(String::from)
        .ok_or_else(|| anyhow!("missing default: {name}"))
}
