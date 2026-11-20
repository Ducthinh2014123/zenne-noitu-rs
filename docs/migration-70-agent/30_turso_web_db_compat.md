# Forensic Audit Report — ID 30

## 1. Direct Turso/libSQL Access vs Backend Proxy Routing

### A. Direct Turso HTTP Pipeline (`v2/pipeline`)
Only **1 library module** and **2 API routes** in `C:\Users\Ai Hoi\Downloads\zenne-noitu-deploy-vercel\` read/write Turso directly:
1. `lib\db.js` (`queryDb`, lines 10–61):
   - Sends raw SQL via `fetch(DB_URL, ...)` to Turso's `/v2/pipeline` endpoint.
   - Contains **hardcoded fallback credentials** (`TURSO_DB_URL` at line 4, `TURSO_DB_TOKEN` at line 5, a live `rw` JWT) used whenever `DATABASE_HTTP_URL` / `DATABASE_AUTH_TOKEN` are unset or match legacy filters (lines 7–8).
   - **Error swallowing**: On non-OK HTTP status (lines 37–41) or exceptions (lines 57–60), `queryDb` logs to console and returns `[]` without throwing.
2. `pages\api\pub\multiplayer\create.js` (`handler`, lines 4–62):
   - Imports `queryDb` (line 1) and executes a direct `INSERT INTO public_rooms (...) VALUES (...)` (lines 31–37).
   - Because `queryDb` returns `[]` on failure, `create.js` returns HTTP `200 { ok: true, ... }` (lines 39–57) even if the Turso `INSERT` failed.
3. `pages\api\pub\multiplayer\rooms.js` (`handler`, lines 3–41):
   - Imports `queryDb` (line 1) and executes a direct `SELECT ... FROM public_rooms WHERE status IN ('waiting', 'playing') ORDER BY created_at DESC LIMIT 30` (lines 16–22).

**Frontend Fallback Wiring**: `lib\pubApi.js` (`multiplayerRooms` lines 108–119, `createMultiplayerRoom` lines 121–136) calls the Rust backend (`/pub/multiplayer/rooms`, `/pub/multiplayer/create`) first and falls back to `/api/pub/multiplayer/rooms` and `/api/pub/multiplayer/create` on error.

### B. Rust/Python Backend Proxy Routes (`BOT_API_URL`)
All other DB-backed API routes proxy to `BOT_API_URL` (default `https://api.zenne-noitu.me` via `resolveBotApiUrl` in `lib\serverApi.js` lines 1–27):
- `pages\api\account\`: `change-password.js`, `disable-2fa.js`, `enable-2fa.js`, `info.js`, `setup-2fa.js`
- `pages\api\auth\`: `register.js`, `steam-callback.js`, `[...nextauth].js`, `register\email\resend.js`, `register\email\verify.js`, `recovery\*`
- `pages\api\pub\`: `game-session.js`, `game-score.js`, `game-token.js`, `game-ms.js`, `game-ttt.js`
- *(Non-DB exception: `pages\api\pub\ai-ask.js` lines 1–21 calls Kira AI `https://kiraai.vn/api/v1/chat/completions` directly.)*

---

## 2. Compatibility Verification vs `zenne-core/src/db.rs`

### A. Schema (`public_rooms`)
- **Reference (`zenne-noitu-rs\crates\zenne-core\src\db.rs`, lines 195–210)**:
  `room_id TEXT PRIMARY KEY`, `name TEXT NOT NULL`, `platform TEXT NOT NULL`, `host_id TEXT NOT NULL`, `host_name TEXT NOT NULL`, `lang TEXT DEFAULT 'vi'`, `max_players INTEGER DEFAULT 0`, `current_players INTEGER DEFAULT 1`, `join_token TEXT UNIQUE`, `web_url TEXT`, `tele_url TEXT`, `status TEXT DEFAULT 'waiting'`, `created_at TEXT DEFAULT (datetime('now','localtime'))`, `updated_at TEXT DEFAULT (datetime('now','localtime'))`.
- **Matches**: `create.js` (lines 32–36) inserts all 14 columns; `rooms.js` (line 17) selects 13 columns (omits `updated_at`).
- **Mismatches / Risks**:
  1. **No stale-room expiry on direct read**: `db.rs::get_public_rooms` (lines 927–933) closes rooms where `datetime(created_at, '+3 hours') < datetime('now','localtime')` before selecting `WHERE status != 'closed' ... LIMIT 50` (line 937). `rooms.js` (lines 16–22) skips the expiry `UPDATE` and queries `WHERE status IN ('waiting', 'playing') ... LIMIT 30`, so expired (>3h) rooms remain listed if the Rust backend hasn't swept them.
  2. **No `ON CONFLICT` upsert**: `db.rs::create_public_room` (lines 992–1002) uses `ON CONFLICT(room_id) DO UPDATE`, whereas `create.js` (lines 31–37) uses a plain `INSERT` (which fails silently via `queryDb` on collision).
  3. **Integer parsing quirk**: `rooms.js` line 28 does `parseInt(r.current_players) || 1`, which coerces `0` to `1`.

### B. Timestamps
- Both `db.rs` (lines 208–209, 929–930, 1002) and `create.js` (line 35) use SQLite `datetime('now','localtime')`. Format is compatible (`YYYY-MM-DD HH:MM:SS` in Turso's server localtime/UTC).

### C. IDs (`room_id`, `host_id`)
- **`room_id`**:
  - `create.js` line 22 generates 6-char uppercase hex (`crypto.randomBytes(3).toString('hex').toUpperCase()`), matching `zenne-telegram\src\state.rs::gen_room_code` (lines 34–38).
  - `zenne-api\src\lib.rs::ah_pub_create_room` (line 768) generates 8-char uppercase UUID prefix (`uuid::Uuid::new_v4().toString()[..8].to_uppercase()`). Both are valid `TEXT PRIMARY KEY` values, though lengths differ (6 vs 8).
- **`host_id`**:
  - `create.js` line 35 hardcodes `'web_user'` (ignoring any NextAuth `session.user.id`), whereas `ah_pub_create_room` (`zenne-api\src\lib.rs` line 775) defaults to `"0"`, and `game-session.js`/`game-score.js` use `-Number(session.user.id)`.

### D. Tokens (`join_token`, `tele_url`)
- **`join_token`**:
  - `create.js` line 23 generates 16-char lowercase hex (`crypto.randomBytes(8).toString('hex')`), matching `zenne-telegram\src\state.rs::gen_join_token` (lines 40–44).
  - `zenne-api\src\lib.rs::ah_pub_create_room` (line 769) generates a 36-char UUID v4.
- **Broken `tele_url` / `telegram_room_tokens` mismatch**:
  - `create.js` (lines 29, 35) and `rooms.js` (line 30) populate `tele_url` as `https://zenne-noitu.vercel.app/telegram/?join=${join_token}`, which `pages\telegram\index.jsx` (line 14) redirects to `https://t.me/zenne_noitu_bot?start=join_${token}`.
  - However, `create.js` **never** writes to `telegram_room_tokens` (`db.rs` lines 211–219, `save_telegram_token` lines 1044–1061), and `zenne-api\src\lib.rs::ah_pub_create_room` (line 782) intentionally sets `tele_url: None` for web rooms (no Telegram `chat_id` exists). Consequently, `tele_url` links created by `create.js` / `rooms.js` point to a `join_token` absent from `telegram_room_tokens`.
