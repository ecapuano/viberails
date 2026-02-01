# Release Process

1. Update the version in `Cargo.toml`
2. Commit the version change
3. Create and push a git tag matching the version (e.g., `v0.1.6`)

The git tag and `Cargo.toml` version must match. Pushing the git tag will trigger the release build.
