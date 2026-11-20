# Python → Rust Migration Report

- **Branch:** `migration/python-parity` (branched from `main` @ `2c6d400`)
- **Legacy Python Reference:** `C:\Users\Ai Hoi\Downloads\gdrive-bot-source\gdrive-bot\`
- **Rust Workspace:** `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\`
- **Web/Vercel Client:** `C:\Users\Ai Hoi\Downloads\zenne-noitu-deploy-vercel\`

---

## 1. Summary Metrics
- **Total legacy feature groups audited:** 29 (`F-01` – `F-29` in `docs/PYTHON_TO_RUST_PARITY_AUDIT.md`)
- **Already implemented in Rust before this pass:** Telegram 24-command suite (`F-17`), DB 12-table schema (`F-02`), 10-worker MultiPlay SoundCloud pool
- **Restored / fixed in this pass:**
  - All 10 priority security & token lifecycle items (constant-time HMAC, `iat` game sessions, WS `verify_web_token`, mandatory `session_token` + monotonic nonce, `X-API-Key` on `/pub/auth/game-token` + `/pub/game/*`, recovery `expires_at` + 2FA + atomic consumption + 5-attempt lockout + Redis rate limiter, SoundCloud `client_id` 24h auto-renewal + 401/403 retry, NextAuth OAuth `refresh_token` persistence & rotation)
  - Full Web/Vercel ↔ `zenne-api` contract parity (`/pub/auth/login`, `verify-2fa`, `oauth`, `game-token`, `/pub/account*`, `/pub/auth/recovery/*`, `/pub/game/create`, `/pub/ai/ask`, `/api/pending`, `/api/review`, `/api/review_bulk`, `DELETE /api/wordbank`, `/api/leaderboard`, `/api/guilds`)
  - `zenne-core/src/db.rs`: 12 secondary indexes, `submit_pending` resubmit for rejected words, `cache_add` hit_count increment, `lb_add_game`, `lb_streak_top`, cumulative `game_submit_score_mode`, `public_rooms` 3h auto-close + `get_public_room`, `tournament_*` methods
  - `src/main.rs`: isolated optional bot tasks so empty tokens do not terminate all bots/API
  - `zenne-discord`: fixed `mastermind` & `typing` tuple bugs and `danhsachtu` `(20, 0)` pagination, added `thamgiagiaidau`, `duyettu`, `connect4`, `uno`
  - `zenne-twitch` & `zenne-revolt`: added `taophong`, `phong`, `top`
  - `zenne-games`: expanded `trieuphu` to 15 distinct questions

---

## 2. Security Vulnerabilities Fixed
1. `zenne-api/src/ws_game.rs`: Enforces `verify_web_token` on WebSocket `"join"`, binds verified `uid` and `username`, rejects duplicate `user_id`.
2. `zenne-api/src/lib.rs` (`ah_pub_game_score`, `ah_pub_game_session_start`): Enforces `X-API-Key`, requires non-empty `session_token`, verifies `iat` elapsed time for `afk`, supports monotonic high-water score tracking per nonce.
3. `zenne-api/src/lib.rs` (`ah_pub_game_token`): Enforces `X-API-Key` / Bearer and reads `{ user_id, username }` from JSON body.
4. `zenne-api/src/auth_routes.rs`: Enforces `expires_at` (RFC3339 & epoch float) on registration OTP, recovery OTP, recovery token, `/recovery/status`, and password reset; enforces `attempts < 5` lockout, Redis/mem rate limiting, atomic `WHERE id = ?1 AND used_at IS NULL` consumption before password mutation, and 2FA/backup-code verification on password reset and `disable-2fa`.
5. `zenne-core/src/auth.rs`: Uses constant-time `Mac::verify_slice` in `verify_web_token` and `verify_game_session_claims` and encodes `iat` in `sign_game_session`.
6. `zenne-music/src/lib.rs`: Adds `get_soundcloud_client_id` automatic scraping & 24h cache + 401/403 retry.
7. `zenne-noitu-deploy-vercel/pages/api/auth/[...nextauth].js`: Persists `accessToken`, `refreshToken`, `accessTokenExpires` in JWT and refreshes Google/Discord/Top.gg OAuth tokens when expired.

---

## 3. Remaining Incomplete / Optional Items (Non-Blocking)
- `zenne-games`: `SudokuManager` (`battleship.rs`) and `UnoManager` (`memory.rs`) remain single-turn/minimal compared to full interactive Discord games; `2048`/`snake` are client-side web games.
- `zenne-discord/src/noitu.rs`: Multiplayer word-chain turn timeout and `!setnoitu` channel enforcement remain relaxed by design for casual chat; `listlang` search filter and `i18n.rs` extra UI locales (beyond `vi`/`en`) are optional.
- `zenne-api/src/ws_game.rs`: Server-side WS turn timeout timer (`time_limit`) is broadcast to clients (which run the UI countdown) but not yet enforced by a background WS timer task.

---

## 4. Tests & Build Verification (Actual Results)
- `cargo fmt --all` + `cargo check --workspace`: **Passed** (exit code `0`)
- `cargo test --workspace`: **15 passed, 0 failed** (`zenne-core`: 3, `zenne-api`: 1, `zenne-discord`: 1, `zenne-games`: 1, `zenne-music`: 9; all other crates compiled & passed)
- `npm run build` (`zenne-noitu-deploy-vercel`): **Passed** (exit code `0`, 35 static pages + all API routes)

---

## 5. Files Changed
- **Rust (`zenne-noitu-rs` on `migration/python-parity`):**
  - `src/main.rs`
  - `crates/zenne-core/src/{auth,cache,db}.rs`
  - `crates/zenne-music/src/lib.rs`
  - `crates/zenne-api/src/{lib,auth_routes,ws_game,pub_games}.rs`
  - `crates/zenne-discord/src/{lib,extra_cmds}.rs`
  - `crates/zenne-twitch/src/lib.rs`
  - `crates/zenne-revolt/src/lib.rs`
  - `crates/zenne-games/src/trieuphu.rs`
  - `docs/PYTHON_TO_RUST_{PARITY_AUDIT,COMMAND_MATRIX,SECURITY_AUDIT,API_COMPATIBILITY,MIGRATION_REPORT}.md`
- **Web (`zenne-noitu-deploy-vercel`):**
  - `pages/api/auth/[...nextauth].js`
