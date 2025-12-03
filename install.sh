#!/bin/sh
# ConceptKernel (ckr) installer script
# Usage: curl -sSL https://raw.githubusercontent.com/ConceptKernel/ck-core-rs/main/install.sh | sh

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Configuration
REPO="ConceptKernel/ck-core-rs"
VERSION="${CKR_VERSION:-latest}"
INSTALL_DIR="${CKR_INSTALL_DIR:-/usr/local/bin}"

echo "${GREEN}ConceptKernel (ckr) Installer${NC}"
echo "================================"
echo ""

# Detect OS and architecture
detect_platform() {
    OS="$(uname -s)"
    ARCH="$(uname -m)"

    case "$OS" in
        Linux*)
            OS="linux"
            ;;
        Darwin*)
            OS="darwin"
            ;;
        *)
            echo "${RED}Error: Unsupported operating system: $OS${NC}"
            exit 1
            ;;
    esac

    case "$ARCH" in
        x86_64)
            ARCH="x86_64"
            ;;
        aarch64|arm64)
            ARCH="aarch64"
            ;;
        *)
            echo "${RED}Error: Unsupported architecture: $ARCH${NC}"
            exit 1
            ;;
    esac

    echo "Detected platform: ${GREEN}$OS-$ARCH${NC}"
}

# Get latest version from GitHub
get_latest_version() {
    if [ "$VERSION" = "latest" ]; then
        echo "Fetching latest version..."
        VERSION=$(curl -sSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
        if [ -z "$VERSION" ]; then
            echo "${RED}Error: Could not determine latest version${NC}"
            exit 1
        fi
    fi
    echo "Version: ${GREEN}$VERSION${NC}"
}

# Download and install binary
install_binary() {
    BINARY_NAME="ckr-${VERSION}-${ARCH}-${OS}"
    DOWNLOAD_URL="https://github.com/$REPO/releases/download/$VERSION/$BINARY_NAME"

    echo ""
    echo "Downloading from: $DOWNLOAD_URL"

    # Create temporary directory
    TMP_DIR=$(mktemp -d)
    trap "rm -rf $TMP_DIR" EXIT

    # Download binary
    if ! curl -sSL -f "$DOWNLOAD_URL" -o "$TMP_DIR/ckr"; then
        echo "${RED}Error: Failed to download binary${NC}"
        echo "URL: $DOWNLOAD_URL"
        echo ""
        echo "Available releases: https://github.com/$REPO/releases"
        exit 1
    fi

    # Make executable
    chmod +x "$TMP_DIR/ckr"

    # Verify it runs
    if ! "$TMP_DIR/ckr" --version > /dev/null 2>&1; then
        echo "${RED}Error: Downloaded binary is not executable${NC}"
        exit 1
    fi

    # Install to destination
    echo "Installing to: ${GREEN}$INSTALL_DIR/ckr${NC}"

    if [ -w "$INSTALL_DIR" ]; then
        mv "$TMP_DIR/ckr" "$INSTALL_DIR/ckr"
    else
        echo "${YELLOW}Requesting sudo access to install to $INSTALL_DIR${NC}"
        sudo mv "$TMP_DIR/ckr" "$INSTALL_DIR/ckr"
    fi

    echo ""
    echo "${GREEN}✓ Installation complete!${NC}"
}

# Verify installation
verify_installation() {
    if command -v ckr > /dev/null 2>&1; then
        INSTALLED_VERSION=$(ckr --version 2>&1 | head -1)
        echo ""
        echo "Installed: ${GREEN}$INSTALLED_VERSION${NC}"
        echo ""
        echo "Try it out:"
        echo "  ${YELLOW}ckr --help${NC}"
        echo ""
    else
        echo ""
        echo "${YELLOW}Warning: 'ckr' not found in PATH${NC}"
        echo "Add $INSTALL_DIR to your PATH:"
        echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
        echo ""
    fi
}

# Main installation flow
main() {
    detect_platform
    get_latest_version
    install_binary
    verify_installation
}

main
