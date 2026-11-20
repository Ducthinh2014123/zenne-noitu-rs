# Operation 70 — Final Python-to-Rust Migration & Parity Audit

**Project:** Zenne-Noitu (`gdrive-bot` Python → `zenne-noitu-rs` Rust + `zenne-noitu-deploy-vercel` Next.js)  
**Branch:** `migration/python-parity`  
**Audit Artifacts:** `docs/migration-70-agent/01_*.md` through `70_final_independent_reviewer.md`

---

## 1. Executive Summary

All 70 forensic specialist investigations (`01`–`70`) were completed and cross-verified. Every verified P0–P3 gap across authentication, cryptographic token lifecycles, database schema migrations, dictionary normalization, Redis/in-memory caching, Discord/Telegram/Twitch/Revolt/Music parity, WebSocket multiplayer, and Next.js Web/Vercel integration has been implemented and verified.

MultiPlay (10 SoundCloud music workers, no Lavalink), existing Discord/Telegram/Twitch/Revolt commands, minigames, and database schemas are preserved without regression.

---

## 2. Implemented Remediation Batches

### Batch 1 — Authentication, Security & Token Lifecycle (`zenne-core`, `zenne-api`, `zenne-noitu-deploy-vercel`)
- **Constant-time verification & CSPRNG:** Added `constant_time_eq`, `verify_raw_token`, `sign_2fa_challenge`, `verify_2fa_challenge`, `OsRng` for game session tokens, OTPs, and backup codes (`crates/zenne-core/src/auth.rs`, `crates/zenne-api/src/auth_routes.rs`).
- **Legacy PBKDF2 migration:** Added `is_legacy_pbkdf2_hash` / `verify_pbkdf2_sha256` (260,000 iterations) in `auth.rs` with transparent bcrypt re-hashing on login in `auth_routes.rs`.
- **2FA & Auth Hardening:**
  - `handle_login` no longer issues a 7-day web token when `totp_enabled == 1`; returns `pre_auth_token` (`purpose: "2fa_challenge"`).
  - `ah_auth_verify_2fa` requires either a valid `pre_auth_token` or `check_admin_auth`, enforces `totp_enabled == 1 AND email_verified == 1`, and rate-limits (`verify_2fa:{uid}`, 5/300s).
  - `handle_toggle_2fa` requires valid TOTP/backup code to disable 2FA (removed password-only bypass).
  - `handle_register_email_verify` removed the pre-verification token-minting shortcut; OTP verification enforces `AND attempts < 5` and constant-time comparison.
  - `handle_recovery_reset_password` adds `rec_reset_2fa:{uid}` rate limiting and checks DB update errors.
  - `ah_pub_account_setup_2fa` guards against overwriting an active `totp_enabled == 1` secret and returns `qr_url` / `qr_uri` / `uri` / `otpauth_url`.
- **Admin API Key & OAuth:**
  - `check_admin_auth` fails closed when `api_key` is empty and uses `constant_time_eq`.
  - Enforced `check_admin_auth` on `ah_leaderboard`, `ah_pub_game_score`, `ah_pub_game_session_start`, `ah_pub_game_ttt`, `ah_pub_game_ms`, and `ah_auth_oauth`.
  - `ah_auth_oauth` handles `UNIQUE(username)` collisions cleanly instead of returning `{ "ok": true }` without a user.
- **Web / NextAuth (`zenne-noitu-deploy-vercel`):**
  - `[...nextauth].js`: passes `X-API-Key` on `/pub/auth/oauth` & `/pub/auth/verify-2fa`, passes `pre_auth_token`, adds 60s expiry skew + `RefreshAccessTokenError` short-circuit in `jwt`, and forwards `session.error`.
  - `pages/api/pub/game-token.js`: sends negative `user_id` for web users to prevent Discord snowflake collisions.

