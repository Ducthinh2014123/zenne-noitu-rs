# Forensic Audit Report — ID 34

## 1. Executive Summary
Forensic audit of environment variables, hardcoded defaults, and fallback secrets across:
- **Legacy Python**: `C:\Users\Ai Hoi\Downloads\gdrive-bot-source\gdrive-bot\` (`bot.py`, `music_system.py`, `telegram_bot.py`, `twitch_bot.py`, `revolt_bot.py`, `run_bots.py`, `.env.example`, `.env`)
- **Rust Rewrite**: `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-core\src\config.rs` (`AppConfig::from_env()`)
- **Vercel Frontend**: `C:\Users\Ai Hoi\Downloads\zenne-noitu-deploy-vercel\`

**Key Critical Findings:**
1. **Hardcoded Fallback Secrets in Rust `config.rs`**: `API_KEY` defaults to `"Duc thinh 1234@"` (cascading into `WEB_TOKEN_SECRET` and `GAME_SESSION_SECRET`), `LAVALINK_PASSWORD` defaults to `"youshallnotpass"`, and `SOUNDCLOUD_CLIENT_ID` defaults to `"QhVEWrrJKvW4uSU5u5EoI0FTXYUGzy5k"`.
2. **Mismatched Super Admin ID**: Python `bot.py:67` hardcodes `SUPER_ADMIN = 1395221255589986374`, whereas Rust `config.rs:64` defaults `SUPER_ADMIN_IDS` to `"1177130080259219507"`.
3. **Cryptographic Secret Derivation Mismatch**: Python `bot.py:107-108` (`_web_token_secret()`) derives the HMAC key as `sha256(API_KEY + "::web-game-token").hexdigest()`, whereas Rust `config.rs:70-71` defaults `web_token_secret` and `game_session_secret` directly to the raw `api_key`.
4. **Missing Env Vars in Rust**: `REDIS_PUBLIC_URL` (used by Python `bot.py:70,393-394`), `TELEGRAM_TOKEN` (alias in `telegram_bot.py:112`), and `EMAIL_API_KEY` / `EMAIL_FROM` / `EMAIL_PROVIDER_URL` (replaced by `SMTP_*` in Rust).
5. **Python `bot.py` Ignores `.env` & Env Vars for API/DB/Timeout**: `bot.py` does not call `load_dotenv()` (unlike `run_bots.py`, `telegram_bot.py`, `twitch_bot.py`, `revolt_bot.py`), and hardcodes `API_KEY`, `API_PORT`, `WEB_BASE_URL`, `TURN_TIMEOUT`, `DB_PATH`, `SUPER_ADMIN`, `WIKI_USERNAME`, and `WIKI_PASSWORD` without `os.getenv()`.
6. **Vercel Hardcoded Live Secrets**: `lib/db.js:4-5` (Turso RW JWT), `pages/api/auth/register.js:2` & `[...nextauth].js:18` (Turnstile secret), `pages/api/pub/ai-ask.js:1` (Kira API key), `pages/api/auth/steam-callback.js:76` (`NEXTAUTH_SECRET || 'fallback'`).

---

## 2. Complete Python Inventory (`gdrive-bot`)

### `bot.py`
| Variable / Constant | Line | Lookup / Mechanism | Hardcoded Default / Value |
|---|---|---|---|
| `DISCORD_TOKEN` | `bot.py:61` | `os.getenv("DISCORD_TOKEN", ...)` | `"MTU0..."` **(Live Token)** |
| `DISCORD_PROXY` | `bot.py:62` | `os.getenv("DISCORD_PROXY", None)` | `None` |
| `TURN_TIMEOUT` | `bot.py:63` | Hardcoded (no `os.getenv`) | `60` |
| `MIN_PLAYERS` | `bot.py:64` | Hardcoded | `1` |
| `DEFAULT_LANG` | `bot.py:65` | Hardcoded | `"vi"` |
| `USER_AGENT` | `bot.py:66` | Hardcoded | `"Masonjohn83bot/1.0 (...)"` |
| `SUPER_ADMIN` | `bot.py:67` | Hardcoded (no `os.getenv`) | `1395221255589986374` |
| `DATABASE_URL` | `bot.py:68` | `os.getenv("DATABASE_URL", ...)` | `"libsql://masonjohn83-masonjohn83.aws-ap-southeast-1.turso.io"` |
| `DATABASE_AUTH_TOKEN` | `bot.py:69` | `os.getenv("DATABASE_AUTH_TOKEN", ...)` | **(Live RW Turso JWT)** |
| `REDIS_URL` | `bot.py:70`, `393-394` | `os.getenv("REDIS_PUBLIC_URL") or os.getenv("REDIS_URL") or ...` | **(Live Redis URL)** |
| `DB_PATH` | `bot.py:71` | Hardcoded (no `os.getenv`) | `"noitu_bot.db"` |
| `WIKI_USERNAME` | `bot.py:72` | Hardcoded | `"Masonjohn83bot@wikibotdiscord"` |
| `WIKI_PASSWORD` | `bot.py:73` | Hardcoded | **(Hardcoded Secret)** |
| `API_KEY` | `bot.py:77` | Hardcoded (ignores `.env`!) | `"Duc thinh 1234@"` **(Hardcoded Secret)** |
| `API_PORT` | `bot.py:78` | Hardcoded (ignores `.env`!) | `26035` |
| `WEB_BASE_URL` | `bot.py:79` | Hardcoded (ignores `.env`!) | `"https://zenne-noitu.vercel.app"` |
| `WEB_GUILD_ID` | `bot.py:81` | Hardcoded | `0` |
| `KIRA_API_KEY` | `bot.py:85` | `os.getenv("KIRA_API_KEY", ...)` | **(Live API Key)** |
| `KIRA_API_ENDPOINT` | `bot.py:86` | `os.getenv("KIRA_API_ENDPOINT", ...)` | `"https://kiraai.vn/api/v1"` |
| `KIRA_DEFAULT_MODEL` | `bot.py:87` | `os.getenv("KIRA_DEFAULT_MODEL", ...)` | `"stealth-missingno"` |
| `EMAIL_API_KEY` | `bot.py:91` | `os.getenv("EMAIL_API_KEY", ...)` | **(Live Resend Key)** |
| `EMAIL_FROM` | `bot.py:92-93` | `os.getenv("EMAIL_FROM", ...)` | `"no-reply@zenne-noitu.me"` (forces if `"kdns.fr"` in value) |
| `EMAIL_PROVIDER_URL` | `bot.py:94` | `os.getenv("EMAIL_PROVIDER_URL", ...)` | `"https://api.resend.com/emails"` |
| `_web_token_secret()` | `bot.py:107-108` | Derived from `API_KEY` | `sha256(API_KEY + "::web-game-token").hexdigest()` |

### `music_system.py`
- `LAVALINK_URI` (`music_system.py:19`): `os.getenv("LAVALINK_URI", "http://june.hidencloud.com:24605")`
- `LAVALINK_PASSWORD` (`music_system.py:20`): `os.getenv("LAVALINK_PASSWORD", "youshallnotpass")`
- `SOUNDCLOUD_CLIENT_ID` (`music_system.py:21`): `os.getenv("SOUNDCLOUD_CLIENT_ID", "QhVEWrrJKvW4uSU5u5EoI0FTXYUGzy5k")`

### `telegram_bot.py`
- `TELEGRAM_BOT_TOKEN` (`telegram_bot.py:112`): `os.getenv("TELEGRAM_BOT_TOKEN") or os.getenv("TELEGRAM_TOKEN", ...)`
- `TURN_TIMEOUT` (`telegram_bot.py:113`): `int(os.getenv("TURN_TIMEOUT", "60"))`
- `DEFAULT_LANG` (`telegram_bot.py:114`): `"vi"`

### `twitch_bot.py`
- `TWITCH_BOT_NICK` (`twitch_bot.py:81`): `os.getenv("TWITCH_BOT_NICK", "zenne_noitu_bot").strip().lower()`
- `TWITCH_OAUTH_TOKEN` (`twitch_bot.py:82`): `os.getenv("TWITCH_OAUTH_TOKEN", "").strip()`
- `TWITCH_CHANNELS_RAW` (`twitch_bot.py:83`): `os.getenv("TWITCH_CHANNELS", "ducthinh2014123").strip()`
- `TWITCH_PREFIX` (`twitch_bot.py:84`): `os.getenv("TWITCH_COMMAND_PREFIX", "!").strip()`
- `TURN_TIMEOUT` (`twitch_bot.py:85`): `int(os.getenv("TURN_TIMEOUT", "60"))`

### `revolt_bot.py`
- `REVOLT_BOT_TOKEN` (`revolt_bot.py:81`): `os.getenv("REVOLT_BOT_TOKEN", ...).strip()`
- `REVOLT_PREFIX` (`revolt_bot.py:82`): `os.getenv("REVOLT_COMMAND_PREFIX", "!").strip()`
- `REVOLT_API_URL` (`revolt_bot.py:83`): Hardcoded `"https://api.revolt.chat"`
- `REVOLT_WS_URL` (`revolt_bot.py:84`): Hardcoded `"wss://events.stoat.chat?format=json"`
- `TURN_TIMEOUT` (`revolt_bot.py:85`): `int(os.getenv("TURN_TIMEOUT", "60"))`

### `run_bots.py`
- `TWITCH_OAUTH_TOKEN` (`run_bots.py:72`) & `REVOLT_BOT_TOKEN` (`run_bots.py:81`)

---

## 3. Rust `AppConfig::from_env()` Comparison (`zenne-core/src/config.rs`)

### A. Missing Env Vars in Rust
1. **`REDIS_PUBLIC_URL`**: Checked in `bot.py:70,393-394`, absent in `config.rs:108` (only `REDIS_URL`).
2. **`TELEGRAM_TOKEN`**: Fallback alias in `telegram_bot.py:112`, absent in `config.rs:103` (only `TELEGRAM_BOT_TOKEN`).
3. **`EMAIL_API_KEY`, `EMAIL_FROM`, `EMAIL_PROVIDER_URL`**: Used in `bot.py:91-94` (Resend HTTP API), absent in Rust (replaced by `SMTP_HOST`, `SMTP_PORT`, `SMTP_USER`, `SMTP_PASS`, `SMTP_FROM` at `config.rs:129-133`).
4. **`WIKI_USERNAME`, `WIKI_PASSWORD`**: Hardcoded in `bot.py:72-73`, absent in `config.rs`.

### B. Mismatched Defaults (Python vs Rust)
| Key | Python Default | Rust `config.rs` Default | Impact |
|---|---|---|---|
| `SUPER_ADMIN` / `SUPER_ADMIN_IDS` | `1395221255589986374` (`bot.py:67`) | `"1177130080259219507"` (`config.rs:64`) | Wrong owner granted super-admin if env unset |
| `WEB_TOKEN_SECRET` / `GAME_SESSION_SECRET` | `sha256(API_KEY + "::web-game-token")` (`bot.py:107-108`) | Raw `&api_key` / `&web_token_secret` (`config.rs:70-71`) | HMAC token incompatibility between Python & Rust |
| `TWITCH_CHANNELS` | `"ducthinh2014123"` (`twitch_bot.py:83,91`) | `""` (empty vec, `config.rs:58-62`) | Twitch bot joins 0 channels if env unset |
| `DISCORD_TOKEN`, `DATABASE_URL`, `DATABASE_AUTH_TOKEN`, `REDIS_URL`, `KIRA_API_KEY`, `TELEGRAM_BOT_TOKEN`, `REVOLT_BOT_TOKEN` | Hardcoded live secrets/URLs | `""` (`config.rs:101,103,105-106,108-109,118,121`) | Safe in Rust, but requires explicit env configuration |

### C. Hardcoded Fallback Secrets in Rust `config.rs`
- `API_KEY` (`config.rs:69`): `"Duc thinh 1234@"` — weak hardcoded admin key, also used as fallback for `WEB_TOKEN_SECRET` (`config.rs:70`) and `GAME_SESSION_SECRET` (`config.rs:71`).
- `LAVALINK_PASSWORD` (`config.rs:128`): `"youshallnotpass"` (with `LAVALINK_URI` `"http://june.hidencloud.com:24605"` at `config.rs:127`).
- `SOUNDCLOUD_CLIENT_ID` (`config.rs:123-126`): `"QhVEWrrJKvW4uSU5u5EoI0FTXYUGzy5k"`.
- `SUPER_ADMIN_IDS` (`config.rs:64`): `"1177130080259219507"`.

---

## 4. Vercel Frontend Audit (`zenne-noitu-deploy-vercel`)

1. **Hardcoded Fallback Secrets:**
   - `lib/db.js:4-5,7-8`: Hardcodes `TURSO_DB_URL` and `TURSO_DB_TOKEN` as fallback if `DATABASE_HTTP_URL` / `DATABASE_AUTH_TOKEN` are missing or match legacy checks.
   - `pages/api/auth/register.js:2,6` & `pages/api/auth/[...nextauth].js:18,22`: Hardcodes `PRIMARY_TURNSTILE_SECRET` before `process.env.TURNSTILE_SECRET_KEY`.
   - `pages/api/pub/ai-ask.js:1`: Hardcodes `KIRA_API_KEY` fallback.
   - `pages/api/auth/steam-callback.js:76`: Falls back to `process.env.NEXTAUTH_SECRET || 'fallback'` for HMAC-SHA256 signing.
2. **Mismatched Defaults:**
   - `pages/api/pub/ai-ask.js:3`: `KIRA_DEFAULT_MODEL` defaults to `'qwen3.8-flash-free'`, whereas Python (`bot.py:87`) and Rust (`config.rs:111`) default to `'stealth-missingno'`.
   - `lib/serverApi.js:11` & `pages/api/auth/steam-callback.js:56`: `BOT_ADMIN_API_KEY` defaults to `''` (empty), whereas Python/Rust default `API_KEY` to `"Duc thinh 1234@"`.
   - `lib/pubApi.js:1-9`, `lib/recoveryApi.js:4-12`, `lib/serverApi.js:1-10`: `resolveBotApiUrl` falls back to `'https://api.zenne-noitu.me'`.
