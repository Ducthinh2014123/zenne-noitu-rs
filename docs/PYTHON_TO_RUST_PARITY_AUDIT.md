# Python → Rust Parity Audit & Migration Matrix

- **Legacy Python Source:** `C:\Users\Ai Hoi\Downloads\gdrive-bot-source\gdrive-bot\` (`bot.py`, `telegram_bot.py`, `music_system.py`, `twitch_bot.py`, `revolt_bot.py`, `*_system.py`)
- **Current Rust Workspace:** `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\` (branch `migration/python-parity`)
- **Web/Vercel Client:** `C:\Users\Ai Hoi\Downloads\zenne-noitu-deploy-vercel\`

---

## Migration Matrix

| ID | Feature | Python source | Rust source | Web dependency | Status | Risk | Required action |
|---|---|---|---|---|---|---|---|
| F-01 | Multi-bot Runner & Early-Exit Isolation | `run_bots.py:49-127` | `src/main.rs:75-84` | All | Partial | High | Do not terminate all bots/API if one bot has empty token (`tokio::select!` bug) |
| F-02 | DB Schema (12 tables) | `bot.py:835-980` | `zenne-core/src/db.rs:91-224` | All | Implemented | Low | Preserve schema |
| F-03 | DB Secondary Indexes (12 indexes) | `bot.py:981-994` | `zenne-core/src/db.rs:91-224` | All | Missing | Medium | Add `CREATE INDEX IF NOT EXISTS` for all 12 indexes in `init_schema` |
| F-04 | DB `submit_pending` Resubmit Rejected | `bot.py:1151-1160` | `zenne-core/src/db.rs:347-379` | `/pending`, Discord | Partial | Medium | Allow resubmitting previously `rejected` words |
| F-05 | DB `cache_add` Hit Count Increment | `bot.py:1255-1256` | `zenne-core/src/db.rs:466-475` | Word-chain | Partial | Low | Use `ON CONFLICT DO UPDATE SET hit_count = hit_count + 1` |
| F-06 | DB `lb_add_game` & `lb_streak_top` | `bot.py:1332-1380` | `zenne-core/src/db.rs` | `/pub/leaderboard`, Discord | Missing | Medium | Implement `lb_add_game` and `lb_streak_top` in `DbManager` |
| F-07 | DB `game_submit_score` Cumulative Mode | `bot.py:1397-1424` | `zenne-core/src/db.rs:591-605` | AFK, Minigames | Partial | Medium | Support `cumulative` flag (`best_score = best_score + ?`) |
| F-08 | DB `public_rooms` 3h Auto-Close & `get_public_room` | `bot.py:1472-1494` | `zenne-core/src/db.rs:847-875` | `/pub/multiplayer` | Partial | Medium | Auto-close rooms > 3h old in `get_public_rooms`, add `get_public_room` |
| F-09 | DB Tournament Methods (`tournament_players`) | `bot.py:1727-1825` | `zenne-core/src/db.rs` | Discord | Missing | Medium | Add `tournament_create`, `get_active`, `join`, `add_word`, `players`, `end` |
| F-10 | Redis Rate Limiter & Non-Blocking Cache | `bot.py:5304-5315` | `zenne-core/src/cache.rs:13-115` | Auth/Recovery | Missing | High | Add Redis/in-memory rate-limiter (`incr_with_ttl`) & avoid wiping entire L1 cache |
| F-11 | Constant-Time HMAC & Game Session `iat` | `bot.py:117-157` | `zenne-core/src/auth.rs:24-81` | Web Auth, Games | Incompatible | High | Use `Mac::verify_slice` (constant-time) and include `iat` in `sign_game_session` |
| F-12 | SoundCloud `client_id` Auto-Renewal | `music_system.py:69-94` | `zenne-music/src/lib.rs:50-63` | Discord Music | Missing | High | Scrape `soundcloud.com` / `sndcdn.com` JS bundles when expired/401, cache 24h |
| F-13 | SoundCloud Playlist Queueing | `music_system.py:346-387` | `zenne-music/src/lib.rs:148-166` | Discord Music | Partial | Low | Keep single-track + MultiPlay 10-worker architecture; optionally queue playlist tracks |
| F-14 | Discord `!nowplaying` & Word-Chain Turn/Timeout | `bot.py:2417-2449, 2632-2776` | `zenne-discord/src/{noitu,extra_cmds}.rs` | Discord | Partial | Medium | Enforce `game_channel` (`!setnoitu`), turn timeout, `lb_add_game`, `to_player` tournament join |
| F-15 | Discord Buggy Command Outputs (`mastermind`, `typing`, `danhsachtu`) | `bot.py:4886` | `zenne-discord/src/extra_cmds.rs:155,488,514` | Discord | Incompatible | Medium | Fix tuple unpacking in `mastermind`/`typing` and `(20, 0)` in `danhsachtu` |
| F-16 | Discord Missing Commands (`connect4`, `uno`, `duyettu`/`tuchoi`, `thamgiagiaidau`) | `bot.py:2455,3385` | `zenne-discord/src/lib.rs` | Discord | Missing | Medium | Add missing commands & aliases |
| F-17 | Telegram Full Command Parity + Callbacks + Deep Link | `telegram_bot.py:1-1932` | `zenne-telegram/src/{lib,noitu,minigames,state}.rs` | `/telegram` | Implemented | Low | Verify `setuilang`/`listlang` optional extras |
| F-18 | Twitch & Revolt Full Word-Chain + Minigames | `twitch_bot.py`, `revolt_bot.py` | `zenne-twitch/src/lib.rs`, `zenne-revolt/src/lib.rs` | Multi-platform | Partial | Medium | Add `taophong`, `phong`, `top`, `setlang`, `wordle`, `trieuphu`, turn timers, DB persistence |
| F-19 | Auth Login Contract (`/pub/auth/login`) | `bot.py:5752-5769` | `zenne-api/src/auth_routes.rs:72-150` | `[...nextauth].js:138` | Incompatible | Critical | Accept `email` OR `username`, check `email_verified`, return top-level `id`, `username`, `email`, `is_admin`, `totp_enabled` |
| F-20 | Auth 2FA Verify (`/pub/auth/verify-2fa`) & Backup Codes | `bot.py:5770-5802` | `zenne-api/src/lib.rs:795-837` | `[...nextauth].js:151` | Incompatible | Critical | Accept `user_id` OR `username`, support backup codes |
| F-21 | Auth OAuth (`/pub/auth/oauth`) | `bot.py:5803-5845` | `zenne-api/src/lib.rs:839-877` | `[...nextauth].js:73`, `steam-callback.js:62` | Incompatible | Critical | Accept `name` alias for `username`, `image`, return top-level `id`, `username`, `is_admin`, `email` |
| F-22 | Game Token (`/pub/auth/game-token`) | `bot.py:5846-5864` | `zenne-api/src/lib.rs:879-887` | `game-token.js:17` | Incompatible | Critical | Require `X-API-Key`, read `{ user_id, username }` from JSON body |
| F-23 | Account & 2FA Routes (`/pub/account*`) | `bot.py:5866-5980` | `zenne-api/src/lib.rs:889-990` | `pages/api/account/*.js` | Incompatible | Critical | Support `X-API-Key` + `user_id` (matching `serverApi.js`) AND Bearer JWT; return `has_password`, `avatar`, `provider`, `backup_codes`; require code/pw on `disable-2fa` |
| F-24 | Recovery Routes (`/pub/auth/recovery/*`) | `bot.py:5982-6204` | `zenne-api/src/auth_routes.rs:487-693` | `pages/api/auth/recovery/*.js` | Incompatible | Critical | Match `{ success, recovery_token, reset_session, canResetPassword, totpEnabled }`, check `expires_at`, atomic `used_at IS NULL`, 5-attempt lockout, 2FA check on reset |
| F-25 | Game Score (`/pub/game/score`) & Session (`/pub/game/session/start`) | `bot.py:4469-4534` | `zenne-api/src/lib.rs:450-491,550-558` | `game-score.js`, `game-session.js` | Incompatible | Critical | Enforce `X-API-Key`, require `session_token`, check `iat` elapsed velocity, support monotonic nonce |
| F-26 | WebSocket `/ws/game/:room_id` Auth, Timer & Persistence | `bot.py:4610-4881` | `zenne-api/src/ws_game.rs` | `[roomId].jsx` | Partial | Critical | Verify `token` on `join`, add per-turn timeout + `timeout` event, persist scores to DB, case-insensitive room lookup |
| F-27 | Room Creation `/pub/game/create` & Admin API Contracts | `bot.py:4199-4357, 4872-4885` | `zenne-api/src/lib.rs` | `play/index.jsx`, `lib/api.js` | Incompatible | High | Return top-level `room_id`, fix `/pub/play/{rid}`, fix `/api/pending`/`review`/`review_bulk`/`wordbank`/`leaderboard`/`guilds` & `/pub/ai/ask` |
| F-28 | NextAuth OAuth Token Persistence & Refresh | N/A | N/A | `[...nextauth].js:196-205` | Missing | Medium | Persist `access_token`, `refresh_token`, `expires_at` in JWT & implement refresh flow |
| F-29 | `zenne-games` Banks & Stubs (`SudokuManager`, `UnoManager`, `TrieuPhu`) | `*_system.py` | `zenne-games/src/*.rs` | Discord/Telegram | Partial | Medium | Expand `TrieuPhu` to 15+ distinct questions, upgrade `SudokuManager` & `UnoManager` with real playable methods |
