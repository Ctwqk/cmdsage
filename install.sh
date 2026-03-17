#!/usr/bin/env bash
set -euo pipefail

echo "=== CmdSage Installer ==="

# 1. Build release binary
echo "[1/3] Building release binary..."
cargo build --release

# 2. Install binary
INSTALL_DIR="${HOME}/.local/bin"
mkdir -p "$INSTALL_DIR"
cp target/release/cmdsage "$INSTALL_DIR/cmdsage"
echo "[2/3] Installed binary to ${INSTALL_DIR}/cmdsage"

# 3. Install command knowledge base
DATA_DIR="${HOME}/.cmdsage"
mkdir -p "$DATA_DIR"
cp -r commands "$DATA_DIR/commands"
echo "[3/3] Installed command library to ${DATA_DIR}/commands"

# Check PATH
if ! echo "$PATH" | grep -q "${INSTALL_DIR}"; then
    echo ""
    echo "NOTE: Add ${INSTALL_DIR} to your PATH:"
    echo "  echo 'export PATH=\"\$HOME/.local/bin:\$PATH\"' >> ~/.bashrc"
    echo "  source ~/.bashrc"
fi

echo ""
echo "=== Installation complete ==="
echo ""
echo "Usage:"
echo "  cmdsage \"查找所有 py 文件\""
echo "  cmdsage --platform macos \"安装软件\""
echo "  cmdsage config show"
echo ""
echo "Optional: download semantic model (~87MB) for better matching:"
echo "  mkdir -p ~/.cmdsage/models/all-MiniLM-L6-v2"
echo "  wget -O ~/.cmdsage/models/all-MiniLM-L6-v2/model.onnx \\"
echo "    https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/onnx/model.onnx"
echo "  wget -O ~/.cmdsage/models/all-MiniLM-L6-v2/tokenizer.json \\"
echo "    https://huggingface.co/sentence-transformers/all-MiniLM-L6-v2/resolve/main/tokenizer.json"
