// Tests for install.rs functionality

// Note: The should_delete_binary tests were removed because the function was removed.
// The old "uninstall" command had "magic" behavior where it would delete the binary
// when all hooks were removed. This has been replaced with:
// - uninstall_hooks: only removes hooks, never touches binary/config/data
// - uninstall_all: explicitly removes everything (hooks, binary, config, data)
