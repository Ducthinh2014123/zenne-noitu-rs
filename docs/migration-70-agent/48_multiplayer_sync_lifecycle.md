# Forensic Audit Report — ID 48

## Executive Summary
The multiplayer room lifecycle spans two repositories—`zenne-noitu-rs` (Rust backend: DB, Discord, Telegram, Axum HTTP/WS) and `zenne-noitu-deploy-vercel` (Next.js web frontend + fallback API routes)—sharing the `public_rooms` and `telegram_room_tokens` tables. While basic creation, listing, 3-hour lazy expiration, and closing exist, there are **critical cross-platform synchronization and lifecycle gaps**: Telegram's `NoituRoomState` and WS's `WsRoom` are completely separate in-memory engines, `telegram_room_tokens` has no expiration/cleanup, `public_rooms.status` is never transitioned to `'playing'` on WS start, and the Vercel fallback `/api/pub/multiplayer/rooms` omits the 3-hour expiration check.

---

## 1. Database Schema & Lifecycle (`public_rooms`, `telegram_room_tokens`)
**File:** `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-core\src\db.rs`
- **Schemas:** `public_rooms` (`L195-210`) & `telegram_room_tokens` (`L211-219`).
- **3-Hour Expiration (`get_public_rooms`, `L925-960`):** Lazily expires rooms where `datetime(created_at, '+3 hours') < datetime('now','localtime')` on read (no background reaper).
- **Single-Room Lookup (`get_public_room`, `L962-986`):** Queries `WHERE UPPER(room_id) = UPPER(?1) LIMIT 1` without 3h expiry or `status != 'closed'` filter.
- **Create/Upsert (`create_public_room`, `L988-1020`):** `ON CONFLICT(room_id) DO UPDATE` does **not** reset `created_at`.
- **Telegram Tokens (`save_telegram_token`, `L1044-1061`; `get_telegram_token`, `L1063-1085`):** No expiration, deletion, or `public_rooms.status` join check.

---

## 2. Discord (`taoroom` / `multiplayer`)
**Files:** `crates\zenne-discord\src\extra_cmds.rs`, `lib.rs`
- **`taoroom` (`extra_cmds.rs:179-212`):** Generates 6-char hex `room_id` and writes `public_rooms` (`platform: "discord"`, `web_url`), directing users to Web WS.
- **`multiplayer` (`extra_cmds.rs:216-241`):** Calls `db.get_public_rooms()` and lists up to 10 rooms.

---

## 3. Telegram (`taophong` / `join_<token>` / `mp_*`)
**Files:** `crates\zenne-telegram\src\noitu.rs`, `state.rs`
- Creates in-memory `NoituRoomState` + `st.room_tokens` + `telegram_room_tokens` + `public_rooms`.
- Closes `public_rooms` on `/stop`, timeouts, and `nt_cancel`, **but never cleans `st.room_tokens` or `telegram_room_tokens`**.
- **Split-brain:** Players joining via `web_url` connect to `WsRoom`, which does **not** sync with Telegram's `NoituRoomState`.

---

## 4. Web (`pages/pub/play/[roomId].jsx`, `/pub/multiplayer/*`, API routes)
- `zenne-api/src/lib.rs`: `/pub/multiplayer/rooms`, `/pub/multiplayer/create`, `/pub/game/create`, `/pub/multiplayer/token/:token`.
- Vercel fallback `pages/api/pub/multiplayer/rooms.js` (`L16-22`) omits 3-hour expiration `UPDATE` and `created_at` filter.

---

## 5. WebSocket (`/ws/game/:room_id`)
**File:** `crates\zenne-api\src\ws_game.rs`
- `handle_ws_connection` (`L156-194`) allows creating/joining `WsRoom` even for non-existent, closed, or full `public_rooms` entries.
- Start/restart/end (`L292-331, 498-515`) never update `public_rooms.status` to `'playing'`/`'ended'` in DB.
- On last-player disconnect (`L533-536`), removes `WsRoom` and calls `db.close_public_room(&rid)`.