### Batch 2 — Database, Wordbank, Wiki & Cache (`zenne-core`, `zenne-api`)
- **Database (`crates/zenne-core/src/db.rs`):**
  - Added idempotent `ALTER TABLE` migrations in `init_schema()`.
  - Made `review_word` atomic (`AND status = 'pending'`, checking affected rows).
  - Enforced `COLLATE NOCASE` in `word_exists` and deterministic tiebreakers in `lb_get_top`, `lb_streak_top`, `game_top`, and `tournament_players`.
  - Extended `game_submit_score` cumulative mode parity (`afk`, `taixiu`, `baucua`, `trivia`, `math`, `trieuphu`, `wordle`, `scramble`).
- **Wiki / Normalization (`crates/zenne-core/src/wiki.rs`):**
  - Strips control and zero-width characters in `normalize_word`.
  - Validates `chars().count() <= 80` instead of UTF-8 byte length.
  - Queries Wiktionary first for `_WIKTIONARY_PRIMARY` languages and only caches `"0"` on definitive misses (not transient network/429/5xx failures).
- **Cache & Rate Limiting (`crates/zenne-core/src/cache.rs`):**
  - Removed global `Mutex<ConnectionManager>` bottleneck; clones lock-free `ConnectionManager`.
  - Respects remaining Redis TTL on L1 backfill, enforces the 50,000-key memory cap in `rate_limited`, and preserves fixed-window expiry timestamps.
- **Public Games (`crates/zenne-api/src/pub_games.rs`, `lib.rs`):**
  - Added `ALLOWED_GAMES` allowlist and `.trim().to_lowercase()` normalization in `sanity_check_score` and `ah_pub_game_score`.

### Batch 3 — Bot, Music, WebSocket & Lifecycle Parity
- **Telegram (`crates/zenne-telegram`):**
  - Removed unused `VtvSession.scrambled` warning; verified and cleaned up `join_token` / `room_tokens` on room join/stop/end.
  - Handled `taixiu` triple ("BÃO") outcome, added player ownership checks + score submission on `tp_5050` / `tp_aud` / `tp_stop`, and cancelled timeouts on bot surrender.
- **Discord (`crates/zenne-discord`):**
  - Fixed `connect4` column `0` / `> 7` bounds check (`extra_cmds.rs`).
  - Added `is_guild_admin` / host authorization checks on `setnoitu`, `setlang`, `tochucgiaidau`, `ketthucgiaidau`, `batdaulai`, and `stop`.
  - Wired `get_game_channel`, `lb_add_game`, `lb_reset_streak`, `tournament_add_word`, multiplayer turn order enforcement, and per-round turn timeouts (`noitu.rs`).
- **Music (`crates/zenne-music`):**
  - Replaced repeated `Regex::new` calls with `OnceLock<Regex>`, added `401`/`403` `client_id` refresh retry on transcoding URLs, checked `res.status().is_success()` in `download_audio_with_prefix`, and fixed `release_worker_guild` lock/cleanup ordering (`multiplay.rs`).
- **Twitch & Revolt (`crates/zenne-twitch`, `crates/zenne-revolt`):**
  - Added Twitch `CAP REQ` and self-message filtering; added Revolt `/users/@me` `bot_id` self-message filtering.
- **WebSocket & Frontend (`crates/zenne-api/src/ws_game.rs`, `pages/pub/play/[roomId].jsx`):**
  - Rejected `uid == 0` WS joins, skipped eliminated (`lives <= 0`) players on `turn_idx` adjustment, cleaned up pre-join empty `ws_rooms`, and fixed `new_host` stale closure with `myNameRef`.
- **Graceful Shutdown (`src/main.rs`):**
  - Aborted spawned bot tasks cleanly on `Ctrl+C` / API exit.

---

## 3. Verification Results

- `cargo fmt --all -- --check`: Pass
- `cargo test --workspace`: Pass (all unit tests across `zenne-core`, `zenne-games`, `zenne-api`, `zenne-music`, `zenne-discord`)
- `cargo clippy --workspace --all-targets -- -D warnings`: Pass
- `npm run build` (`zenne-noitu-deploy-vercel`): Pass
