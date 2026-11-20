# Forensic Audit Report — ID 19

**Scope:** HTTP Game & Room endpoints parity audit across Python (`gdrive-bot-source/gdrive-bot/bot.py`), Rust (`zenne-noitu-rs/crates/zenne-api/src/{lib.rs,pub_games.rs}` + `zenne-core/src/{auth.rs,db.rs}`), and Web callers (`zenne-noitu-deploy-vercel`).

---

## 1. Authentication, Session Tokens, Nonce & Replay Protection

| Mechanism | Python (`bot.py`) | Rust (`zenne-api` / `zenne-core`) | Web Caller (`zenne-noitu-deploy-vercel`) | Parity Status |
|---|---|---|---|---|
| **Admin API Key (`X-API-Key`)** | `_chk(req)` (`bot.py:4175-4183`) checks `X-API-Key` header or `?api_key=` query param. API server disabled if `API_KEY` is empty (`bot.py:6293`). | `check_admin_auth` (`lib.rs:30-44`) checks `Authorization: Bearer` or `X-API-Key` header (no query param). If `api_key` is empty, auth is bypassed (`lib.rs:460,585`). | `botApi` (`lib/serverApi.js:9-27`) sends `X-API-Key: BOT_ADMIN_API_KEY`. | **Partial** (empty key bypass in Rust; no query param). |
| **TTT & MS Auth** | `_ah_pub_game_ttt` (`bot.py:1585`) & `_ah_pub_game_ms` (`bot.py:1675`) enforce `_chk(req)`. | `ah_pub_game_ttt` (`lib.rs:726-738`) & `ah_pub_game_ms` (`lib.rs:740-752`) **do NOT call `check_admin_auth`**. | `pages/api/pub/game-ttt.js` & `game-ms.js` require NextAuth session, then call `botApi`. | **CRITICAL GAP** — Rust `/pub/game/ttt` and `/pub/game/ms` are unauthenticated and accept arbitrary `user_id`. |
| **`session_token` Signing & Format** | `_sign_game_session` (`bot.py:133-142`): base64url(JSON `{uid, game, iat, nonce, exp}`) + HMAC-SHA256 hex. TTL 3600s. | `sign_game_session` (`auth.rs:39-49`): base64url(`"{uid}:{game}:{iat}:{exp}:{nonce}"`) + HMAC-SHA256 hex. TTL 3600s. | `pages/api/pub/game-session.js:21-29` proxies `POST /pub/game/session/start`. | **Incompatible format** between Python and Rust (cannot validate tokens across runtimes). |
| **Monotonic Nonce & Replay** | `_USED_GAME_SESSION_SCORES` (`bot.py:160,4501-4511`): in-memory `dict[nonce -> last_score]`; clears all at `>20,000` entries. Rejects `score <= last_score` (returns 200 `{ok:true}` without DB write); for `afk`, submits `delta = score - last_score`. | `ApiState.used_nonces` (`lib.rs:27,504-513`): in-memory `DashMap<String, i64>`, **unbounded** (never evicted). Rejects `score <= prev` (returns 200 `{ok:true}`); for `afk`, submits `score - prev`. | `pages/pub/{2048,snake,afk,quiz}.jsx` reuse `session_token` across periodic/end-of-game submissions. | **Partial** (Rust `used_nonces` leaks memory unboundedly; neither persists nonces across restarts). |

---

## 2. Request / Response Schemas & Validation Gaps

1. **`POST /pub/game/session/start`**
   - **Python** (`_ah_pub_game_session_start`, `bot.py:4516-4534`): Requires `game` in `_RATE_LIMITED_GAMES` (`bot.py:161-168`, 29 games, lowercased) and valid int `user_id`. Returns `{"token": ...}`.
   - **Rust** (`ah_pub_game_session_start`, `lib.rs:580-596`): **Does not validate `game`** against `_RATE_LIMITED_GAMES` and does not lowercase `game`. `user_id` is `Option<i64>` defaulting to `0` (`lib.rs:592`). Returns `{"token": ...}`.
   - **Web** (`pages/api/pub/game-session.js:5`): Restricts `ALLOWED_GAMES` to `['snake', '2048', 'afk', 'quiz']`.

2. **`POST /pub/game/score`**
   - **Python** (`_ah_pub_game_score`, `bot.py:4469-4513`): Enforces `game in _RATE_LIMITED_GAMES`, clamps `score` to `[0, 10_000_000]`, returns `400` on invalid/missing `session_token`.
   - **Rust** (`ah_pub_game_score`, `lib.rs:455-520`): **No `_RATE_LIMITED_GAMES` allowlist**; returns `401` on invalid `session_token` (vs `400` in Python).

3. **`GET /pub/games`** (bonus parity check)
   - **Python** (`bot.py:4537-4568`): Returns 29 games (includes `snake`).
   - **Rust** (`lib.rs:413-445`): Returns 28 games (**missing `snake`**).

4. **`POST /pub/game/ttt` & `POST /pub/game/ms`**
   - **Schema parity is high**: `{ user_id, action, index }` -> TTT `{ board, result, streak }`, MS `{ cols, rows, mines }` / `{ result, revealed, mines?, elapsed?, score? }`.
   - Both store active game sessions in-memory (`_TTT_SESSIONS` / `_MS_SESSIONS` in `bot.py:1534,1637`; `TttSessionsMap` / `MsSessionsMap` in `pub_games.rs:39-40`), and persist wins directly to DB (`tictactoe` streak, `minesweeper` score `max(0, 1000 - int(elapsed * 3))`).

---

## 3. Anti-Cheat Bounds (`_sanity_check_score` vs `sanity_check_score`)

