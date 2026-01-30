#!/bin/bash
set -euo pipefail

BASE_URL="http://localhost:8000"
BINARY_NAME="viberails"

# Detect OS
detect_os() {
    case "$(uname -s)" in
        Linux*)  echo "linux" ;;
        Darwin*) echo "macos" ;;
        MINGW*|MSYS*|CYGWIN*) echo "windows" ;;
        *)
            echo "Error: Unsupported operating system: $(uname -s)" >&2
            exit 1
            ;;
    esac
}

# Detect architecture
detect_arch() {
    case "$(uname -m)" in
        x86_64|amd64) echo "x64" ;;
        aarch64|arm64) echo "arm64" ;;
        *)
            echo "Error: Unsupported architecture: $(uname -m)" >&2
            exit 1
            ;;
    esac
}

do_install() {
    local os arch artifact_name download_url

    os="$(detect_os)"
    arch="$(detect_arch)"

    # Windows arm64 is not supported
    if [ "$os" = "windows" ] && [ "$arch" = "arm64" ]; then
        echo "Error: Windows ARM64 is not supported" >&2
        exit 1
    fi

    artifact_name="viberails-${os}-${arch}"
    download_url="${BASE_URL}/${artifact_name}"

    echo "Detected: ${os} ${arch}"
    echo "Downloading ${artifact_name}..."

    # Create temp directory
    tmp_dir="$(mktemp -d)"
    tmp_file="${tmp_dir}/${artifact_name}"

    # Download binary
    if command -v curl &>/dev/null; then
        curl -fsSL "$download_url" -o "$tmp_file"
    elif command -v wget &>/dev/null; then
        wget -q "$download_url" -O "$tmp_file"
    else
        echo "Error: curl or wget is required" >&2
        exit 1
    fi

    # Make executable
    chmod +x "$tmp_file"

    echo "Successfully downloaded ${BINARY_NAME}"

    # Run login subcommand
    "$tmp_file" login

    # Run install subcommand
    "$tmp_file" install

    # Clean up temp directory
    rm -rf "$tmp_dir"
}

do_uninstall() {
    echo "Uninstall is not supported (binary is not permanently installed)"
    exit 1
}

main() {
    local command="${1:-install}"

    case "$command" in
        install)
            do_install
            ;;
        uninstall)
            do_uninstall
            ;;
        *)
            echo "Error: Unknown command: $command" >&2
            echo "Usage: $0 [install|uninstall]" >&2
            exit 1
            ;;
    esac
}

main "$@"
