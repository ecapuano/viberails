use std::{fs, io::Write, path::Path, thread::sleep, time::Duration};

use anyhow::{Context, Result, bail};
use colored::Colorize;
use log::{info, warn};
use tiny_http::StatusCode;

use crate::{
    common::{EXECUTABLE_EXT, EXECUTABLE_NAME, PROJECT_NAME},
    default::get_embedded_default,
    hooks::binary_location,
};

const DEF_COPY_ATTEMPS: usize = 4;

pub fn get_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        _ => std::env::consts::ARCH,
    }
}

#[cfg(not(windows))]
pub fn make_executable(file_path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(file_path, fs::Permissions::from_mode(0o755))
        .with_context(|| format!("Unable to make {} executable", file_path.display()))?;
    Ok(())
}

pub fn upgrade() -> Result<()> {
    let plat = std::env::consts::OS;
    let arch = get_arch();

    let base_url = get_embedded_default("upgrade_url");

    let url = format!("{base_url}/{PROJECT_NAME}-{plat}-{arch}{EXECUTABLE_EXT}");

    //
    // We'll save it to a tmp file first and then install it where it shoud
    // be if this works
    //
    let td = tempfile::Builder::new()
        .prefix("upgrade_")
        .tempdir()
        .context("Unable to create a temp directory")?;

    let tmp_file = td.path().join(EXECUTABLE_NAME);

    let mut fd = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&tmp_file)
        .with_context(|| format!("Unable to open {} for writing", tmp_file.display()))?;

    info!("Downloading: {url}");

    let res = minreq::get(&url)
        .send()
        .with_context(|| format!("{url} failed"))?;

    if !(200..300).contains(&res.status_code) {
        let status_str = StatusCode::from(res.status_code).default_reason_phrase();
        bail!("{url} returned {} ({})", res.status_code, status_str);
    }

    fd.write_all(res.as_bytes())?;
    fd.flush()?;

    drop(fd);

    #[cfg(not(windows))]
    make_executable(&tmp_file)?;

    let dst = binary_location()?;

    let mut attempts = DEF_COPY_ATTEMPS;

    loop {
        let ret = fs::copy(&tmp_file, &dst);

        if ret.is_ok() {
            break;
        }

        if let Err(e) = ret {
            warn!(
                "Unable to copy {} to {} ({e})",
                tmp_file.display(),
                dst.display()
            );

            if 0 == attempts {
                return Err(e).with_context(|| {
                    format!(
                        "Unable to copy {} to {})",
                        tmp_file.display(),
                        dst.display()
                    )
                })?;
            }
        }

        attempts = attempts.saturating_sub(1);
        sleep(Duration::from_secs(5));
    }

    let msg = format!("Successfully upgraded {}", dst.display()).green();
    println!("{msg}");
    Ok(())
}
