# Audit Report 01: Core Word-Chain Engine & State

- **ID:** `01`
- **Scope:** `bot.py` (`C:\Users\Ai Hoi\Downloads\gdrive-bot-source\gdrive-bot\bot.py`) vs `zenne-discord` (`C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-discord\src\{noitu,lib,extra_cmds}.rs`, cross-checked with `zenne-core\src\db.rs`)
- **Status:** `GAPS IDENTIFIED` (Major behavioral & security parity gaps; several `zenne-core` DB methods exist but are unwired)

## Findings

### 1. `on_message` Word-Chain Loop (`bot.py:2632-2776` vs `noitu.rs:49-178`, `lib.rs:511-543`)
- **Turn enforcement broken in multiplayer:** `bot.py:2649-2650` enforces `len(game.turn_order) > 1 and message.author.id != game.turn_order[game.turn_index]`. `noitu.rs:75-77` only checks `room.bot_mode && !room.players.is_empty() && room.players[0] != uid`; in multiplayer (`bot_mode == false`), any user can play out of turn and `noitu.rs:125-127` auto-appends them to `room.players`.
- **Concurrent word race condition:** `noitu.rs:101-108` drops `room` mutex across `wiki::check_word(...).await` and re-acquires it without re-checking `room.used.contains(&word)` or `room.last_word`. Two simultaneous valid messages can both pass validation before state updates.
- **Missing timeout management:** `bot.py:2651,2671,2678,2697,2726,2776` cancels and reschedules `run_timeout` on every turn and invalid attempt. `noitu.rs` has no turn timer.
- **Missing streak tracking & reset:** `bot.py:2667-2670,2693-2696,2722-2725,2737-2744` tracks `user_streaks`, resets on errors via `db_lb_reset_streak`, and passes `streak_now` to `db_lb_add_word`. `noitu.rs:139` calls `app.db.lb_add_word(guild_id, uid as i64)` with no streak state or reset.
- **Missing input/language pre-checks & error hints:** `bot.py:2647` rejects `len(word) > 80 or "\n" in word` and `bot.py:2689-2698` runs `is_valid_for_lang`. `noitu.rs:60-63,102-103` omits both. `bot.py:2659-2664,2715-2720` provides `find_continuations` hints on `wrong_chain` / `wiki_not_found`, whereas `noitu.rs:88,115` passes `("hint", "")`.
- **Room keying difference:** `bot.py:2637,2905` keys `games` per `guild_id`; `noitu.rs:37,41` keys `NoituManager.rooms` per `channel_id`.
- **Rust-only feature:** `noitu.rs:141-175` implements `bot_mode` (AI opponent), not present in `bot.py`'s Discord loop.

### 2. `run_timeout` (`bot.py:2417-2449` vs `noitu.rs`)
- **Not implemented in Rust:** `bot.py:2417-2449` sleeps `TURN_TIMEOUT` (60s), eliminates the timed-out player, ends the game if `<= 1` player remains (declaring winner + scoreboard), or advances turn and respawns `run_timeout`. `zenne-discord` has no equivalent background timeout task.

### 3. `on_raw_reaction_add` (`bot.py:2614-2628` vs `lib.rs:473-546`)
- **Not implemented in Rust:** `bot.py:2614-2628` listens for `🙋` (`\U0001f64b`) on `game.lobby_msg_id` while `game.joining` is true to add players to `game.join_list`. `lib.rs:479-545` `event_handler` only handles `Ready`, `CacheReady`, and `Message`.

### 4. Lifecycle Commands (`c_start`, `c_thamgia`, `c_batdautro`, `c_stop`, `c_batdaulai`)
- **`c_start` (`bot.py:2898-2934`) & `c_batdautro` (`bot.py:2951-2983`) vs `start` (`lib.rs:52-94`):**
  - `bot.py` separates lobby creation (`c_start`, `joining=True`, checks `in_game_ch`, checks no active/joining game, checks active tournament, adds `🙋` reaction) from game start (`c_batdautro`, requires `host_id` or `is_admin`, `MIN_PLAYERS`, shuffles `turn_order`, starts `run_timeout`).
  - `lib.rs:52-94` aliases `batdautro`, `bdt`, `startgame` onto `start`, skips the lobby phase (`r.active = true` immediately), has no active-game guard (anyone can overwrite a running game), and adds `mode: Option<String>` (`"bot"`).
