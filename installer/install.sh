#!/bin/bash
set -euo pipefail

BASE_URL="http://localhost:8000"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"
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

main() {
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
    trap 'rm -rf "$tmp_dir"' EXIT

    # Download binary
    if command -v curl &>/dev/null; then
        curl -fsSL "$download_url" -o "${tmp_dir}/${BINARY_NAME}"
    elif command -v wget &>/dev/null; then
        wget -q "$download_url" -O "${tmp_dir}/${BINARY_NAME}"
    else
        echo "Error: curl or wget is required" >&2
        exit 1
    fi

    # Make executable
    chmod +x "${tmp_dir}/${BINARY_NAME}"

    # Install
    if [ -w "$INSTALL_DIR" ]; then
        mv "${tmp_dir}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
    else
        echo "Installing to ${INSTALL_DIR} (requires sudo)..."
        sudo mv "${tmp_dir}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
    fi

    echo "Successfully installed ${BINARY_NAME} to ${INSTALL_DIR}/${BINARY_NAME}"

    # Run login subcommand
    "${INSTALL_DIR}/${BINARY_NAME}" login
}

main
