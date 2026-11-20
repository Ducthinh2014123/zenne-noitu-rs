# Forensic Audit Report — ID 62

## Scope & Paths Audited
- **Vercel / Next.js API Routes**: `C:\Users\Ai Hoi\Downloads\zenne-noitu-deploy-vercel\pages\api\**`
- **Vercel Client/Server Helpers**: `lib\{api,pubApi,recoveryApi,serverApi}.js`
- **Rust Backend (`zenne-api`)**: `crates\zenne-api\src\{lib.rs,auth_routes.rs,pub_games.rs}`

---

## Findings: Contract Mismatches & Anomalies

1. **`POST /pub/auth/login` issues valid JWT before 2FA verification (`auth_routes.rs:211-251`):** Top-level `totp_enabled` is `bool` while nested `user.totp_enabled` is `i64`.
2. **`POST /pub/auth/oauth` silent fallback (`lib.rs:922-978`):** Ignores `UNIQUE` `username` constraint error and returns `{ "ok": true }` without `id`/`token`, causing `[...nextauth].js` & `steam-callback.js` to fall back to string `providerAccountId` / `steamId`, breaking `i64` deserialization downstream (`422`).
3. **`POST /api/auth/recovery/request.js:11-14`:** Converts `400`/`502` into `200 { success: true }` (anti-enumeration).
4. **`pages/api/auth/recovery/status.js:5,9`:** Accepts `GET` from browser and translates to `POST /pub/auth/recovery/status`.
5. **`POST /api/pub/game-token.js:17-20`:** Sends **positive** `session.user.id` whereas `game-score.js`, `game-session.js`, `game-ttt.js`, `game-ms.js` all send **negative** `-Number(session.user.id)`.
6. **`game-session.js` & `game-score.js`:** Restrict `ALLOWED_GAMES` to `['snake','2048','afk','quiz']` and clamp to `10_000_000`, whereas Rust `sanity_check_score` (`pub_games.rs:341-347`) rejects `2048 > 500_000`, `snake > 10_000`, `quiz > 1_000_000`.
7. **`/pub/game/ttt` & `/pub/game/ms` (`lib.rs:726-752`):** Unauthenticated on Rust side (no `check_admin_auth`).
8. **`POST /api/pub/ai-ask.js`:** Bypasses Rust `/pub/ai/ask` and returns `{ ok, answer, model }`, whereas Rust `ah_pub_ai_ask` (`lib.rs:549-572`) ignores `model` and returns `{ ok, reply, answer }`.
9. **`POST /pub/multiplayer/create` vs `pages/api/pub/multiplayer/create.js`:** Mismatched `room_id` format (8-char UUID vs 6-char hex), empty `name` handling (`200` default vs `400`), and response fields.
10. **`GET /pub/multiplayer/rooms` vs `pages/api/pub/multiplayer/rooms.js`:** Fallback omits `data` array and 3h expiration.
