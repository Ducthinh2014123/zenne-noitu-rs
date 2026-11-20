# Forensic Audit Report — ID 20

**Scope**: WebSocket `/ws/game/:room_id` parity audit across:
- **Python**: `C:\Users\Ai Hoi\Downloads\archive-2026-09-23T144146Z\bot.py` (`WebRoom`, `_ah_ws_game`)
- **Rust**: `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-api\src\ws_game.rs` (`WsRoom`, `handle_ws_connection`), `crates\zenne-api\src\lib.rs`, `crates\zenne-core\src\auth.rs`, `crates\zenne-core\src\db.rs`
- **Web**: `C:\Users\Ai Hoi\Downloads\zenne-noitu-deploy-vercel\pages\pub\play\[roomId].jsx` (note: `pages\multiplayer\[roomId].jsx` does not exist; multiplayer room UI lives at `pages\pub\play\[roomId].jsx`), `pages\api\pub\game-token.js`, `lib\pubApi.js`

---

## 1. Auth & Token Verification
| Aspect | Python (`bot.py`) | Rust (`ws_game.rs` / `auth.rs`) | Web (`[roomId].jsx`) |
|---|---|---|---|
| **WS Handshake** | `_ah_ws_game` (`bot.py:3788-3796`): returns 404 if `room_id not in _WEB_ROOMS`; sends `{"type":"connected",...}` on accept. | `ah_ws_game_room` (`lib.rs:598-606`) & `handle_ws_connection` (`ws_game.rs:156-194`): auto-creates `WsRoom` if missing; no `connected` frame. | `connect()` (`[roomId].jsx:54-65`): fetches token via `pubApi.gameToken()` (`POST /api/pub/game-token`), sends `{type:'join', name, token}` on `ws.onopen`. |
| **Token Claims** | `_sign_web_token` (`bot.py:70-75`) & `_verify_web_token` (`bot.py:77-89`): HMAC-SHA256 (`API_KEY + "::web-game-token"`), claims `{"uid", "un", "exp"}`. | `verify_web_token` (`auth.rs:24-37`): HMAC-SHA256 (`web_token_secret`), reads `uid` (defaults to `0` if missing, `ws_game.rs:223`) and `username` (`ws_game.rs:224-228`). | Relies on `/api/pub/game-token` (`game-token.js:17-20`) -> `/pub/auth/game-token` (`lib.rs:986`). |
| **On Auth Failure** | Sends `error` and `continue`s (keeps socket open, `bot.py:3806-3807`). | Sends `error` and `break`s (closes socket, `ws_game.rs:214-221`). | Flashes `d.msg` (`[roomId].jsx:124-126`) but stays on `phase === 'name'` if join fails. |
| **Name & Dupes** | Truncates name to 20 chars; appends `#<last4_uid>` on name collision; rejects duplicate `user_id` (`bot.py:3809-3813`). | No 20-char truncation, no collision suffix; rejects duplicate `conn_id` or `token_uid != 0` (`ws_game.rs:230-254`). | Sends `session.user.username \|\| session.user.name` (`[roomId].jsx:159`). |

**Findings**:
- **Claim key mismatch**: Python signs `"un"` (`bot.py:72`) whereas Rust reads `"username"` (`ws_game.rs:225`). If a Python-issued token is ever verified by Rust, `token_name` falls back to client-supplied `val["name"]`.
- **Silent `uid = 0` fallback in Rust**: `ws_game.rs:223` defaults missing `uid` to `0`, bypassing duplicate-user checks (`token_uid != 0`) and skipping DB leaderboard writes (`p_uid != 0`).

---

## 2. Message Schemas (Join / Leave / Turn / Word / Hint / Chat)
- **Chat**: Not implemented in Python (`bot.py`), Rust (`ws_game.rs`), or Web (`[roomId].jsx`).
- **`ping` / `pong`**: Implemented in Python (`bot.py:3841-3842`); **missing in Rust** (`ws_game.rs:516`); unused by Web.
- **`joined` & `state`**:
  - Python (`bot.py:3610-3614, 3818-3820`) includes `history` (last 20) and `your_name`.
  - Rust (`ws_game.rs:74-85, 270-280`) omits `history` and `your_name`.
