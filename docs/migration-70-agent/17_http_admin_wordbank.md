# Forensic Audit Report — ID 17

**Scope**: HTTP Admin & Wordbank endpoints parity audit across:
- **Python**: `C:\Users\Ai Hoi\Downloads\gdrive-bot-source\gdrive-bot\bot.py`
- **Rust**: `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-api\src\lib.rs` & `crates\zenne-core\src\db.rs`, `config.rs`, `lib.rs`
- **Web Callers**: `C:\Users\Ai Hoi\Downloads\zenne-noitu-deploy-vercel\lib\api.js`, `lib\serverApi.js`, `pages\pending.jsx`, `pages\wordbank.jsx`, `pages\leaderboard.jsx`, `pages\servers.jsx`

---

## 1. Auth Headers & API Key Validation

| Aspect | Python (`bot.py`) | Rust (`zenne-api/src/lib.rs`) | Web Callers |
|---|---|---|---|
| **Validator** | `_chk(req)` (`bot.py:4175-4183`) | `check_admin_auth(headers, api_key)` (`lib.rs:30-44`) | `lib/api.js:13`, `lib/serverApi.js:16`, `pages/pending.jsx:20` |
| **Accepted inputs** | `X-API-Key` header **or** `?api_key=` query param | `Authorization: Bearer <key>` **or** `X-API-Key` header (no query param) | Sends `X-API-Key` header only |
| **Comparison** | `key != API_KEY` (`bot.py:4178`) — **NOT constant-time** | `tok == api_key`, `k == api_key` (`lib.rs:33,39`) — **NOT constant-time** | — |
| **Empty `API_KEY`** | `start_web_api()` disables server if `not API_KEY` (`bot.py:6293`) | `check_admin_auth` does not guard `api_key.is_empty()`; if `API_KEY=""`, `X-API-Key: ` passes (`"" == ""`) | — |
| **Failure status/body** | `401` `{"error": "Invalid API key (BOT_ADMIN_API_KEY khong khop API_KEY cua bot)"}` | `401` `{"error": "Unauthorized"}` | `pending.jsx:21` checks `401 \|\| 403` |
| **CORS headers** | `_cors_mw` (`bot.py:4169-4171`): `Allow-Headers: Content-Type, X-API-Key` (blocks `Authorization` preflight) | `CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any)` (`lib.rs:54-57`) | — |

---

## 2. Endpoint-by-Endpoint Parity Matrix

### `GET /health` (Rust also exposes `GET /api/health`)
- **Auth**: Public in both (`bot.py:4199,6298`; `lib.rs:60-61,164`).
- **Schema drift**:
  - Python (`_ah_health`, `bot.py:4200`): `{"status": "ok", "bot": str(bot.user)}`.
  - Rust (`ah_health`, `lib.rs:165`): `{"status": "ok", "engine": "rust-axum"}` (drops `bot`, adds `engine`).

### `GET /api/stats`
- **Auth**: Admin auth in both (`bot.py:4204`; `lib.rs:169`).
- **Schema drift**:
  - Python (`_ah_stats`, `bot.py:4206-4213`): `{"pending", "approved", "rejected", "wordbank", "cache", "guilds"}`.
  - Rust (`ah_stats`, `lib.rs:176-177` -> `stats_summary`, `db.rs:706-738`): `{"word_bank", "wordbank", "approved", "rejected", "pending_words", "wiki_cache", "total_players", "games_played", "total_games_words"}`.
  - **Mismatch**: Rust renames `pending` -> `pending_words`, `cache` -> `wiki_cache`, and **omits `guilds`**.

