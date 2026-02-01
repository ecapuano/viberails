use git2::{DescribeOptions, Repository};

fn main() {
    let (git_hash, git_version) = match Repository::discover(".") {
        Ok(repo) => {
            let hash = repo
                .head()
                .ok()
                .and_then(|head| head.peel_to_commit().ok())
                .map_or_else(
                    || "unknown".to_string(),
                    |commit| {
                        let id = commit.id();
                        // Short hash with 'g' prefix (git convention)
                        format!("g{}", &id.to_string()[..7])
                    },
                );

            // Get version from the nearest tag only
            let version = repo
                .describe(DescribeOptions::new().describe_tags())
                .ok()
                .and_then(|desc| desc.format(None).ok())
                .map_or_else(
                    || "unknown".to_string(),
                    |v| {
                        // Strip 'v' prefix and any suffix like "-2-g1fa6a60"
                        let v = v.strip_prefix('v').unwrap_or(&v);
                        v.split('-').next().unwrap_or(v).to_string()
                    },
                );

            (hash, version)
        }
        Err(_) => ("gunknown".to_string(), "unknown".to_string()),
    };

    println!("cargo::rerun-if-changed=.git/HEAD");
    println!("cargo::rerun-if-changed=.git/refs/heads");
    println!("cargo::rerun-if-changed=Cargo.toml");

    println!("cargo:rustc-env=GIT_HASH={git_hash}");
    println!("cargo:rustc-env=GIT_VERSION={git_version}");
}
