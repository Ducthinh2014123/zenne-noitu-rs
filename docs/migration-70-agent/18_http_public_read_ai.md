# Forensic Audit Report — ID 18
**Scope:** HTTP Public Read & AI endpoints parity audit (`/pub/status`, `/pub/wordbank`, `/pub/rejected`, `/pub/servers`, `/pub/milestones`, `/pub/leaderboard`, `/pub/games`, `/pub/game/leaderboard`, `/pub/ai/ask`) across:
- **Python (`bot.py`)**: `C:\Users\Ai Hoi\Downloads\gdrive-bot-source\gdrive-bot\bot.py`
- **Rust (`zenne-api` / `zenne-core`)**: `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-api\src\lib.rs`, `zenne-core\src\db.rs`, `zenne-core\src\kira.rs`, `zenne-core\src\lib.rs`
- **Web (`zenne-noitu-deploy-vercel`)**: `lib/pubApi.js`, `pages/pub/*.jsx`, `pages/api/pub/ai-ask.js`

---

## 1. Cross-Cutting Concerns (CORS, Caching, Rate-Limiting)

| Aspect | Python (`bot.py`) | Rust (`zenne-api`) | Web (`zenne-noitu-deploy-vercel`) | Parity Status |
|---|---|---|---|---|
| **CORS** | `_cors_mw` (`bot.py:4153-4172`): `Access-Control-Allow-Origin: *`, `Methods: GET, POST, DELETE, OPTIONS`, `Headers: Content-Type, X-API-Key`. Handles `OPTIONS` (200), `HTTPException`, and unhandled `Exception` (500 JSON). | `CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any)` (`lib.rs:54-57`). | Browser calls `DEFAULT_API_URL` directly via `fetch` (`pubApi.js:26-39`). | **Compatible** (Rust is slightly more permissive on methods/headers). |
| **HTTP Caching** | None (`Cache-Control` not set on any `/pub/*` route). | None (`Cache-Control` not set). | Standard `fetch` without custom cache directives. | **Parity** (no HTTP caching). |
| **In-Memory / Redis Cache** | None on these 9 routes (`_ah_pub_leaderboard` & `_ah_pub_game_leaderboard` call `bot.fetch_user(uid)` per row). | `enrich_discord_users` (`lib.rs:332-396`) caches Discord user profiles in `dc_user:{uid}` for `3600s`. | None. | **Intentional improvement in Rust** (avoids Discord API rate limits). |
| **Rate-Limiting** | **None** on any of these 9 endpoints (`_rate_limited` at `bot.py:5304` is only used on `/pub/auth/*`). | **None** on any of these 9 endpoints (`cache.rate_limited` is only used in `auth_routes.rs`). | **None** in `pages/api/pub/ai-ask.js`. | **Parity gap (Security)**: `/pub/ai/ask` and `/api/pub/ai-ask` are unauthenticated & unthrottled AI proxies. |
| **Route Aliases** | Only `/pub/*` (`bot.py:6307-6323`). | `/pub/*` + `/api/pub/{status,leaderboard,games,game/leaderboard,ai/ask}` (`lib.rs:129-136`). | Uses `/pub/*` (`pubApi.js:138-145`) and `/api/pub/ai-ask` (`pubApi.js:95`). | **Compatible**. |

---

## 2. Endpoint-by-Endpoint Parity Matrix

### 2.1 `GET /pub/status`
- **Handlers**: Python `_ah_pub_status` (`bot.py:4358-4378`) | Rust `ah_pub_status` (`lib.rs:241-253`, `db.rs:706-738`) | Web `pages/pub/index.jsx:37, 62-75`
- **Query Params**: None.
- **Response JSON**:
  - Python: `{ online: bool, latency_ms: float|null, guild_count: int, user_count: int, stats: { approved: int, rejected: int, wordbank: int, total_games_words: int } }`
  - Rust: Adds `status: "online"|"offline"`, `runtime: "Rust (Tokio + Axum)"`, and extra `stats` keys (`word_bank`, `pending_words`, `wiki_cache`, `total_players`, `games_played`).
  - Web reads: `online`, `latency_ms`, `guild_count`, `user_count`, `stats.wordbank`, `stats.total_games_words`.
- **Parity**: **Pass** (superset in Rust; Web unaffected).

