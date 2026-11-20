# Forensic Audit Report — ID `07`

## Scope
- **Reference (Python):** `C:\Users\Ai Hoi\Downloads\gdrive-bot-source\gdrive-bot\telegram_bot.py` (`cmd_quiz`..`cmd_mygame` at `L1122-1418`, `handle_callback_query` at `L1424-1811`, `post_init_setup` at `L1817-1849`, `error_handler` at `L1856-1866`) & `bot.py` (`db_game_submit_score` at `L1397-1424`).
- **Target (Rust):** `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-telegram\src\{lib.rs,minigames.rs,noitu.rs,state.rs}` & `zenne-core\src\db.rs` (`L653-683`).

## Status
**FAIL (2 Critical/High Logic & Security Bugs, Multiple Parity Gaps, 1 Unused Field)**

---

## Findings

### 1. CRITICAL — `game_submit_score` Uses High-Score (`MAX`) Mode Instead of Cumulative (`+`) Mode
- **Python:** `telegram_bot.py` explicitly passes `cumulative=True` for `noitu` (`L578, L837, L871`), `vuatiengviet` (`L901`), `wordle` (`L926`), `taixiu` (`L1204`), `baucua` (`L1264`), `quiz` (`L1577`), `cadao` (`L1653`), `chinhta` (`L1678`), and `flagquiz` (`L1703`). Only `trieuphu` uses `cumulative=False` (`L1736, L1757`).
- **Rust:** `zenne-core/src/db.rs:653-656` defines:
  ```rust
  pub async fn game_submit_score(&self, game: &str, user_id: i64, score: i64) {
      self.game_submit_score_mode(game, user_id, score, game == "afk").await;
  }
  ```
  All minigame calls in `minigames.rs` (`L344, L472, L738, L866, L898, L927, L980, L1024`) and `noitu.rs` (`L694, L1618, L1682, L1740, L1803`) call `game_submit_score` instead of `game_submit_score_mode(..., true)`. Consequently, `best_score` is never accumulated for any minigame (e.g., `/quiz` stays capped at 10, `/cadao` at 15, `/taixiu` at 50).

### 2. HIGH — `tp_5050`, `tp_aud`, and `tp_stop` Lack Player Ownership Checks
- **Rust:** `minigames.rs:962-969` (`tp_ans:`) checks `uid != tp_ref.player_id`, matching Python (`telegram_bot.py:1727`). However, `tp_5050` (`minigames.rs:1039-1091`), `tp_aud` (`minigames.rs:1093-1122`), and `tp_stop` (`minigames.rs:1124-1142`) do **not** check `uid != tp.player_id`. Any user in a group chat can consume the active player's lifelines or prematurely stop their game.

### 3. MEDIUM — Unused Field (`VtvSession::scrambled`)
- **Rust:** `state.rs:86` defines `pub scrambled: String` on `VtvSession`, populated in `minigames.rs:268`, but never read anywhere (`dead_code` warning if not suppressed).

### 4. MEDIUM — Callback Error Handling & Silent Failures
- **Silent API errors:** `minigames.rs` and `noitu.rs` discard all `bot.send_message`, `bot.edit_message_text`, `bot.edit_message_reply_markup`, and `bot.answer_callback_query` results via `let _ = ...` with no `tracing::warn!`/`error!`, unlike Python's global `error_handler` (`telegram_bot.py:1856-1866`).
- **Stale `qz_ans:` click:** `minigames.rs:734-736` silently answers with an empty toast when `active_quiz` is missing, whereas Python (`telegram_bot.py:1571-1573`) alerts `"Câu hỏi này đã kết thúc!"`.
- **Inaccessible quick-menu messages:** `noitu.rs:1209-1230` (`btn_quick_quiz`, `btn_quick_vtv`, `btn_quick_taixiu`) uses `msg.regular_message()`, which returns `None` for `InaccessibleMessage`, silently dropping the command.

### 5. LOW — Minigame Content & Feature Parity Gaps
- **`cmd_quiz` (`minigames.rs:211-259` vs `telegram_bot.py:1122-1147`):** Rust ignores `level` (1–12) and `khoahoc` subject (`lib.rs:86-94` routes `khoahoc` to `start_general`), hardcodes `+10` points, and omits `explanation`.
- **`cmd_cadao` (`minigames.rs:127-168, 497-530`):** Hardcoded 8 items; omits `meaning` on answer.
- **`cmd_chinhta` (`minigames.rs:532-556`):** Omits `explanation` on answer.
- **`cmd_flag` (`minigames.rs:170-209, 558-591`):** Hardcoded 10 flags; omits `capital`, `continent`, and `clue`.
- **`cmd_trieuphu` (`minigames.rs:992`):** Walks `TRIEUPHU_BANK[0..14]` in fixed sequential order every game instead of randomizing per tier.
- **`cmd_wordle` (`minigames.rs:654-678`):** Uses 5-letter English words instead of 5-letter unaccented Vietnamese words (`_WORDLE_5_LETTERS_VI`).
- **`cmd_vtv` (`minigames.rs:776`):** `vtv.target.chars().next()` does not `.trim()` before taking the first character.
- **`cmd_mygame` (`minigames.rs:680-715`) & `post_init_setup` (`lib.rs:13-46, 132-136`):** Full parity (same 8 games in `cmd_mygame`—both omit `flagquiz`/`wordle`—and same 24 `BotCommand`s registered at startup).

---

## Recommended Tests
1. **DB Cumulative vs High-Score Test (`zenne-core/src/db.rs`):** Submit two `quiz` scores (`10`, `10`) and two `trieuphu` scores (`2_000_000`, `0`) for the same `user_id`; assert `quiz` `best_score == 20` (`plays == 2`) and `trieuphu` `best_score == 2_000_000` (`plays == 2`).
2. **TrieuPhu Authorization Test (`zenne-telegram`):** Start `/trieuphu` as user A, dispatch `tp_5050`, `tp_aud`, and `tp_stop` callbacks as user B; assert lifelines remain `true` and session remains active.
3. **Quiz / Minigame Stale Callback Test:** Click `qz_ans:0` twice in a row; verify the second click does not panic and returns an informative alert.
4. **Compiler Warning Check:** Run `cargo clippy -p zenne-telegram` in CI to catch unused fields (`VtvSession::scrambled`) and ignored `Result`s.
