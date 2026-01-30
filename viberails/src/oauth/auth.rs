use anyhow::Result;

use crate::oauth::{LoginArgs, login};

pub fn authorize(config: &LoginArgs) -> Result<()> {
    let _login = login(config)?;

    Ok(())
}