### 2.2 `GET /pub/wordbank`
- **Handlers**: Python `_ah_pub_wordbank` (`bot.py:4380-4390`) | Rust `ah_pub_wordbank` (`lib.rs:255-271`, `db.rs:740-792`) | Web `pages/pub/wordbank.jsx:18-19, 48-49`
- **Query Params**: `q` (default `""`), `limit` (default `50`, max `200`), `offset` (default `0`).
- **Response JSON**: `{ data: [{ word: str, added_at: str }], total: int }`.
  - Minor difference: Python returns `added_at: null` if SQL NULL; Rust uses `COALESCE(added_at,'')` returning `""`.
- **Error Status Codes**:
  - Python: `int(qm.get("limit", 50))` raises `ValueError` on non-numeric `limit`/`offset` -> **500** JSON via `_cors_mw`.
  - Rust: `.parse::<i64>().ok().unwrap_or(...)` silently falls back to defaults (**200**).
- **Parity**: **Minor drift** (invalid query param handling & NULL vs `""`).

### 2.3 `GET /pub/rejected`
- **Handlers**: Python `_ah_pub_rejected` (`bot.py:4392-4403`) | Rust `ah_pub_rejected` (`lib.rs:273-289`, `db.rs:794-850`) | Web `pages/pub/rejected.jsx:18-19, 45-46`
- **Query Params**: `q` (default `""`), `limit` (default `50`, max `200`), `offset` (default `0`).
- **Response JSON**: `{ data: [{ word: str, submitted_at: str }], total: int }` (Rust `COALESCE(submitted_at,'')` vs Python `null`).
- **Error Status Codes**: Same as `/pub/wordbank` (Python 500 vs Rust fallback 200 on non-numeric `limit`/`offset`).
- **Parity**: **Minor drift**.

### 2.4 `GET /pub/servers`
- **Handlers**: Python `_ah_pub_servers` (`bot.py:4436-4442`) | Rust `ah_pub_servers` (`lib.rs:291-294`, `zenne-core/src/lib.rs:17-23`) | Web `pages/pub/servers.jsx:12, 19, 26-42`
- **Query Params**: None.
- **Response JSON**: `{ data: [{ id: str, name: str, member_count: int, icon: str|null }], total: int }`.
- **Parity**: **Pass** (assuming `live.guilds` in Rust is sorted desc & capped at 50 by the Discord gateway task).

### 2.5 `GET /pub/milestones`
- **Handlers**: Python `_ah_pub_milestones` (`bot.py:4444-4466`) | Rust `ah_pub_milestones` (`lib.rs:296-330`) | Web `pages/pub/milestones.jsx:12, 23-49`, `index.jsx:38, 43-44`
- **Query Params**: None.
- **Response JSON**: `{ data: [{ key, label, emoji, type, target, current, reached, pct }], current: { servers, wordbank, games, users } }`.
- **Behavioral Drift**: When `live.user_count == 0`, Rust falls back to `summary["total_players"]` (`lib.rs:300-304`), whereas Python returns `0`.
- **Parity**: **Pass** (minor offline fallback difference).

### 2.6 `GET /pub/leaderboard`
- **Handlers**: Python `_ah_pub_leaderboard` (`bot.py:4405-4434`) | Rust `ah_pub_leaderboard` (`lib.rs:398-411`, `db.rs:852-876`) | Web `pages/pub/leaderboard.jsx:45, 72-103`
- **Query Params**: `limit` (default `20`, max `100`), `mode` (default `"words"`, `"streak"` or `"words"`).
- **Response JSON**: `{ data: [{ user_id: str, name: str, avatar: str|null, total_words: int, max_streak: int }] }`.
- **Error Status Codes**: Python **500** on non-numeric `limit` vs Rust **200** fallback.
- **Parity**: **Pass** (minor query-parse drift).

### 2.7 `GET /pub/games`
- **Handlers**: Python `_ah_pub_games` (`bot.py:4537-4570`) | Rust `ah_pub_games` (`lib.rs:413-445`) | Web: not called (`pages/pub/games.jsx` & `leaderboard.jsx` hardcode the list).
- **Response JSON**: `{ data: [{ id, name, icon, type, unit }], total: int }`.
- **Divergence**:
  - Python returns **29** games (`total: 29`), starting with `{"id": "snake", ...}` (`bot.py:4540`).
  - Rust returns **28** games (`total: 28`) — **`"snake"` is missing** from `ah_pub_games` (`lib.rs:414-444`), even though `pub_games::sanity_check_score` (`pub_games.rs:343`) supports `"snake"`.

