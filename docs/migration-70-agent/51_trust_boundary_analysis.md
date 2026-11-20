# Forensic Audit Report — ID 51

## 1. Trust-Boundary Map
1. **Browser → Next.js (`zenne-noitu-deploy-vercel`):** NextAuth JWT cookie (`[...nextauth].js:218`); Admin UI (`pending.jsx:23`, `lib/api.js:2-18`) stores `nt_api_key` in `localStorage` and calls Rust Axum directly; Public UI calls Next.js proxies or Rust Axum directly via `lib/pubApi.js`.
2. **Next.js → Rust Axum / Python:** `botApi()` (`lib/serverApi.js:9-27`) attaches `X-API-Key`; `/pub/auth/*` proxies omit `X-API-Key`; `multiplayer/{create,rooms}.js` bypass backend API and write/read Turso directly via `lib/db.js`.
3. **Rust Axum / Python → Turso & Redis:** `DbManager::new` (`db.rs:70-85`), `CacheManager::new` (`cache.rs:19-48`).
4. **Browser → WebSocket `/ws/game/:room_id`:** Browser fetches HMAC token from `/api/pub/game-token`, opens WS to `ws_game::handle_ws_connection`.
5. **Discord / Telegram → DB:** Platform-authenticated gateway events write directly to Turso.

---

## 2. Critical Vulnerabilities & Trust-Assumption Breaks
1. **Unauthenticated `POST /pub/auth/oauth` (`lib.rs:922-978`, `bot.py:5797-5838`):** No `check_admin_auth` or OAuth token verification; `ON CONFLICT(email) DO UPDATE` mints a 7-day `web_token` for any email.
2. **2FA Bypass (`auth_routes.rs:211-251`):** Omitting `totp_code` on `/pub/auth/login` still mints a 7-day `web_token` alongside `requires_2fa: true`.
3. **Hardcoded Credentials & Key-Sharing (`config.rs:69-71`, `lib/db.js:4-5`, `bot.py:61-91`, `[...nextauth].js:147-148`, `ai-ask.js:1`):** Turso RW JWT, `API_KEY`, FB secret, Kira key exposed in source.
4. **Unauthenticated Minigame Score Injection (`lib.rs:726-752`, `pub_games.rs:130-324`):** `/pub/game/ttt` & `/pub/game/ms` trust caller-supplied `user_id` with no auth.
5. **WebSocket & Multiplayer Trust Breaks (`ws_game.rs:179-238, 457`, `game-token.js:18`):** Unauthenticated room creation; WS lazily creates rooms for arbitrary `room_id` without checking `join_token`; `game-token.js:18` passes **positive** `session.user.id` causing `ws_game.rs:457` to write positive web IDs into `leaderboard.user_id` (colliding with Discord/Telegram IDs).
6. **Unauthenticated AI Proxy (`lib.rs:555-572`, `ai-ask.js:5-62`).**
7. **Missing Guild/Chat Admin Checks (`zenne-discord/src/lib.rs:107-119`, `zenne-telegram/src/noitu.rs:1090-1124`)** & Super-Admin ID mismatch (`bot.py:67` vs `config.rs:64`).
