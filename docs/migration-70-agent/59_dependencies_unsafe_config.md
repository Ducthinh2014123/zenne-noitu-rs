# Forensic Audit Report — ID 59

**Scope:** External dependency vulnerabilities, hardcoded secrets/tokens, unsafe configurations, and `.unwrap()` / `.expect()` panic sites across `zenne-noitu-rs`, `zenne-noitu-deploy-vercel`, and `gdrive-bot-source/gdrive-bot`.

---

## 1. Hardcoded Secrets, Tokens & Weak Fallback Credentials
- **`zenne-noitu-rs/crates/zenne-core/src/config.rs` (`AppConfig::from_env`):** `API_KEY` defaults to `"Duc thinh 1234@"` (`L69`), cascading to `WEB_TOKEN_SECRET` (`L70`) and `GAME_SESSION_SECRET` (`L71`); `SOUNDCLOUD_CLIENT_ID` (`L125`), `LAVALINK_URI` & `LAVALINK_PASSWORD` (`L127-128`).
- **`zenne-noitu-deploy-vercel`:** `PRIMARY_TURNSTILE_SECRET` (`[...nextauth].js:18`, `register.js:2`), `FACEBOOK_CLIENT_ID/SECRET` (`[...nextauth].js:147-148`), `NEXTAUTH_SECRET || 'fallback'` (`steam-callback.js:76`), `KIRA_API_KEY` (`ai-ask.js:1`), `TURSO_DB_URL/TOKEN` (`lib/db.js:4-5`).
- **`gdrive-bot-source/gdrive-bot`:** `DISCORD_TOKEN`, `DATABASE_AUTH_TOKEN`, `REDIS_URL`, `WIKI_PASSWORD`, `API_KEY`, `KIRA_API_KEY`, `EMAIL_API_KEY` (`bot.py:61-91`).

## 2. Unsafe CORS & Authentication Configurations
- **`zenne-api/src/lib.rs:54-57`:** `CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any)`.
- **`zenne-noitu-deploy-vercel/pages/api/auth/[...nextauth].js:133-154`:** `allowDangerousEmailAccountLinking: true` on all 5 OAuth providers.

## 3. `.unwrap()` / `.expect()` Panic Sites in Rust (`zenne-noitu-rs`)
- **`multiplay.rs`:** All 19 `.unwrap()` calls are inside `#[cfg(test)]` (0 in production).
- **Production `.unwrap()` (4 total):** `pub_games.rs:305`, `battleship.rs:132`, `memory.rs:100`, `zenne-music/src/lib.rs:360` (`Regex::new` on every HLS playlist fetch).

## 4. External Dependency & Container/Build Vulnerabilities
- **`Dockerfile` / `entrypoint.sh`:** Unverified `cloudflared-linux-amd64` download (`Dockerfile:8-10`), unpinned `debian:bookworm-slim`, runs as `root`, tunnel token passed on CLI (`entrypoint.sh:6`).
- **`Cargo.lock`:** Legacy `rustls 0.21/0.22`, `reqwest 0.11`, `hyper 0.14`, `sha1 0.10`.
- **`package.json`:** `"next": "14.2.3"` (vulnerable to `< 14.2.25` CVEs including CVE-2024-34351, CVE-2025-29927).
- **`requirements.txt`:** Unpinned `>=` lower bounds (`aiohttp>=3.9.0`).
