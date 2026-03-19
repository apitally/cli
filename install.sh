#!/bin/sh

# Apitally CLI installer
# https://github.com/apitally/cli
#
# Usage:
#   curl -fsSL https://cli.apitally.io/install.sh | sh
#
# Environment variables:
#   APITALLY_VERSION     - Install a specific version (e.g. "v0.1.0") instead of latest
#   APITALLY_INSTALL_DIR - Override the install directory (default: ~/.local/bin)

set -u

REPO="apitally/cli"
BINARY="apitally"
TMP_DIR=""

# --- Helper functions --------------------------------------------------------

say() {
    echo "$1"
}

err() {
    local red
    local reset
    red=$(tput setaf 1 2>/dev/null || echo '')
    reset=$(tput sgr0 2>/dev/null || echo '')
    say "${red}error${reset}: $1" >&2
    exit 1
}

# Run a command that should never fail. If it does, exit with an error.
ensure() {
    if ! "$@"; then err "command failed: $*"; fi
}

need_cmd() {
    if ! command -v "$1" > /dev/null 2>&1; then
        err "need '$1' (command not found)"
    fi
}

# Download a URL to a file using curl or wget
downloader() {
    if command -v curl > /dev/null 2>&1; then
        curl -fsSL "$1" -o "$2"
    elif command -v wget > /dev/null 2>&1; then
        wget -q "$1" -O "$2"
    else
        err "need 'curl' or 'wget' (command not found)"
    fi
}

# Fetch a URL and print its contents to stdout
fetch() {
    if command -v curl > /dev/null 2>&1; then
        curl -fsSL "$1"
    elif command -v wget > /dev/null 2>&1; then
        wget -qO- "$1"
    else
        err "need 'curl' or 'wget' (command not found)"
    fi
}

# --- Main --------------------------------------------------------------------

main() {
    # Check that all required commands are available upfront
    need_cmd uname
    need_cmd mktemp
    need_cmd chmod
    need_cmd mkdir
    need_cmd rm
    need_cmd tar

    # Detect OS
    local os
    case "$(uname -s)" in
        Linux)  os="unknown-linux-gnu" ;;
        Darwin) os="apple-darwin" ;;
        *)      err "unsupported operating system: $(uname -s)" ;;
    esac

    # Detect CPU architecture
    local arch
    arch="$(uname -m)"
    case "$arch" in
        x86_64 | amd64) arch="x86_64" ;;
        aarch64 | arm64) arch="aarch64" ;;
        *) err "unsupported architecture: $arch" ;;
    esac

    # On macOS, detect if running under Rosetta 2 emulation and prefer the
    # native arm64 binary instead
    if [ "$os" = "apple-darwin" ] && [ "$arch" = "x86_64" ]; then
        if sysctl hw.optional.arm64 2>/dev/null | grep -q ': 1'; then
            arch="aarch64"
        fi
    fi

    local target="${arch}-${os}"
    local archive="${BINARY}-${target}.tar.gz"

    # Resolve the version to install
    local version
    if [ -n "${APITALLY_VERSION:-}" ]; then
        version="$APITALLY_VERSION"
    else
        local api_response
        api_response=$(fetch "https://api.github.com/repos/${REPO}/releases/latest") \
            || err "could not fetch latest release from GitHub (check your internet connection)"
        version=$(echo "$api_response" | grep '"tag_name"' | sed 's/.*"tag_name": *"//;s/".*//')
        if [ -z "$version" ]; then
            err "could not determine latest version"
        fi
    fi

    # Resolve install directory following XDG precedence
    local install_dir
    if [ -n "${APITALLY_INSTALL_DIR:-}" ]; then
        install_dir="$APITALLY_INSTALL_DIR"
    elif [ -n "${XDG_BIN_HOME:-}" ]; then
        install_dir="$XDG_BIN_HOME"
    else
        install_dir="${HOME}/.local/bin"
    fi

    say "downloading ${BINARY} ${version}..."

    # Create a temp dir and ensure it gets cleaned up on exit
    TMP_DIR=$(mktemp -d) || err "could not create temp directory"
    trap 'rm -rf "$TMP_DIR"' EXIT INT TERM

    # Download the release archive
    local url="https://github.com/${REPO}/releases/download/${version}/${archive}"
    if ! downloader "$url" "${TMP_DIR}/${archive}"; then
        err "failed to download ${url}"
    fi

    # Extract the binary
    ensure tar xzf "${TMP_DIR}/${archive}" --no-same-owner -C "$TMP_DIR"

    # Install the binary
    ensure mkdir -p "$install_dir"
    ensure mv "${TMP_DIR}/${BINARY}" "${install_dir}/${BINARY}"
    ensure chmod +x "${install_dir}/${BINARY}"

    say "installed to ${install_dir}/${BINARY}"

    # Add install dir to PATH if not already there
    case ":${PATH}:" in
        *":${install_dir}:"*)
            ;;
        *)
            local bold
            local reset
            bold=$(tput bold 2>/dev/null || echo '')
            reset=$(tput sgr0 2>/dev/null || echo '')

            # Detect the user's shell to pick the right profile file
            local rc_file
            case "${SHELL:-}" in
                */zsh)  rc_file="${HOME}/.zshrc" ;;
                */bash) rc_file="${HOME}/.bashrc" ;;
                *)      rc_file="${HOME}/.profile" ;;
            esac

            # Add PATH line if the file doesn't already reference the install dir
            if ! grep -qF "$install_dir" "$rc_file" 2>/dev/null; then
                echo >> "$rc_file"
                echo "export PATH=\"${install_dir}:\$PATH\"" >> "$rc_file"
                say "added ${install_dir} to PATH in ${rc_file}"
                say "restart your shell or run: ${bold}source ${rc_file}${reset}"
            fi
            ;;
    esac
}

main "$@"
