# Forensic Audit Report — Assignment 10

**ID:** `10`  
**Scope:** Full Feature & Lifecycle Parity of Python `twitch_bot.py` & `revolt_bot.py` (`C:\Users\Ai Hoi\Downloads\gdrive-bot-source\gdrive-bot\`) vs Rust `crates\zenne-twitch\src\lib.rs` & `crates\zenne-revolt\src\lib.rs` (`C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\`).  
**Status:** **PARTIAL PARITY / SIGNIFICANT GAPS IN RUST** (Core chat loop and command names exist in Rust, but lifecycle safety, timers, DB score persistence, betting/solo mechanics, and several anti-abuse guards are missing or diverged).

---

## Findings

### 1. IRC / Revolt Reconnects & Connection Lifecycle
- **Twitch Transport & IRCv3 Tags:**
  - Python connects via raw SSL IRC (`irc.chat.twitch.tv:6697`) and requests `CAP REQ :twitch.tv/tags twitch.tv/commands twitch.tv/membership` (`twitch_bot.py:227-235`), enabling `user-id`, `display-name`, and `badges`/`mod` parsing (`twitch_bot.py:317-325,357-358`).
  - Rust connects via WebSocket (`wss://irc-ws.chat.twitch.tv:443`) without `CAP REQ` (`zenne-twitch/src/lib.rs:46-53`), so it lacks Twitch user IDs, display names, and broadcaster/mod badges.
- **Revolt Gateway URL & Startup Identity:**
  - Python queries `GET /users/@me` for `bot_id` and connects to Stoat gateway `wss://events.stoat.chat?format=json` with a 25s ping loop (`revolt_bot.py:84,213-242`). Note: if `/users/@me` fails at startup, Python exits permanently without retry (`revolt_bot.py:223-227`).
  - Rust connects to legacy `wss://ws.revolt.chat` with a 30s ping interval and does not call `/users/@me` (`zenne-revolt/src/lib.rs:49-54`).
- **Reconnect Backoff Bug (Python):**
  - Both Python bots only sleep `5s` inside `except Exception:` (`twitch_bot.py:290-294`, `revolt_bot.py:253-257`). On a clean EOF/close (`break` at `twitch_bot.py:259` or `revolt_bot.py:249`), they bypass the sleep and reconnect in a tight `0s` loop. Rust correctly sleeps `5s` on all disconnects (`zenne-twitch/src/lib.rs:333-334`, `zenne-revolt/src/lib.rs:354-355`).

### 2. Self-Message Filtering
- **Twitch:** Python filters `username.lower() == TWITCH_BOT_NICK.lower()` (`twitch_bot.py:327-328`). Rust has **no** self-message check in `parse_privmsg` or the `PRIVMSG` loop (`zenne-twitch/src/lib.rs:70-72,338-344`).
- **Revolt:** Python filters `author_id == self.bot_id` (`revolt_bot.py:297-298`). Rust has **no** `bot_id` and **no** self-message filter (`zenne-revolt/src/lib.rs:85-88`). Because Revolt broadcasts the bot's own `Message` events over the gateway, Rust can react to its own messages.

### 3. Throttling & Message Framing
- **Twitch:** Python uses an `asyncio.Queue` + `_sender_loop` draining 1 message per `0.8s` and strips `\r`/`\n` (`twitch_bot.py:194-221`). Rust sends `PRIVMSG` directly with **no** rate-limit queue and **no** newline sanitization (`zenne-twitch/src/lib.rs:186-191,325-327`), risking Twitch rate-limit drops and IRC line injection on multi-line `!ask` responses.
- **Revolt:** Neither Python (`revolt_bot.py:156-175`) nor Rust (`zenne-revolt/src/lib.rs:28-36`) throttles REST `POST /messages`; Python logs non-200/201 responses whereas Rust silently discards HTTP status (`let _ = ...`).

### 4. Turn Timeouts & Mini-Game Timers
- **Python:** Enforces `TURN_TIMEOUT` (default 60s) on `noitu` turns (`twitch_bot.py:444-465`, `revolt_bot.py:429-453`) and countdown timers on all 5 mini-games (`vtv` 45s, `quiz` 35s, `cadao` 35s, `chinhta` 30s, `flag` 35s; `twitch_bot.py:577-712`, `revolt_bot.py:556-705`), cancelling them on `!stop` (`cancel_all_timers`).
- **Rust:** **Zero** turn timeouts for `noitu` and **zero** timers for any mini-game (`zenne-twitch/src/lib.rs`, `zenne-revolt/src/lib.rs`). Active games remain open indefinitely until answered or overwritten.

### 5. Wrong-Answer Feedback & Game Mechanics
- **`noitu` (Word Chain):**
  - Python enforces 2–4 words, locks `solo` games to `solo_user_id`, gives wrong-word feedback with up to 3 dictionary suggestions, and awards `+50` bonus + ends game when the bot has no continuation (`twitch_bot.py:470-530`, `revolt_bot.py:458-513`).
  - Rust has **no** `solo` player lock, **no** feedback when `wiki::check_word` fails, and **no** bot-defeat/win handling when `find_continuations` is empty (`zenne-twitch/src/lib.rs:132-194`, `zenne-revolt/src/lib.rs:156-215`).
- **Mini-Games:**
  - `quiz`: Python ignores wrong `A/B/C/D` guesses so other players can answer (`twitch_bot.py:738-748`, `revolt_bot.py:734-744`). Rust **ends the quiz on the first wrong `A/B/C/D` guess** with `"❌ Sai rồi!"` (`zenne-twitch/src/lib.rs:111-128`, `zenne-revolt/src/lib.rs:141-154`).
  - `cadao`, `chinhta`, `flag`: Python provides 4-option A/B/C/D choices for `cadao` and `flag`, and `1`/`2` selection for `chinhta` (`twitch_bot.py:623-716`, `revolt_bot.py:609-715`). Rust omits multiple-choice options for `cadao`/`flag` and expects text answers for `chinhta` (`zenne-twitch/src/lib.rs:300-311`, `zenne-revolt/src/lib.rs:319-330`).
  - Active-game guard: Python prevents starting a mini-game if one is already active (`twitch_bot.py:562-564`, `revolt_bot.py:541-543`); Rust overwrites the active game.

### 6. DB Persistence
- **Scores & Leaderboards:**
  - Python persists all `noitu`, mini-game, `taixiu`, and `baucua` score deltas to `db_game_submit_score("twitch"|"revolt", ...)` and reads them via `!diem` (`db_game_user_score`) and `!top` (`db_game_top`) (`twitch_bot.py:503,733,829,870,956,965`, `revolt_bot.py:485,729,822,867,953,962`).
  - Rust **does not persist any scores** to DB: `noitu` scores live only in in-memory `DashMap` (`r.scores`), `!diem` reads only in-memory `r.scores`, and `!top` queries global `ctx.db.lb_get_top(0, 5)` (`total_words`) rather than platform game scores (`zenne-twitch/src/lib.rs:162,229-243`, `zenne-revolt/src/lib.rs:187,248-262`).
- **Public Rooms (`!taophong`):**
  - Python generates random 6-hex `room_id` and 16-hex `join_token` (`twitch_bot.py:905-922`, `revolt_bot.py:900-916`).
  - Rust generates deterministic `ch_hash & 0xFFFFFF` `room_id` (collides per channel) and `join_token: None` (`zenne-twitch/src/lib.rs:245-261`, `zenne-revolt/src/lib.rs:264-280`).

### 7. Command Parity
- All 17 core commands exist in both implementations, with notable divergences:
  - `!stop`: Python Twitch restricts to `is_mod` or `noitu_host_id` (`twitch_bot.py:544-546`); Rust allows any user to stop (`zenne-twitch/src/lib.rs:225-228`).
  - `!taixiu` / `!baucua`: Python requires a bet side/animal and optional bet amount (5–100) with DB point payout/deduction (`twitch_bot.py:793-880`, `revolt_bot.py:789-880`); Rust `baucua` only rolls 3 faces with no choice/betting, and `taixiu` rolls even with no args and no DB betting (`zenne-twitch/src/lib.rs:281-291`, `zenne-revolt/src/lib.rs:300-310`).
  - **Aliases:** Python has `huongdan`/`lenh`, `play`, `end`, `ai`/`kira`, `point` missing in Rust; Rust has `check` (`tratu`) and `ctk` (`contukhong`) missing in Python.

---

## Recommended Tests
1. **Reconnect Backoff & Auth Failure Test:** Simulate clean EOF/WS close on both Python and Rust Twitch/Revolt clients; verify 5s backoff triggers on clean close (currently fails in Python) and startup `/users/@me` failure retries in `revolt_bot.py`.
2. **Self-Message Loop Test:** Inject a `Message` / `PRIVMSG` event with `author == bot_id` (Revolt) and `nick == bot_nick` (Twitch) containing `!noitu` or an active game answer; verify no self-triggering occurs in Rust.
3. **Twitch Rate-Limit & Newline Framing Test:** Trigger burst replies and multi-line `!ask` output in `zenne-twitch`; verify outbound messages are throttled and `\r`/`\n` are stripped before `PRIVMSG`.
4. **Timeout & Timer Cancellation Test:** Start `noitu`, `vtv`, `quiz`, `cadao`, `chinhta`, `flag` in Rust and Python; verify auto-expiry after timeout and clean cancellation on `!stop`.
5. **Solo Lock & Quiz Wrong-Answer Test:** In `noitu solo`, send a word from a second user (must be ignored); in `quiz`, send a wrong `A/B/C/D` guess followed by the right guess from another user (verify round remains open until correct answer or timeout).
6. **DB Score & Room Persistence Test:** Complete `noitu`, `vtv`, `taixiu`, `baucua`, and `!taophong` in Rust; verify `game_scores` rows update in DB, `!diem`/`!top` reflect persisted scores, and `!taophong` generates unique `room_id` + `join_token`.