### 2.8 `GET /pub/game/leaderboard`
- **Handlers**: Python `_ah_pub_game_leaderboard` (`bot.py:4573-4600`) | Rust `ah_pub_game_leaderboard` (`lib.rs:522-535`, `db.rs:878-903`) | Web `pages/pub/leaderboard.jsx:44`, `pages/pub/afk.jsx:45`
- **Query Params**: `game`, `limit` (default `20`, max `100`).
- **Divergence**:
  1. **Validation & Error Status**: Python validates `game in _GAMES` (`bot.py:4578-4579`) and returns **400** `{"error": "Game khong hop le"}` if missing/invalid. Rust defaults missing `game` to `"noitu"` (`lib.rs:526`) and does **not** validate against `_GAMES`, returning **200** `{ "game": game, "data": [] }` for unknown games.
  2. **Response Shape**: Rust adds `"game": game` at top level (`lib.rs:534`).
  3. **Query Parse**: Python **500** on non-numeric `limit` vs Rust **200** fallback.

### 2.9 `POST /pub/ai/ask`
- **Handlers**: Python `_ah_pub_ai_ask` (`bot.py:6207-6228`, `4058-4097`) | Rust `ah_pub_ai_ask` (`lib.rs:549-572`, `kira.rs:4-53`) | Web `lib/pubApi.js:94-105`, `pages/api/pub/ai-ask.js:1-62`, `pages/pub/ai.jsx:49-53`
- **Major Divergences**:
  1. **Web Bypass**: `pubApi.askAi` calls Next.js `/api/pub/ai-ask` instead of `BOT_API_URL/pub/ai/ask`. `pages/api/pub/ai-ask.js` calls `kiraai.vn` directly with its own `KIRA_API_KEY`.
  2. **Request Body**: Python accepts `{ question?, prompt?, model? }`. Rust `AiAskBody` (`lib.rs:550-553`) accepts `prompt` (with `#[serde(alias = "question")]`) and **ignores `model`**.
  3. **Validation & Error Codes**:
     - Bad JSON / missing `prompt`: Python returns **400**; Rust `Json<AiAskBody>` returns **422 Unprocessable Entity** (or **400** on syntax error).
     - Empty/whitespace `prompt`: Python returns **400** `{"error": "Vui lòng nhập câu hỏi."}`; Rust does **not** trim/reject empty `prompt` and forwards it to Kira AI.
     - Upstream Kira error: Python `ask_ai_kira` returns `"Lỗi từ AI (...)"` inside **200** `answer` (only raising -> **500** on network/client exception); Rust returns **502 Bad Gateway** `{"error": e}`.
  4. **Response JSON**:
     - Python & Vercel `/api/pub/ai-ask`: `{ ok: true, answer: str, model: str }`.
     - Rust: `{ ok: true, reply: str, answer: str }` (**missing `model`**, which `pages/pub/ai.jsx:53` reads; adds `reply`).

---

## 3. Recommended Tests

1. **Contract / Parity Tests (Axum `oneshot` or live HTTP against Python & Rust)**:
   - `GET /pub/games`: Assert `"snake"` is present in `data` and `total == 29` (will currently fail on Rust — catches missing `"snake"`).
   - `GET /pub/game/leaderboard?game=invalid` & missing `game`: Assert `400` status and `error` JSON (will currently fail on Rust).
   - `GET /pub/wordbank?limit=abc&offset=-1` and `/pub/rejected`, `/pub/leaderboard`: Assert consistent `400` or safe clamping (prevents Python `500 ValueError` crash).
   - `POST /pub/ai/ask`:
     - Empty/whitespace `{"question": "   "}` -> assert `400` (catches missing validation in Rust).
     - `{"question": "hi", "model": "custom-model"}` -> assert `200` with `ok`, `answer`, `model` fields (catches missing `model` support/field in Rust).
     - Invalid JSON `{` -> assert `400` (not `422`).
2. **CORS Preflight & Error-Path Tests**:
   - Send `OPTIONS` to `/pub/status` and `/pub/ai/ask`; verify `200`/`204` and `Access-Control-Allow-Origin: *`.
   - Trigger `400`/`502` on `/pub/game/leaderboard` and `/pub/ai/ask`; verify CORS headers are still present on error responses.
3. **Abuse / Rate-Limit Test**:
   - Burst `POST /pub/ai/ask` and `/api/pub/ai-ask` (>N req/min per IP) once rate limiting is added, asserting `429 Too Many Requests`.
