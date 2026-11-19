@echo off
chcp 65001 >nul
set "CARGO_TARGET_DIR=%TEMP%\zenne-rs-target"
set "MODE=%~1"
if "%MODE%"=="" set "MODE=all"

echo ==================================================
echo 🚀 Build ^& Run Zenne Bot (Rust - Windows) [%MODE%]
echo ==================================================
cargo run --release -- %MODE%
pause
