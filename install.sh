#!/bin/bash
# Agent Process Manager (APM) Installation Script

set -e

# Configuration
REPO="sunnya97/agent-process-manager"
INSTALL_DIR="/usr/local/bin"
BINARY_NAME="apm"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Functions
error() {
    echo -e "${RED}Error: $1${NC}" >&2
    exit 1
}

info() {
    echo -e "${GREEN}$1${NC}"
}

warning() {
    echo -e "${YELLOW}$1${NC}"
}

# Detect OS and architecture
detect_platform() {
    local os=""
    local arch=""
    
    # Detect OS
    case "$(uname -s)" in
        Linux*)     os="linux";;
        Darwin*)    os="macos";;
        *)          error "Unsupported operating system: $(uname -s)";;
    esac
    
    # Detect architecture
    case "$(uname -m)" in
        x86_64)     arch="x64";;
        amd64)      arch="x64";;
        arm64)      arch="arm64";;
        aarch64)    arch="arm64";;
        *)          error "Unsupported architecture: $(uname -m)";;
    esac
    
    # Special case: macOS ARM64 is supported, Linux ARM64 is not yet
    if [[ "$os" == "linux" && "$arch" == "arm64" ]]; then
        error "Linux ARM64 is not yet supported"
    fi
    
    echo "${os}-${arch}"
}

# Get the latest release version
get_latest_version() {
    local version=$(curl -s "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
    
    if [[ -z "$version" ]]; then
        error "Failed to get latest version"
    fi
    
    echo "$version"
}

# Download and install APM
install_apm() {
    local version="${1:-$(get_latest_version)}"
    local platform="$(detect_platform)"
    local download_url="https://github.com/${REPO}/releases/download/${version}/apm-${platform}"
    local temp_file="/tmp/apm-download-$$"
    
    info "Installing APM ${version} for ${platform}..."
    
    # Download binary
    info "Downloading from ${download_url}..."
    if ! curl -sSL "${download_url}" -o "${temp_file}"; then
        error "Failed to download APM binary"
    fi
    
    # Make binary executable
    chmod +x "${temp_file}"
    
    # Check if we need sudo
    if [[ -w "${INSTALL_DIR}" ]]; then
        mv "${temp_file}" "${INSTALL_DIR}/${BINARY_NAME}"
    else
        warning "Installing to ${INSTALL_DIR} requires sudo access"
        sudo mv "${temp_file}" "${INSTALL_DIR}/${BINARY_NAME}"
    fi
    
    # Verify installation
    if command -v apm &> /dev/null; then
        info "APM installed successfully!"
        apm --version
    else
        error "Installation failed - APM not found in PATH"
    fi
}

# Main script
main() {
    echo "Agent Process Manager (APM) Installer"
    echo "===================================="
    echo
    
    # Parse arguments
    local version=""
    if [[ $# -gt 0 ]]; then
        version="$1"
        if [[ ! "$version" =~ ^v[0-9]+\.[0-9]+\.[0-9]+ ]]; then
            error "Invalid version format. Use format: v0.3.0"
        fi
    fi
    
    # Check for existing installation
    if command -v apm &> /dev/null; then
        local current_version=$(apm --version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' || echo "unknown")
        warning "APM is already installed (version: ${current_version})"
        read -p "Do you want to reinstall? (y/N) " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            info "Installation cancelled"
            exit 0
        fi
    fi
    
    # Install APM
    install_apm "$version"
    
    echo
    info "Installation complete! Run 'apm --help' to get started."
    info "To start the APM daemon, run: apm start"
}

# Run main function
main "$@"