- **`c_thamgia` (`bot.py:2937-2948`) vs `thamgia` (`extra_cmds.rs:20-40`):**
  - `bot.py` checks `games[gid].joining` and appends to `join_list`. `extra_cmds.rs:24-32` checks `r.active` and appends to `r.players`.
- **`c_stop` (`bot.py:2986-3006`) vs `stop` (`lib.rs:97-104`):**
  - `bot.py:2993-3006` enforces `host_id` or `is_admin`, cancels timeout, posts final scoreboard & word count, and pops the game. `lib.rs:98-104` has **no permission check** (any user can stop), does not check if a game was active, and omits the scoreboard.
- **`c_batdaulai` (`bot.py:3047-3068`) vs `batdaulai` (`extra_cmds.rs:43-63`):**
  - `bot.py:3054-3056` enforces `host_id` or `is_admin` and restarts `run_timeout`. `extra_cmds.rs:44-63` has **no permission check** (any user can reset the current word) and fetches `wiki::random_word` before checking `r.active`.

### 5. `game_channel` Enforcement (`bot.py:2406-2411,2900-2904` vs `extra_cmds.rs:7-17`, `lib.rs:57-94`)
- **Missing enforcement & admin guard:** `bot.py:2406-2411` defines `in_game_ch(ctx)` using `db_get_game_channel(gid)` and enforces it in `c_start` (`bot.py:2900-2904`), while `c_setnoitu` (`bot.py:2785`) requires `is_admin`.
- In Rust, `zenne-core/src/db.rs:302` implements `get_game_channel`, and `extra_cmds.rs:10` (`setnoitu`) calls `set_game_channel`, **but `get_game_channel` is never called anywhere in `zenne-discord`** (`start` and `handle_word_message` work in any channel). `extra_cmds.rs:7` (`setnoitu`) also lacks an admin permission check.

### 6. DB `lb_add_game` & `tournament_add_word` Calls
- **`tournament_add_word`:** `bot.py:2745-2747` calls `db_tournament_add_word(gid, uid)` on every valid word in `on_message`. `zenne-core/src/db.rs:1138` implements `tournament_add_word`, **but it is never called in `noitu.rs` or anywhere in `zenne-discord`**.
- **`lb_add_game`:** `bot.py:1332` defines `db_lb_add_game` (called only in web mode at `bot.py:4710`, not in Discord `run_timeout`/`c_stop`). `zenne-core/src/db.rs:611` implements `lb_add_game`, **but it is never called in `zenne-discord`**.

## Recommended Tests
1. **Multiplayer Turn Enforcement:** Start a non-bot game with 2+ players in `noitu.rs`; verify out-of-turn messages are ignored and `turn_idx` advances strictly in order.
2. **Concurrent Word Race:** Send two simultaneous valid words matching `last_word` to `handle_word_message`; assert only one succeeds and `used` / `round` / `scores` do not double-accept.
3. **Command Authorization:** Verify non-host, non-admin users are rejected by `!stop`, `!batdaulai`, and `!setnoitu`.
4. **`game_channel` Enforcement:** Set `game_channel` via `set_game_channel`; verify `!start` and word-chain messages in another channel are rejected.
5. **Tournament & Game Stats Wiring:** Play a valid word while an active tournament exists in `tournaments`; assert `tournament_add_word` increments `tournament_players.words_scored`. Assert `lb_add_game` is invoked on game end if intended for Discord parity.
6. **Timeout & Lobby Flow (if parity required):** Test 60s `run_timeout` player elimination/winner declaration and `🙋` reaction join (`on_raw_reaction_add`).