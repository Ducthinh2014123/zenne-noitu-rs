# Forensic Audit Report — ID 55

**Scope:** `crates/zenne-api/src/{lib.rs,auth_routes.rs,ws_game.rs,pub_games.rs}` (with supporting checks in `crates/zenne-core/src/{auth.rs,cache.rs,config.rs}`).

---

### 1. Missing `check_admin_auth` on `/api/leaderboard` and `/pub/game/{ttt,ms}`
- **`/api/leaderboard` (`zenne-api/src/lib.rs:71`, `ah_leaderboard` at `lib.rs:229-239`):** Does not accept `HeaderMap` and never calls `check_admin_auth`.
- **`/pub/game/ttt` (`lib.rs:84`, `ah_pub_game_ttt` at `lib.rs:726-738` -> `pub_games::handle_ttt` at `pub_games.rs:125-202`):** Unauthenticated; trusts caller-supplied `user_id` directly from JSON and writes win-streak scores to DB.
- **`/pub/game/ms` (`lib.rs:85`, `ah_pub_game_ms` at `lib.rs:740-752` -> `pub_games::handle_ms` at `pub_games.rs:251-335`):** Unauthenticated; trusts arbitrary `user_id` from JSON and submits Minesweeper scores to DB.

---

### 2. Empty `api_key` Authentication Bypass
- **Fail-open route guards (`zenne-api/src/lib.rs:460`, `lib.rs:585`):** `ah_pub_game_score` and `ah_pub_game_session_start` guard admin-only access with `if !st.ctx.config.api_key.is_empty() && !check_admin_auth(&headers, &st.ctx.config.api_key)` — failing open when `api_key` is empty.
- **`check_admin_auth` empty-string match (`lib.rs:30-44`):** Does not reject an empty `api_key`, allowing `X-API-Key: ` / `Authorization: Bearer ` to match `"" == ""`.

---

### 3. IP vs. Identifier Rate Limiting
- `start_web_api` (`lib.rs:159`) runs `axum::serve` without `ConnectInfo<SocketAddr>`. No IP-based rate limiting exists anywhere in `zenne-api`.
- All rate limits in `auth_routes.rs` (`login:`, `reg:`, `reg_verify:`, `reg_resend:`, `rec_req:`, `rec_ev:`) are identifier-only — enabling bypass via identifier rotation and targeted account DoS/lockout.

---

### 4. Unthrottled `/pub/ai/ask` & `/pub/auth/verify-2fa`
- **`/pub/ai/ask` (`lib.rs:90, 136, 555-572`):** Unauthenticated and unthrottled AI proxy.
- **`/pub/auth/verify-2fa` (`lib.rs:103, 852-910`):** Unthrottled **and** accepts `user_id`/`username` + `code` without password or pre-auth 2FA challenge token, minting a 7-day web token directly.

---

### 5. WebSocket `uid=0` Fallback
- `handle_ws_connection` `"join"` (`ws_game.rs:223, 244, 255-256, 456`) defaults missing/non-int `"uid"` to `0`, bypassing duplicate-user checks (`token_uid != 0`) and skipping DB leaderboard writes (`p_uid != 0`) while allowing `uid=0` players to join and become room host.
