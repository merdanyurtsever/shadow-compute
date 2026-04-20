#!/bin/bash
echo "[Deploy] Compiling Shadow Matrix (Release)..."
cargo build --release

echo "[Deploy] Symlinking binaries to ~/.local/bin..."
mkdir -p ~/.local/bin
ln -sf $(pwd)/target/release/daemon ~/.local/bin/shadow-daemon
ln -sf $(pwd)/target/release/cli ~/.local/bin/shadow-cli

echo "[Deploy] Restarting systemd service..."
systemctl --user restart shadow-matrix.service

echo "[Deploy] Matrix updated and online."
