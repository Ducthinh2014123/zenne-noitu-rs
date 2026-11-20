# Forensic Audit Report — ID 36

**Scope:** Legacy HTTP/network clients, timeouts, connection pooling, headers (`User-Agent`), retry/backoff, and error handling across Python (`C:\Users\Ai Hoi\Downloads\gdrive-bot-source\gdrive-bot\`) vs. Rust (`C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\`).

---

## 1. Inventory of Network Libraries & Call Sites

### Python (`gdrive-bot`)
- **`httpx`** (sync & async):
  - `bot.py:8`, `get_wiki_session()` (`bot.py:2048-2092`): Singleton `httpx.Client` for Wikipedia login & Wiktionary/Wikipedia queries (`_sync_find_continuations` `bot.py:1974`, `_sync_check_wiktionary` `bot.py:2123`, `_sync_random_wiktionary` `bot.py:2194`, `_sync_random` `bot.py:2215`).
  - `music_system.py:12`: Ephemeral `httpx.AsyncClient` in `get_soundcloud_client_id()` (`music_system.py:82-86`) and `resolve_soundcloud_stream()` (`music_system.py:107-130`).
- **`aiohttp`**:
  - `bot.py:26-27`: Ephemeral `aiohttp.ClientSession()` in `ask_ai_kira()` (`bot.py:4082-4083`), plus `aiohttp.web` HTTP/WS server in `start_web_api()` (`bot.py:6297-6347`).
  - `revolt_bot.py:24`: Persistent `aiohttp.ClientSession()` in `RevoltBotClient` (`revolt_bot.py:149, 210, 268-269`) for REST (`send_msg` `revolt_bot.py:170`, `get_user_name` `revolt_bot.py:186`, `start` `revolt_bot.py:216`) and WebSocket (`ws_connect` `revolt_bot.py:237`).
- **`urllib.request` / `urllib.parse` / `urllib.error`**:
  - `bot.py:5353-5365` (`_send_recovery_email`) & `bot.py:5439-5451` (`_send_registration_otp_email`): Blocking `urllib.request.urlopen()` to Resend transactional email API (`EMAIL_PROVIDER_URL`).
  - `lay_token_twitch.py:134-142` (`get_twitch_username`): `urllib.request.urlopen()` to `https://api.twitch.tv/helix/users`.
- **`requests` / `youtube_dl` / `yt_dlp`**: **Not used directly** in `gdrive-bot/` (not in `requirements.txt:1-8`). Music uses `wavelink>=3.5.0` (`music_system.py:16, 220-226`) + direct `httpx.AsyncClient` SoundCloud API v2 resolution.
- **Other transports**: `wikipediaapi` (`bot.py:7, 2094-2105`), `discord.py` (`bot.py:2265`), `python-telegram-bot` (`telegram_bot.py:1881`), raw `asyncio.open_connection(..., ssl=ssl_ctx)` (`twitch_bot.py:223-229`), `redis.from_url` (`bot.py:411-418`).

### Rust (`zenne-noitu-rs`)
- **`reqwest` (`0.12`, `rustls-tls`, `json`, `gzip`, `brotli`)** (`Cargo.toml:28`):
  - Shared `reqwest::Client` initialized once in `AppContext::init()` (`crates/zenne-core/src/lib.rs:48-51`) and cloned via `Arc` / `Client` across `zenne-core/src/{kira.rs,wiki.rs}`, `zenne-music/src/lib.rs`, `zenne-revolt/src/lib.rs`, `zenne-api/src/lib.rs`.
- **`serenity` (`0.12`) + `poise` (`0.6`) + `songbird` (`0.6`)**, **`teloxide` (`0.13`)**, **`tokio-tungstenite` (`0.24`)**, **`lettre` (`0.11`)**, **`redis` (`0.27`)**.

---

## 2. Side-by-Side Forensic Comparison

### A. Timeouts
| Surface | Python (`gdrive-bot`) | Rust (`zenne-noitu-rs`) |
|---|---|---|
| **Global/Shared HTTP Client** | None. | `Client::builder().timeout(Duration::from_secs(15))` (`crates/zenne-core/src/lib.rs:50`). No separate `connect_timeout`. |
| **Wiki / Wiktionary** | `httpx.Client(..., timeout=10.0)` (`bot.py:2064`), overridden per-call to `timeout=6.0` (`bot.py:1974, 2123, 2194, 2215`). | Inherits `15s` global `reqwest` timeout (`crates/zenne-core/src/wiki.rs:100, 122, 150`). |
| **Kira AI** | `aiohttp.ClientTimeout(total=45)` (`bot.py:4083`). | Inherits `15s` global `reqwest` timeout (`crates/zenne-core/src/kira.rs:30-35`) — **3× shorter than Python's 45s**, risking premature timeouts on slow LLM completions. |
| **SoundCloud Resolver & Audio** | `httpx.AsyncClient(timeout=8.0)` (`music_system.py:82, 107`). | Inherits `15s` global `reqwest` timeout (`crates/zenne-music/src/lib.rs:76, 193, 346, 390`). |
| **Revolt REST** | **Unbounded / default `aiohttp` 300s** (`revolt_bot.py:170, 186, 216`). | Inherits `15s` global `reqwest` timeout (`crates/zenne-revolt/src/lib.rs:30-35`). |
| **Email / Auth** | `urllib.request.urlopen(request, timeout=10)` (`bot.py:5365, 5451`). | `lettre::AsyncSmtpTransport` default timeout (`crates/zenne-api/src/auth_routes.rs:124-136`). |
| **Redis** | `socket_timeout=5, socket_connect_timeout=5` (`bot.py:414-415`). | `redis::aio::ConnectionManager::new(client)` default (`crates/zenne-core/src/cache.rs:22`). |

### B. Connection Pooling & Client Lifecycle
- **Python:** `get_wiki_session()` (`bot.py:2048-2092`) reuses global `httpx.Client`; `ask_ai_kira()` (`bot.py:4082`) and `music_system.py:82, 107` create ephemeral clients per call; `urllib.request.urlopen()` opens a new connection per call.
- **Rust:** `AppContext::init()` (`crates/zenne-core/src/lib.rs:48-51`) builds **one** `reqwest::Client` shared across all crates. `zenne-music/src/lib.rs:385-401` bounds concurrent HLS segment downloads via `.buffer_unordered(12)`.

### C. Headers (`User-Agent` & Auth)
- **Python:** Wikipedia/Wiktionary `Masonjohn83bot/1.0`, Kira AI `ZenneBot-AI/1.0`, Email `Zenne-NoiTu-Recovery/1.0`, SoundCloud spoofed Chrome UA (`music_system.py:79, 103`).
- **Rust:** Unified `ZenneNoituBot/2.0 (Rust)` (`crates/zenne-core/src/lib.rs:49`). SoundCloud (`crates/zenne-music/src/lib.rs:76-93, 193-258`) sends `ZenneNoituBot/2.0 (Rust)` instead of the browser UA.

### D. Retry & Backoff
- **Python:** Zero HTTP retry/backoff; fixed `5s` WS/IRC reconnect (`revolt_bot.py:257`, `twitch_bot.py:294`).
- **Rust:** SoundCloud 401/403 token refresh retry once (`zenne-music/src/lib.rs:200-209, 246-259`); HLS segment 3x tight retry loop without delay/jitter (`lib.rs:389-399`); zero retry/backoff on Wiki/Kira/Revolt/API; fixed `5s` WS reconnect (`zenne-revolt/src/lib.rs:355`, `zenne-twitch/src/lib.rs:334`).

---

## 3. Key Forensic Findings & Risk Summary
1. **Kira AI Timeout Regression:** Rust's shared `15s` `reqwest` timeout (`zenne-core/src/lib.rs:50`) vs. Python's `45s` (`bot.py:4083`) will abort slow AI responses prematurely.
2. **Negative Caching on Network Errors (`wiki.rs:136`):** Transient `reqwest` timeouts/errors in `wiki::check_word` fall through and write `"0"` to Redis/memory cache for 1 hour (`3600s`).
3. **Unwired `DISCORD_PROXY` (`zenne-core/src/config.rs:6, 53, 102`):** Configured in env but unused by Serenity/Reqwest.
4. **Silent Failure in `send_revolt_msg` (`zenne-revolt/src/lib.rs:30-35`):** `let _ = ...` drops HTTP status and transport errors without logging.
5. **Tight Retry Loop & Silent Segment Drop (`zenne-music/src/lib.rs:389-398`):** 3 immediate retries with no backoff/jitter, and silent empty-vec fallback on failure.
6. **Fixed 5s Reconnect Loops (`revolt_bot.py:257`, `twitch_bot.py:294`, `zenne-revolt/src/lib.rs:355`, `zenne-twitch/src/lib.rs:334`):** Both codebases lack exponential backoff + jitter on WebSocket/IRC reconnects.