| Game / Check | Python (`bot.py:171-209`) | Rust (`pub_games.rs:337-348` & `lib.rs:494-501`) |
|---|---|---|
| **Min play time** | `elapsed < 0.2` -> **400 reject** | **Missing** |
| **`2048`** | `score % 4 == 0`, `<= 200_000`, `<= elapsed * 500 + 500` | `<= 500_000` only (**missing `% 4` & rate check**) |
| **`snake`** | `score % 10 == 0`, `<= 5_000`, `<= elapsed * 100 + 100` | `<= 10_000` only (**missing `% 10` & rate check**) |
| **`afk`** | `score <= elapsed + 15` | `score <= elapsed + 60` (`lib.rs:495`) & `<= 1_000_000` |
| **`quiz`** | `score <= elapsed * 80 + 500` | `<= 1_000_000` (**no rate check**) |
| **`vuatiengviet`, `duoihinh`** | `<= 100` | `<= 1_000_000` |
| **`wordle`** | `<= 120` | `<= 1_000_000` |
| **`typing`** | `<= 350` | `<= 1_000_000` |
| **`trieuphu`** | `<= 150_000_000` | `<= 1_000_000` (**blocks valid >1M rewards!**) |

---

## 4. Room Creation, Listing, Joining & DB Persistence

1. **`POST /pub/game/create`**
   - **Python** (`_ah_pub_create_room`, `bot.py:4872-4881`): Accepts `{lang, time_limit}` (clamped `10..120`, default `12`), creates in-memory `WebRoom` only (no DB), returns `{"room_id", "lang", "time_limit", "url"}`.
   - **Rust** (`ah_pub_create_room`, `lib.rs:764-787`): Persists to `public_rooms` DB (does not pre-create `ws_rooms`), ignores `time_limit`, returns `{"ok": true, "room_id", "room"}` (missing top-level `url` and `time_limit`).
2. **`POST /pub/multiplayer/create`**
   - **Python** (`_ah_pub_mp_create`, `bot.py:6237-6277`): 6-char hex `room_id`, 16-char hex `join_token`, `host_id="web"`, creates `WebRoom` + DB `public_rooms`, returns `{"ok": true, "room": {...}}` (no top-level `room_id`).
   - **Rust** (`ah_pub_mp_create` -> `ah_pub_create_room`, `lib.rs:764-794`): 8-char UUID prefix `room_id`, 36-char UUID `join_token`, `host_id="0"`, DB `public_rooms` only, returns `{"ok": true, "room_id", "room"}`.
   - **Web** (`lib/pubApi.js:121-136`): Calls Rust/Python `/pub/multiplayer/create` directly from browser; falls back to `pages/api/pub/multiplayer/create.js` which writes directly to Turso `public_rooms` (`host_id='web_user'`).
3. **`GET /pub/multiplayer/rooms`**
   - **Python** (`_ah_pub_mp_rooms`, `bot.py:6233-6235` & `1472-1487`): Expires rooms `>3h`, queries `status IN ('waiting','playing') LIMIT 30` (includes `created_at`), returns `{"ok": true, "rooms": [...]}`.
   - **Rust** (`ah_pub_mp_rooms`, `lib.rs:537-540` & `db.rs:925-960`): Expires rooms `>3h`, queries `status != 'closed' LIMIT 50` (omits `created_at`), returns `{"ok": true, "rooms": [...], "data": [...]}`.
4. **`GET /pub/multiplayer/token/{token}`**
   - **Python** (`_ah_pub_mp_token_info`, `bot.py:6279-6289`): Checks `telegram_room_tokens` first, then `public_rooms.join_token`; returns `{"ok": true, "room_code", "name"|"host_name", "lang"}`.
   - **Rust** (`ah_pub_mp_token_info`, `lib.rs:796-813`): **Does not check `telegram_room_tokens`**; scans `get_public_rooms()` (misses closed/older rooms) by `join_token` OR `room_id`, and returns `{"ok": true, "room": r}` (**schema mismatch**: lacks top-level `room_code`, `lang`, `name`/`host_name`).
5. **DB Persistence (`game_scores` & `public_rooms`)**
   - `db_game_submit_score` (`bot.py:1397-1424`) and `game_submit_score_mode` (`db.rs:658-683`) have **full SQL parity**: `afk` accumulates (`best_score + excluded.best_score`), all other games keep `MAX(best_score, excluded.best_score)` and increment `plays`.

---

## 5. Recommended Tests

1. **Auth Regression (`zenne-api`)**: Verify `POST /pub/game/ttt` and `POST /pub/game/ms` return `401 Unauthorized` when `X-API-Key` is missing/invalid (once `check_admin_auth` is added).
2. **Anti-Cheat Parity (`pub_games.rs` / `lib.rs`)**:
   - `POST /pub/game/score` with `elapsed < 0.2` -> `400`.
   - `2048` with `score = 10` (`% 4 != 0`), `score = 250_000` (`> 200_000`), or `score = 4000` at `elapsed = 1s` -> `400`.
   - `snake` with `score = 15` (`% 10 != 0`), `score = 6000` (`> 5000`), or `score = 1000` at `elapsed = 1s` -> `400`.
   - `trieuphu` with `score = 150_000_000` -> `200` (currently rejected by `1_000_000` default cap).
   - `afk` monotonic delta: submit `30` then `50` with same `session_token` -> DB `best_score` equals `50`, not `80`.
3. **Token & Room Schema Parity**:
   - `GET /pub/multiplayer/token/{tg_token}` resolves a row from `telegram_room_tokens` and returns `room_code`, `lang`, `host_name`.
   - `POST /pub/game/create` respects `time_limit` (clamped `10..120`) and returns `url`.
   - `GET /pub/games` includes `snake` (29 games total).