### `GET /api/pending`
- **Auth**: Admin auth in both (`bot.py:4217`; `lib.rs:185`).
- **Pagination/Filtering drift**:
  - Python (`_ah_pending`, `bot.py:4216-4236`): supports `status` (default `"pending"`), `limit` (default `50`, max `200`), `offset` (default `0`), `q` (`LIKE`), `guild_id`. Orders `submitted_at DESC`. Returns `{"data": [...], "total": N, "limit": limit, "offset": offset}`.
  - Rust (`ah_pending`, `lib.rs:180-196` -> `get_pending`, `db.rs:400-429`): supports `guild_id`, `status` (default `"pending"`), **ignores `limit`, `offset`, `q`** (hardcoded `ORDER BY id DESC LIMIT 100`). Returns `{"items": rows, "data": rows}` (omits `total`, `limit`, `offset`; `PendingWord` at `db.rs:8-15` omits `reviewed_by`, `reviewed_at`).
- **Web caller impact**: `lib/api.js:20` calls `/api/pending` without params; `pages/pending.jsx:78-84` filters `approved`/`rejected` tabs client-side, which always yields 0 items because both backends default to `status=pending`.

### `POST /api/review`
- **Auth**: Admin auth in both (`bot.py:4240`; `lib.rs:211`).
- **CRITICAL Request Schema Bug**:
  - `lib/api.js:21` sends `{ id, action }` (`"approve"` or `"reject"`).
  - **Python** (`_ah_review`, `bot.py:4247`): reads `body.get("approve", True)` and **ignores `action`** — rejecting a word from the Web UI silently **approves** it in Python!
  - **Rust** (`ah_review`, `lib.rs:218-220`): checks `body.approve.unwrap_or_else(|| body.action.as_deref() == Some("approve"))` — handles both.
  - `reviewer_id` default: `SUPER_ADMIN` (`bot.py:4248`) vs `0` (`lib.rs:224`).
- **Status Codes & Response**:
  - Python: `400` missing `id`, `404` not found, `409` already reviewed (`{"error": "Already reviewed", "status": ...}`), `200` `{"ok": true, "status": ns, "word": ...}`.
  - Rust: always `200` `{"ok": true|false}` (no `404`/`409`; invalid JSON/missing `id` yields Axum `422`/`400`).

### `POST /api/review/bulk` & `POST /api/review_bulk`
- **CRITICAL Route Mismatch**:
  - `lib/api.js:22` calls `POST /api/review_bulk`.
  - **Python** (`bot.py:6302`): registers **only `/api/review/bulk`** -> Web UI bulk review **404s** on Python.
  - **Rust** (`lib.rs:65-66`): registers both `/api/review/bulk` and `/api/review_bulk`.
- **Same `action` bug in Python** (`bot.py:4275` ignores `action`, defaults `approve=True`).
- **Response**: Python returns `{"ok": true, "processed": N, "status": ns}`; Rust (`lib.rs:651`) omits `status`.

### `GET /api/wordbank` & `DELETE /api/wordbank`
- **GET Pagination/Filtering**:
  - Python (`_ah_wordbank_get`, `bot.py:4294-4313`): supports `guild_id`, `q`, `limit` (default `100`, max `500`), `offset` (`0`). Returns `SELECT *` + `total, limit, offset`.
  - Rust (`ah_wordbank_get`, `lib.rs:654-678` -> `pub_wordbank`, `db.rs:740-792`): **ignores `guild_id`**, caps `limit` at `200` (default `50`), returns only `{word, added_at}`, and omits `limit`/`offset` in response.
- **CRITICAL `DELETE /api/wordbank` Mismatch**:
  - `lib/api.js:24` sends `DELETE /api/wordbank` with `{ word }` (no `guild_id`).
  - **Python** (`_ah_wordbank_del`, `bot.py:4323-4326`): requires both `word` and `guild_id` -> **always returns `400`** to Web UI.
  - **Rust** (`ah_wordbank_del`, `lib.rs:690-712`): accepts `?word=` or body `{ word }`, ignores `guild_id`, deletes across all guilds, and returns `200 {"ok": true}` even if `word` is missing.

### `GET /api/leaderboard`
- **CRITICAL Auth Mismatch**:
  - Python (`_ah_leaderboard`, `bot.py:4334`): **requires admin auth** (`_chk`).
  - Rust (`ah_leaderboard`, `lib.rs:229-239`): **no auth check** (publicly exposed).
