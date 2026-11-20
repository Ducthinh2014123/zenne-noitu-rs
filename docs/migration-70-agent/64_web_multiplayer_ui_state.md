# Forensic Audit Report — ID 64

## Scope Inspected
- `zenne-noitu-deploy-vercel/pages/pub/{multiplayer.jsx, play/[roomId].jsx}`, `pages/telegram/index.jsx`
- `zenne-noitu-rs/crates/zenne-api/src/{lib.rs, ws_game.rs}`, `zenne-core/src/db.rs`

---

## Critical & High-Severity Findings
1. **[CRITICAL] Host Transfer Permanently Broken on Web Client (`[roomId].jsx:54-129`):** `ws.onmessage` captures `myName === ''` at `connect()`, so `d.name === myName` on `new_host` is always `false`. Promoted host never gets `isHost = true`.
2. **[HIGH] No Server-Side Turn Timer (`ws_game.rs`):** Client listens for `timeout`, but server never spawns a turn timer. AFK players stall the match forever. Mid-game joiners also don't start `startTick`.
3. **[HIGH] `max_players` Never Enforced (`ws_game.rs:179-194, 240-267`):** `WsRoom` ignores `max_players`; `multiplayer.jsx:270-276` renders join links for full rooms.
4. **[HIGH] Stale & Orphaned Rooms (`lib.rs:779`, `ws_game.rs:179-194, 520`, `db.rs:925-933, 1026, 1037`):** Unjoined rooms stay in `public_rooms` with `current_players: 1` for 3h; `ws_rooms` leaks pre-join disconnects; `update_public_room_players`/`close_public_room` use case-sensitive `WHERE room_id = ?`; `ws_game.rs` allows joining closed/non-existent rooms.

## Medium-Severity Findings
- Unauthenticated `ah_pub_create_room`; no filtering or `status` badge in `multiplayer.jsx`.
- `telegram/index.jsx:78` fallback links to `/pub/play` without `roomId`; copy button copies `teleUrl` instead of token.
- `public_rooms.status` is never updated to `'playing'`/`'ended'` on WS start/end.
- `turn_idx` on disconnect (`ws_game.rs:544-546`) can land on an eliminated (`lives == 0`) player.
