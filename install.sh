#!/usr/bin/env bash

set -e

REPO_URL="https://github.com/Jlesster/pacwoman"
CLONE_DIR="/tmp/pacwoman"

echo "==> Installing rustup..."
sudo pacman -S --needed rustup

echo "==> Setting Rust stable toolchain..."
rustup default stable

echo "==> Cloning pacwoman repository..."

if [ -d "$CLONE_DIR" ]; then
    echo "==> Removing existing clone..."
    rm -rf "$CLONE_DIR"
fi

git clone "$REPO_URL" "$CLONE_DIR"

cd "$CLONE_DIR"

echo "==> Building pacwoman..."
cargo build --release

echo "==> Installing binary..."
sudo cp target/release/pacwoman /usr/local/bin

echo "==> Done!"
echo "You can now run:"
echo "    pacwoman"