- **`player_joined`**:
  - Python (`bot.py:3821-3822`) broadcasts `{type, name, count, is_host}` to all.
  - Rust (`ws_game.rs:284-285`) broadcasts `{type, name}` via `broadcast_except` (omits `count`, `is_host`).
- **`checking`**:
  - Python (`bot.py:3739`) sends `{type: "checking", word}` only to the submitting `ws`.
  - Rust (`ws_game.rs:361`) broadcasts `{type: "checking"}` to **all** players. Web (`[roomId].jsx:87-88`) sets `checking = true` for all clients on receipt.
- **`word_accepted`**:
  - Python (`bot.py:3759-3760`) sends cumulative `player["score"]`.
  - Rust (`ws_game.rs:443-449`) hardcodes `"score": 1`. Web logs `(+${d.score})` (`[roomId].jsx:86`), causing inconsistent UI logs.
- **Invalid word / format / turn errors**:
  - Python (`bot.py:3696-3697, 3719-3729`) returns `{type: "error", msg}` **without** deducting a life for wrong turn, wrong starting syllable (with continuation hints), length `< 2`, special chars, or `> 4` words.
  - Rust (`ws_game.rs:355-357, 392-433`) silently ignores wrong-turn/non-host `start`/`restart`, and treats wrong starting syllable or invalid chars as `word_invalid` (**deducting 1 life**).
- **`hint` / `hint_result` / `hint_limit_reached`**:
  - Both enforce 3 hints/game. Python returns up to 8 words (`bot.py:3779`) and includes `hints_left: 0` in `hint_limit_reached` (`3774`); Rust returns up to 5 words (`ws_game.rs:485`) and omits `hints_left` in `hint_limit_reached` (`482`).

---

## 3. Turn Timers & Timeouts
- **Python (`bot.py:3650-3666`)**: Server-side `asyncio.ensure_future(self._run_timer())` sleeps `self.time_limit` (10–120s). On timeout, decrements `lives`, broadcasts `{type: "timeout", player, lives}`, eliminates at `<= 0`, else advances `cur_idx`, stamps turn, broadcasts `state`, and reschedules.
- **Rust (`ws_game.rs:47`)**: **CRITICAL GAP — No server-side turn timer exists.** `WsRoom.time_limit` is hardcoded to `30` (ignoring `public_rooms` / room config) and no timeout task is spawned. An AFK player stalls the game indefinitely.
- **Web (`[roomId].jsx:45-48, 77, 86`)**: Client-side countdown `startTick(limit)` only resets on `game_start` and `word_accepted`. It **does not reset** on `word_invalid`, `word_used`, `timeout`, or `state` turn changes, causing client timer desync.

---

## 4. Anti-Bot Speed Check
- **Python (`bot.py:3632-3637, 3700-3717`)**: `_stamp_turn()` sets `turn_started_at = time.time()`. In `handle_word()`, submissions `< 0.5s` increment `suspect_count`; at `< 3` it sends `{"type": "speed_warning", "count", "msg"}`, and at `>= 3` it sends `error` and calls `_eliminate(ws)`.
- **Rust (`ws_game.rs`)**: **MISSING** — no `turn_started_at`, no speed check, no `speed_warning`.
- **Web (`[roomId].jsx:308`)**: Blocks paste via `onPaste={e => e.preventDefault()}`, but **does not handle** `speed_warning` messages in `ws.onmessage` (`[roomId].jsx:66-128`).

---

