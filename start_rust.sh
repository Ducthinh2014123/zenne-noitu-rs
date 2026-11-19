#!/usr/bin/env bash
set -e

echo "=================================================="
echo "🚀 Build & Run Zenne Bot (Rust Edition)"
echo "=================================================="

if ! command -v cargo >/dev/null 2>&1; then
  echo "[*] Dang cai dat Rust toolchain..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source "$HOME/.cargo/env"
fi

echo "[*] Cai dat thu vien he thong (ffmpeg, cmake, libopus, pkg-config)..."
if command -v apt-get >/dev/null 2>&1; then
  sudo apt-get update -y
  sudo apt-get install -y build-essential cmake pkg-config libopus-dev ffmpeg libssl-dev
fi

echo "[*] Dang bien dich release binary sieu nhe..."
cargo build --release

echo "[+] Hoan tat! Dang khoi chay ./target/release/zenne-bot all"
./target/release/zenne-bot all
