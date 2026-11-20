# Forensic Audit Report — ID 63

## 1. Audited Files
- **Backend WebSocket:** `zenne-noitu-rs/crates/zenne-api/src/ws_game.rs` (`L1-559`)
- **Frontend Game Room & Helper:** `zenne-noitu-deploy-vercel/pages/pub/play/[roomId].jsx` (`L1-331`), `lib/pubApi.js` (`L1-155`)

---

## 2. Client -> Server JSON Messages
All 5 client message types (`join`, `start`, `word`, `hint`, `restart`) match `ws_game.rs` field names. Non-text/invalid JSON frames are silently skipped.

---

## 3. Server -> Client JSON Messages & Bugs
- **`error` (`ws_game.rs:215-219`, `[roomId].jsx:124-127`):** On bad token, client is still in `phase === 'name'` where `flash` is not rendered, so user never sees `d.msg`.
- **`joined` (`ws_game.rs:270-280`, `[roomId].jsx:69-73`):** Mid-game join (`d.state === 'playing'`) does not call `startTick`.
- **`state` (`ws_game.rs:550`):** Disconnecting while `room.state == "ended"` broadcasts `state_payload()`, wiping `gs.winner` and `gs.scores` on clients.
- **`new_host` (`ws_game.rs:542`, `[roomId].jsx:105-107`):** **Critical Stale Closure Bug** — `connect` captures `myName` (`''`), so `d.name === myName` is always `false` and promoted host never gets `isHost = true`.
- **`timeout` (`[roomId].jsx:97-98`):** Orphaned handler; server never enforces or emits `timeout`.

---

## 4. Ping/Pong Heartbeat
Neither `[roomId].jsx` nor `ws_game.rs` implements application-level `ping`/`pong` messages or a dead-connection reaper.

---

## 5. Reconnection & Error Handling
- No automatic reconnection or reconnect button in `[roomId].jsx`.
- `ws.onerror` / `setError` is only rendered in `phase === 'name'`.

---

## 6. Timer Synchronization
Server (`ws_game.rs:47`) hardcodes `time_limit: 30` with no server-side timer. Client `startTick` (`[roomId].jsx:45-48`) does not reset on `word_invalid`, `word_used`, `player_left`, or mid-game `joined`, and keeps ticking during `checking`.
