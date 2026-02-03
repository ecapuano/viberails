use std::process::Command;

fn main() {
    let git_hash = Command::new("git")
        .args(["rev-parse", "--short=7", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map_or_else(|| "gunknown".to_string(), |s| format!("g{}", s.trim()));

    let git_version = Command::new("git")
        .args(["describe", "--tags", "--abbrev=0"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map_or_else(
            || "unknown".to_string(),
            |s| {
                let trimmed = s.trim();
                trimmed.strip_prefix('v').unwrap_or(trimmed).to_string()
            },
        );

    println!("cargo::rerun-if-changed=.git/HEAD");
    println!("cargo::rerun-if-changed=.git/refs/heads");
    println!("cargo::rerun-if-changed=Cargo.toml");

    println!("cargo:rustc-env=GIT_HASH={git_hash}");
    println!("cargo:rustc-env=GIT_VERSION={git_version}");

    // Embed icon and version info in Windows executable
    #[cfg(windows)]
    {
        use winres::VersionInfo;

        let mut res = winres::WindowsResource::new();
        res.set_icon("resources/windows/assets/icon.ico");
        res.set("ProductName", "VibeRails");
        res.set("FileDescription", "VibeRails");
        res.set("LegalCopyright", "Copyright © 2026");
        res.set("ProductVersion", &git_version);
        res.set("FileVersion", &git_version);

        // Parse git_version (e.g., "1.2.3") into numeric version
        let version_parts: Vec<u64> = git_version
            .split('.')
            .filter_map(|s| s.parse().ok())
            .collect();
        let major = version_parts.first().copied().unwrap_or(0);
        let minor = version_parts.get(1).copied().unwrap_or(0);
        let patch = version_parts.get(2).copied().unwrap_or(0);
        let numeric_version = (major << 48) | (minor << 32) | (patch << 16);
        res.set_version_info(VersionInfo::FILEVERSION, numeric_version);
        res.set_version_info(VersionInfo::PRODUCTVERSION, numeric_version);

        if let Err(e) = res.compile() {
            eprintln!("cargo:warning=Failed to compile Windows resources: {e}");
        }
    }
}