## 5. Bot / AI Opponent Behavior
- Neither Python `WebRoom` (`bot.py`), Rust `WsRoom` (`ws_game.rs`), nor Web (`[roomId].jsx`) implements an AI/bot opponent in WS multiplayer rooms.
- Python seeds `history` with `{"word": first_word, "player": "🤖 BOT", "valid": True}` (`bot.py:3644`). Both backends allow solo play (game ends when the solo player loses all 3 lives).

---

## 6. DB Persistence
- **Room State**:
  - Python: In-memory `_WEB_ROOMS` only (`bot.py:3583`); not persisted to `public_rooms`.
  - Rust: `WsRoomsMap` synced to `public_rooms` via `update_public_room_players` (`ws_game.rs:290, 554`) and `close_public_room` (`ws_game.rs:536`).
- **Leaderboard**:
  - Python: Calls `db_lb_add_word(WEB_GUILD_ID, -player["user_id"], player["score"])` (`bot.py:3752`) and `db_lb_add_game(WEB_GUILD_ID, auth_uids, winner_uid)` (`bot.py:3688`), using **negative** `-user_id` for web users.
  - Rust: Calls `ctx.db.lb_add_word(0, p_uid).await` (`ws_game.rs:457`) with **positive** `p_uid` (ID sign mismatch with Python). **CRITICAL GAP**: `ctx.db.lb_add_game` (`db.rs:611-627`) is **never called** in `ws_game.rs`, so `games_played` and `games_won` are never recorded for WS games.

---

## 7. Connection Cleanup on Disconnect
- **Python (`bot.py:3616-3630, 3845-3846`)**: `finally` calls `room._remove(ws)` which pops the player, adjusts `cur_idx`, broadcasts `player_left`, transfers `host_ws` (`new_host`), and cancels `_timer` + removes room if empty.
  - **Bugs**: If a disconnect during `"playing"` leaves `<= 1` player, `_remove` does **not** call `_end_game()`. If the current-turn player disconnects, `_remove` does **not** reschedule `_timer` or broadcast `state_msg()`.
- **Rust (`ws_game.rs:520-558`)**: On loop exit, removes player, broadcasts `player_left`, closes room in `DashMap` + DB if empty, transfers `host_conn_id` (`new_host`), calls `check_game_end()` (properly ends game if `<= 1` alive) or broadcasts `state_payload()`, updates DB player count, and calls `writer_task.abort()`.
  - **Bug**: `players.retain(...)` shifts indices without adjusting `turn_idx` when a player *before* `turn_idx` leaves (skipping a player's turn), and clamping `turn_idx` does not skip eliminated (`lives == 0`) players.

---

## Recommended Tests
1. **Rust Integration Tests (`ws_game.rs`)**:
   - **Turn Timer & Timeout**: Verify turn timeout fires after `time_limit`, decrements `lives`, emits `timeout`, eliminates at `0`, and advances to next alive player.
   - **Disconnect Turn-Index & End-Game**: Test player at index `0` disconnecting while `turn_idx == 1` (ensure player `1` keeps turn), and disconnect leaving 1 alive player triggers `game_end` + `lb_add_game`.
   - **DB Leaderboard Parity**: Assert `lb_add_word` and `lb_add_game` are both called on WS game completion with consistent `user_id` sign convention.
   - **Ping/Pong & Anti-Bot Speed Check**: Test `{"type":"ping"}` -> `{"type":"pong"}` and `< 500ms` submission -> `speed_warning` -> elimination at 3 strikes.
2. **Python Unit/Async Tests (`bot.py` `WebRoom`)**:
   - **Disconnect During Play**: Assert `_remove` triggers `_end_game()` when `len(players) <= 1` and reschedules `_timer` + broadcasts `state_msg()` when `cur_ws` disconnects.
3. **Web Frontend / E2E Tests (`[roomId].jsx`)**:
   - **Timer Reset**: Verify `startTick(timeLimit)` resets on `word_invalid`, `word_used`, `timeout`, and `state` turn change.
   - **Message Handling**: Verify `speed_warning` renders a warning flash and `checking` only locks the active player's input.
