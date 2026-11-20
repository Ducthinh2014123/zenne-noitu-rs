# Forensic Audit Report — ID 70

## 1. Coverage Verification (Reports 01–69)
All **69 audit reports** (`01_word_chain_core.md` through `69_concurrency_stress_audit.md`) are present in `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\docs\migration-70-agent\` with **no missing domains or coverage gaps**:
- **01–10:** Core word-chain engine, Wiki/i18n, wordbank review, tournaments/leaderboards, Discord admin, Telegram word-chain & minigames, SoundCloud music, MultiPlay workers, Twitch/Revolt.
- **11–16:** Full minigame parity (Quiz, VTV/Cadao/ChinhTa/Flag, Wordle/Anagram/Catword/Tamsao, Triệu Phú/Guess, Betting/Logic, Board/Arcade).
- **17–25:** HTTP Admin/Public/Game/Room APIs, WebSocket room, Auth (Registration/OTP, Login/2FA, OAuth/Game-Token, Account Management, Password Recovery).
- **26–30:** DB schemas/migrations, indexes/queries, atomicity/concurrency, Redis cache parity, Turso web DB compatibility.
- **31–40:** Python dynamic registration, hidden commands/aliases, exceptions/fallbacks, legacy config/env, process lifecycle, HTTP timeouts, Unicode normalization, rare dictionaries, temp paths, feature call graph.
- **41–50:** Discord slash/prefix/perms/voice events, Telegram callbacks/formatting/errors, SoundCloud download errors, MultiPlay races, multiplayer lifecycle, minigame anti-cheat, leaderboards/economy.
- **51–60:** Trust boundaries, OAuth token rotation, crypto/signing keys, account takeover, API authz/rate limits, DB atomicity, Tokio shutdown/leaks, Redis cache consistency, dependencies/unsafe config, adversarial review.
- **61–69:** NextAuth/Vercel session & API contracts, WS payloads/reconnect, web multiplayer UI state, DB migration compat, persistence audit, test coverage, property/fuzz targets, concurrency stress designs.

---

## 2. Contradictions & False Positives Identified (Spot-Checked Against Source)
1. **Empty `API_KEY` Auth Bypass (Reports 53 & 55 vs. Report 60):**
   - `AppConfig::from_env()` (`zenne-core/src/config.rs:45-51, 69`) falls back to `"Duc thinh 1234@"`, so `api_key` is never empty via `from_env()`. However, `check_admin_auth` (`lib.rs:30-44`) and `ah_pub_game_score`/`ah_pub_game_session_start` (`lib.rs:460, 585`) still have a latent fail-open bug if constructed with `""`.
2. **Missing `flag_quiz` Integration in `zenne-telegram` (Report 40 vs. Report 12):**
   - **False positive in Report 40** — `/flag` (`flagquiz`, `quocky`) is registered in `zenne-telegram/src/lib.rs:39, 100` and routed to `minigames::cmd_flag` (though it uses its own 1-strike handler rather than `zenne-games::flag_quiz`).
3. **WS `uid=0` vs. Positive Web `user_id` (Reports 51, 55, 60, 62):**
   - Authenticated web players via `pages/api/pub/game-token.js:18` receive **positive** `session.user.id`, which `ws_game.rs:457` writes to `leaderboard.user_id` (colliding with Discord/Telegram IDs). Unauthenticated/malformed `"join"` frames without `uid` default to `0`, bypassing duplicate-user checks (`ws_game.rs:244`) while skipping DB writes (`if p_uid != 0`).

---

## 3. Prioritized Implementation Plan
- **P0 (Critical Security & Data Integrity):** Eliminate 2FA bypass & unauthenticated token minting (`handle_login`, `ah_auth_verify_2fa`, `handle_register_email_verify`, `ah_auth_oauth`, `handle_disable_2fa`); fail-closed `check_admin_auth` + constant-time compare; add `check_admin_auth` to `/api/leaderboard`, `/pub/game/ttt`, `/pub/game/ms`; PBKDF2 legacy password verification + `OsRng`; atomic `attempts < 5` & `ALTER TABLE` migrations; negative `user_id` in `game-token.js` + reject `uid=0` in `ws_game.rs`.
- **P1 (High Parity & Correctness):** NextAuth `session.error` forwarding & refresh-loop fix; `setup-2fa` `qr_url`; `ah_pub_game_score` allowlist & `sanity_check_score`; WS `new_host` stale closure fix & server turn timer; Discord `noitu` turn/streak/timeout/admin/tournament wiring; Telegram cumulative scoring + `taixiu` triple fix + `tp_*` ownership; Wiki NFC + Wiktionary-first + no negative cache on network errors; Cache `Mutex<ConnectionManager>` removal + cap.
- **P2 (Medium Reliability & UX):** `CancellationToken` graceful shutdown; `ws_rooms` pre-join leak & `room_tokens` cleanup; `download_audio_with_prefix` status check + `OnceLock<Regex>` + `multiplay.rs` `worker_guild_assignments` leak & lock-order fix; Discord `connect4` off-by-one fix; Twitch/Revolt self-message filter; Telegram `html_escape(lang)` + 4096-char splitting.
- **P3 (Tests, CI & Polish):** Add unit/integration/concurrency tests across crates, fix `zenne-telegram` `dead_code` warnings, verify `cargo fmt`, `cargo check`, `cargo test`, `cargo clippy -- -D warnings`, and `npm run build`.