- **Filtering/Pagination Drift**:
  - Python (`bot.py:4335-4346`): optional `guild_id` (omitted = all guilds), `mode` (`"streak"`/`"words"`), `limit` (default `20`, max `100`).
  - Rust (`lib.rs:233-238`): `guild_id` defaults to `0` (omitting `guild_id` restricts to web-only `guild_id=0`), **ignores `mode` and `limit`** (hardcoded `20`).
- **Web caller drift**: `pages/leaderboard.jsx:32,35` reads `p.username`, which neither Python nor Rust returns (both return `user_id` only).

### `GET /api/guilds`
- **Auth**: Admin auth in both (`bot.py:4350`; `lib.rs:715`).
- **CRITICAL Data Source & Schema Mismatch**:
  - Python (`_ah_guilds`, `bot.py:4349-4353`): queries `guild_settings` table -> `{"data": [{"guild_id", "lang", "game_channel_id"}]}`.
  - Rust (`ah_guilds`, `lib.rs:714-724`): reads `live.guilds` (`GuildSummary`, `zenne-core/src/lib.rs:18-23`) -> `{"guilds": ..., "data": [{"id", "name", "member_count", "icon"}]}`.
  - `pages/servers.jsx:22-25` expects `guild_id`, `name`, and `lang` — Python lacks `name`; Rust lacks `guild_id` and `lang`.

---

## 3. Error Handling Gaps
- **Python (`bot.py:4164-4166`)**: Unhandled exceptions in `_cors_mw` return `500` with `{"error": str(ex), "trace": _tb.format_exc()}` — **leaks stack traces**. Non-integer query params (`limit`, `offset`, `guild_id` in `_ah_pending`, `_ah_wordbank_get`, `_ah_leaderboard`) raise `ValueError` and trigger this `500` trace leak.
- **Rust (`zenne-api` / `db.rs`)**: Invalid integer query params silently fall back to defaults via `.parse::<i64>().ok()`; DB errors in `db.rs` are swallowed (`unwrap_or_default` / `vec![]`) and return `200 OK` with empty data rather than `500`.

---

## 4. Recommended Tests
1. **Constant-time Auth & Edge-case Tests**:
   - Verify `check_admin_auth` (Rust) and `_chk` (Python) use constant-time comparison (`subtle::ConstantTimeEq` / `hmac.compare_digest`) and reject empty `API_KEY` instead of matching `"" == ""`.
   - Verify `GET /api/leaderboard` enforces admin auth in Rust (currently 200 without key).
2. **Web-to-Backend Contract Tests (Python & Rust)**:
   - `POST /api/review` with `{ id, action: "reject" }` -> assert DB status becomes `rejected` (catches Python `approve=True` bug) and `404`/`409` on missing/already-reviewed IDs.
   - `POST /api/review_bulk` -> assert `200` on both `/api/review/bulk` and `/api/review_bulk` (catches Python 404).
   - `DELETE /api/wordbank` with `{ word }` only -> assert expected `200` or `400` parity and non-empty `word` validation.
3. **Pagination, Filtering & Schema Parity Tests**:
   - `GET /api/pending?q=foo&limit=10&offset=5&status=rejected` -> assert Rust honors `q`, `limit`, `offset` and returns `total`, `limit`, `offset`.
   - `GET /api/stats` -> assert `guilds` key is present in Rust.
   - `GET /api/leaderboard?mode=streak&limit=50` -> assert Rust honors `mode`, `limit`, and optional `guild_id`.
   - `GET /api/guilds` -> assert consistent fields (`guild_id`, `name`, `lang`) matching `pages/servers.jsx`.
4. **Malformed Input Fuzzing**:
   - Send `limit=abc&offset=xyz&guild_id=nan` to Python admin endpoints -> assert `400` JSON error without Python traceback leakage.
