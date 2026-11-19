# Zenne Bot — Rust Edition (`zenne-noitu-rs`)

Phiên bản viết lại hoàn toàn bằng **Rust** (Tokio + Serenity/Poise + Songbird + Teloxide + Axum + libSQL + Redis), giúp tiết kiệm RAM tối đa (~15–30 MB), không còn hiện tượng memory leak như Python và xử lý đa nền tảng cực nhanh.

## Kiến trúc Workspace
- `crates/zenne-core`: Cấu hình `.env`, đa ngôn ngữ (`languages.json` nhúng sẵn), libSQL/Turso, Redis + bộ nhớ đệm có giới hạn kích thước, Wikipedia/Wiktionary checker, Kira AI, Auth/TOTP/HMAC.
- `crates/zenne-games`: AFK, Tài Xỉu, Bầu Cua, Vua Tiếng Việt, Quiz (Tổng hợp & Toán), Wordle, Đảo chữ (Anagram), Ca Dao, Chính Tả, Emoji, Quốc Kỳ, Lịch Sử, Tôi Là Ai, Mastermind, Connect 4, Bắn Tàu, Memory, Sudoku, Typing Test, UNO.
- `crates/zenne-music`: Bộ giải mã SoundCloud độc quyền (từ chối link ngoài SoundCloud), tự tải và ghép `init.mp4` + `.m4s` thành file `.m4a` cục bộ phát qua Songbird.
- `crates/zenne-api`: Web Admin API & WebSocket dùng Axum (port `26035`).
- `crates/zenne-discord`: Bot Discord (Slash & Prefix `!`, Nối từ, Nhạc, Minigames, AFK, AI).
- `crates/zenne-telegram`: Bot Telegram (`teloxide`).
- `crates/zenne-twitch`: Bot Twitch IRC WebSocket.
- `crates/zenne-revolt`: Bot Revolt (Stoat) WebSocket + REST.

## Cách chạy
```bash
# Chạy toàn bộ 4 bot + Web API
cargo run --release -- all

# Hoặc chạy từng bot riêng lẻ
cargo run --release -- discord
cargo run --release -- telegram
cargo run --release -- twitch
cargo run --release -- revolt
```
Trên Linux VPS, bạn có thể chạy `./start_rust.sh`.
