# Forensic Audit Report — ID 67

## Executive Summary
- **`zenne-noitu-rs`**: **15 total tests** (6 `#[test]`, 9 `#[tokio::test]`), **0 `tests/` integration directories**, and **no `cargo test` step** in `.github/workflows/build.yml`.
- **`zenne-noitu-deploy-vercel`**: **0 automated tests**.
- **`gdrive-bot-source`**: **0 automated `unittest`/`pytest` suites** (only 2 manual `urllib.request` smoke scripts: `test_health.py`, `test_register.py`).

## Existing Coverage
- `zenne-core/src/auth.rs`: 2 `#[test]` (`test_web_token_sign_verify_and_tamper`, `test_game_session_iat_and_claims`)
- `zenne-core/src/wiki.rs`: 1 `#[test]` (`test_word_chain_vietnamese_and_english`)
- `zenne-api/src/pub_games.rs`: 1 `#[test]` (`test_ttt_minimax_and_sanity_checks`)
- `zenne-games/src/lib.rs`: 1 `#[test]` (`test_minigames_preserved_and_functional`)
- `zenne-music/src/multiplay.rs`: 9 `#[tokio::test]`
- `zenne-discord/src/lib.rs`: 1 `#[test]` (`test_existing_command_registry_preserved`)
- **0 tests** in `zenne-core/{cache,config,db,i18n,kira}`, `zenne-api/{auth_routes,lib,ws_game}`, `zenne-games` (16+ game modules), `zenne-music/src/lib.rs`, `zenne-discord/{extra_cmds,music,noitu,workers}`, `zenne-telegram`, `zenne-revolt`, `zenne-twitch`.

## Highest-Priority Missing Tests
1. **Security & Auth (`auth.rs`, `auth_routes.rs`, `zenne-api/lib.rs`):** 2FA enforcement on login, `/pub/auth/verify-2fa` & `/pub/auth/oauth` authz, `PBKDF2` legacy password verification, `attempts < 5` OTP lockout, `check_admin_auth`, `/pub/game/{ttt,ms}` authz, `sanity_check_score` bounds, nonce replay.
2. **DB & Cache (`db.rs`, `cache.rs`, `wiki.rs`, `i18n.rs`):** Migrations (`ALTER TABLE`), `review_word`/`submit_pending` atomicity, cumulative vs high-score `game_submit_score_mode`, `rate_limited` window/cap, Unicode NFC `wiki::valid_chain`, all 14 `i18n` locales.
3. **WS & Minigames (`ws_game.rs`, `pub_games.rs`, `zenne-games`, bot crates):** WS turn timer, disconnect turn-index, `connect4`/`battleship`/`mastermind`/`wordle` unit tests, `zenne-discord` `noitu` turn/streak/tournament wiring, `zenne-telegram` `tp_*` ownership, `zenne-twitch`/`zenne-revolt` self-message filtering.
4. **CI Gate:** Add `cargo test --workspace` and `cargo clippy --workspace --all-targets -- -D warnings` to `.github/workflows/build.yml`.
