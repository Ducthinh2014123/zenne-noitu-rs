# Forensic Audit Report — ID `06`

## Scope
- **Python reference**: `C:\Users\Ai Hoi\Downloads\gdrive-bot-source\gdrive-bot\telegram_bot.py` (`cmd_start`, `cmd_noitu`, `cmd_solo`, `cmd_play`, `cmd_stop`, `cmd_multiplayer`, `cmd_taophong`, `_run_turn_timeout`, `handle_text_message`, and lobby/MP callbacks in `handle_callback_query`).
- **Rust port**: `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-telegram\src\{lib.rs, noitu.rs, state.rs}` (plus `zenne-core/src/{db.rs, wiki.rs}` where invoked).
- **Audit targets**: HTML escaping, deep-link `join_<token>`, DB persistence (`lb_add_game`), dead-end suggestions, and timeout cleanup.

## Status
**PARTIAL / ISSUES FOUND** — Core command parity and most HTML escaping are implemented in Rust, but there are **1 missing DB persistence call (`lb_add_game`)**, **2 token/room lifecycle & timeout-cleanup bugs**, **1 unescaped user-supplied HTML field (`lang`)**, and **minor suggestion/fallback gaps**.

---

## Findings

### 1. DB Persistence (`lb_add_game` Missing in Rust)
- **Finding**: When a multiplayer game ends via turn timeout with 1 survivor (`winner`), Python records the game & win via `db_lb_add_game(game.chat_id, [p["id"] for p in game.players], winner["id"])` (`telegram_bot.py:577`). Rust's `schedule_turn_timeout` calls `game_submit_score("noitu", winner.id, 5)` and `close_public_room(&rc)` (`noitu.rs:694-695`), but **never calls `st_c.app.db.lb_add_game(...)`** (even though `Db::lb_add_game` exists at `zenne-core/src/db.rs:611`). As a result, `games_won` / `games_played` in `/top` (`noitu.rs:1075`) remain `0` for Telegram games.
- **Note**: In `telegram_bot.py:577`, `game.players` has already been mutated by `.pop()` (`560`), so Python only passes `[winner["id"]]` rather than all initial participants.

### 2. Deep-Link `join_<token>` & Stale Token Hijacking
- **Finding**:
  - `st.room_tokens` maps `join_token -> chat_id` (`state.rs:160`), whereas Python maps `join_token -> TelegramGameState` (`telegram_bot.py:145`).
  - Neither `cmd_stop` (`noitu.rs:467-528`), `nt_cancel` (`noitu.rs:1290-1313`), nor `schedule_turn_timeout` (`noitu.rs:673-711`) removes `join_token` from `st.room_tokens`.
  - Consequently, in Rust, if a room is closed/overwritten and a **new** room is later opened in the same `chat_id`, clicking an **old** `join_<token>` link resolves `room_chat_id` via `st.room_tokens` (or DB fallback `get_telegram_token` at `noitu.rs:103`) and joins the **new** room in `st.noitu_rooms.get(&room_chat_id)` (`noitu.rs:109`) because `g.join_token == token` is never verified.

### 3. Timeout Cleanup & Room Overwrite Race
- **Finding**:
  - `cmd_taophong` (`noitu.rs:746-788`) and `MpWizardStep::MaxPlayers` (`noitu.rs:1904-1907`) call `execute_create_public_room` (`noitu.rs:790-900`) **without checking** if `st.noitu_rooms` already has an active game in `chat_id.0` (unlike `cmd_noitu` at `noitu.rs:302-312` and `cmd_solo` at `noitu.rs:385-396`).
  - Overwriting `st.noitu_rooms.insert(chat_id.0, room_arc)` (`noitu.rs:832`) leaves any previous room's `timeout_handle` running; when that orphaned `schedule_turn_timeout` task fires, it calls `st_c.noitu_rooms.remove(&chat_id.0)` (`noitu.rs:650, 678, 696`), deleting the **new** room. (Python shares the same overwrite bug at `telegram_bot.py:684`.)
  - On Solo bot surrender (`noitu.rs:1681-1697`), `st.noitu_rooms.remove(&chat_id.0)` is called without setting `g.active = false` (though the timeout was already cancelled at `noitu.rs:1481`).

### 4. HTML Escaping
- **Rust improvements over Python**:
  - Rust fixes Python's Markdown-in-HTML bug (`**Sảnh Chơi Công Khai**` at `telegram_bot.py:613` -> `<b>Sảnh Chơi Công Khai</b>` at `noitu.rs:74`).
  - Rust escapes `start_word`, `last_syllable`, `word`, `bot_word`, `sugs`, `join_url`, `web_url`, `tele_url`, `room_id`, and `host_name` (`noitu.rs:55, 611-615, 726, 872-878, 1260, 1387-1389, 1492-1507, 1531-1533, 1581-1595, 1633-1718`), all of which were unescaped in Python (`telegram_bot.py:521-525, 768-888, 1531`).
- **Remaining Rust gap**:
  - User-supplied `lang` (from `/taophong <name>|<lang>|<max>` at `noitu.rs:776-780` or `MpWizardStep::Lang` at `noitu.rs:1882`) is interpolated raw via `lang.to_uppercase()` without `html_escape` in `build_lobby_text` (`noitu.rs:49`), `cmd_start` (`noitu.rs:146, 171`), `execute_create_public_room` (`noitu.rs:875`), `mp_list` (`noitu.rs:1391`), and `MpWizardStep::Lang` (`noitu.rs:1898`). (Only `cmd_setlang` at `noitu.rs:1119` escapes it.)

### 5. Dead-End Suggestions & Bot Fallback
- **Finding**:
  - On invalid chain (`noitu.rs:1486`) and unknown word (`noitu.rs:1575`), Rust passes `&[]` for `used_words` to `wiki::find_continuations`, matching Python (`telegram_bot.py:767, 816`), meaning suggestions can include words already used in the current match.
  - In Solo mode (`noitu.rs:1644-1655`), when `find_continuations` returns no unused words, Rust immediately surrenders (`noitu.rs:1681`), whereas Python (`telegram_bot.py:852-855`) has a secondary fallback to `db_suggest_similar(game.lang, game.last_syllable, limit=10)`.

---

## Recommended Tests
1. **Timeout Win Persistence Test**: Start a 2-player multiplayer room, let player 1 time out in `schedule_turn_timeout`, and assert `lb_add_game` increments `games_played` and `games_won` for the winner in DB.
2. **Stale Deep-Link Token Test**: Create room A (`token_A`), cancel/stop it, create room B in the same `chat_id`, then invoke `cmd_start` with `join_<token_A>` and assert it is rejected rather than joining room B.
3. **Active Room Overwrite / Orphaned Timer Test**: Start an active game in `chat_id`, run `/taophong` or complete `MpWizardStep::MaxPlayers` in the same `chat_id`, and verify either creation is blocked while active or the old `timeout_handle` is aborted so it cannot remove the new room.
4. **HTML Escaping Fuzz Test**: Pass `<script>&"` as `lang` in `/taophong` and `MpWizardStep::Lang`, and verify `execute_create_public_room` / `build_lobby_text` / `mp_list` produce valid escaped HTML.
5. **Suggestion Filtering Test**: Play a word `W`, then submit an invalid word on the same syllable and assert `W` is excluded from `find_continuations` suggestions.
