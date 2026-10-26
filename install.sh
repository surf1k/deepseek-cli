#!/usr/bin/env bash
set -euo pipefail

# DeepSeek CLI Installer
# One-line installation:
#   curl -fsSL https://raw.githubusercontent.com/surf1k/deepseek-cli/main/install.sh | bash

REPO="surf1k/deepseek-cli"
BIN_NAME="deepseek"
INSTALL_DIR="${HOME}/.local/bin"

mkdir -p "${INSTALL_DIR}"

echo -e "\033[1;34m"
cat << 'EOF'
   ___               ____            _      ____ _     ___ 
  / _ \___ ___ ___  / __/__ ___ k   | |    / ___| |   |_ _|
 / // / -_) -_) _ \_\ \/ -_) -_) k  | |   | |   | |    | | 
/____/\__/\__/_//_/___/\__/\__/_/_/ |_|   |_|   |___| |___|
EOF
echo -e "\033[0m"
echo "Installing DeepSeek CLI (${REPO})..."

# Check if cargo is available for instant compilation
if command -v cargo >/dev/null 2>&1; then
    echo "Rust toolchain detected. Installing via cargo..."
    cargo install --git "https://github.com/${REPO}" --force
    echo ""
    echo -e "\033[1;32m✓ DeepSeek CLI installed successfully via cargo!\033[0m"
else
    # Detect OS and Arch
    OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
    ARCH="$(uname -m)"

    case "${ARCH}" in
        x86_64|amd64)
            TARGET_ARCH="x86_64"
            ;;
        aarch64|arm64)
            TARGET_ARCH="aarch64"
            ;;
        *)
            echo "Unsupported architecture: ${ARCH}"
            exit 1
            ;;
    esac

    case "${OS}" in
        linux)
            TARGET_OS="unknown-linux-gnu"
            ;;
        darwin)
            TARGET_OS="apple-darwin"
            ;;
        *)
            echo "Unsupported OS: ${OS}"
            exit 1
            ;;
    esac

    TARGET="${TARGET_ARCH}-${TARGET_OS}"
    RELEASE_URL="https://github.com/${REPO}/releases/latest/download/deepseek-${TARGET}.tar.gz"

    echo "Downloading prebuilt binary for ${TARGET}..."
    TMP_DIR="$(mktemp -d)"
    trap 'rm -rf "${TMP_DIR}"' EXIT

    if curl -fSL "${RELEASE_URL}" -o "${TMP_DIR}/deepseek.tar.gz" 2>/dev/null; then
        tar -xzf "${TMP_DIR}/deepseek.tar.gz" -C "${TMP_DIR}"
        cp "${TMP_DIR}/deepseek" "${INSTALL_DIR}/deepseek"
        chmod +x "${INSTALL_DIR}/deepseek"
        echo -e "\033[1;32m✓ Downloaded and installed to ${INSTALL_DIR}/deepseek\033[0m"
    else
        echo "Could not fetch prebuilt binary for ${TARGET}."
        echo "Please install Rust (https://rustup.rs) and run: cargo install --git https://github.com/${REPO}"
        exit 1
    fi
fi

# Ensure ~/.local/bin is in PATH
if [[ ":$PATH:" != *":${INSTALL_DIR}:"* ]]; then
    echo ""
    echo -e "\033[1;33mNotice:\033[0m ${INSTALL_DIR} is not in your PATH."
    echo "Add this line to your ~/.bashrc or ~/.zshrc:"
    echo "  export PATH=\"\${HOME}/.local/bin:\${PATH}\""
fi

echo ""
echo -e "\033[1;36mGet started with 100% Free Web Account or API Key:\033[0m"
echo "  deepseek          # Free Web Account login or interactive mode"
echo "  deepseek balance  # Check your account balance & status"
echo ""
echo "Or run autonomous coding agent:"
echo "  deepseek agent \"find bugs in tests and run cargo test\""
