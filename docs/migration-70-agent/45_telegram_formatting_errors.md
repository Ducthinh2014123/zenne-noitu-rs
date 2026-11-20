# Forensic Audit Report — ID 45

**Scope:** Telegram command responses, formatting, and error messages audit comparing Python `gdrive-bot-telegram/telegram_bot.py` (1932 lines; identical SHA256 to `gdrive-bot-source/gdrive-bot/telegram_bot.py` and `gdrive-bot-source/gdrive-bot-source/telegram_bot.py`) against Rust `zenne-noitu-rs/crates/zenne-telegram/src/{lib.rs, state.rs, noitu.rs, minigames.rs}`.

---

## 1. `ParseMode::Html` Usage

- **Python (`telegram_bot.py`):** Uses `parse_mode=ParseMode.HTML` across HTML-formatted `reply_text`, `send_message`, and `edit_message_text` calls (including `error_handler` at L1864).
- **Rust (`zenne-telegram`):** All messages containing HTML tags (`<b>`, `<i>`, `<code>`, `<a href="...">`) specify `.parse_mode(ParseMode::Html)`.
  - `lib.rs`: L111 (unknown command fallback).
  - `noitu.rs` & `minigames.rs`: 9 plain-text-only `send_message` calls omit `.parse_mode(ParseMode::Html)` (`minigames.rs:292, 392`; `noitu.rs:459, 471, 534, 541, 547, 1056, 1550`), none of which contain HTML markup.

---

## 2. HTML Escaping Parity

- **Escaper Implementation:**
  - Python uses `html.escape(s)` (escapes `&`, `<`, `>`, `"`, `'`).
  - Rust (`state.rs:12-17`, `html_escape`) escapes `&`, `<`, `>`, `"` (omits `'`, which is acceptable in Telegram HTML mode).
- **Missing `html_escape` on User-Controlled `lang` in Rust:**
  - `noitu.rs:1119` (`cmd_setlang` write path) escapes `&new_lang.to_uppercase()`, proving `lang` can hold arbitrary user text (e.g. `<script>` or `<a>`).
  - However, **every read/display site** of `lang` in Rust omits `html_escape`:
    - `noitu.rs:49` (`build_lobby_text`)
    - `noitu.rs:146, 171` (`cmd_start` join flow)
    - `noitu.rs:875` (`execute_create_public_room` — directly from `/taophong` args or wizard)
    - `noitu.rs:963` (`cmd_tratu`)
    - `noitu.rs:1105` (`cmd_setlang` with no args)
    - `noitu.rs:1391` (`mp_list` callback — `r.lang.to_uppercase()`, whereas `r.name`, `r.room_id`, `r.host_name` are escaped)
    - `noitu.rs:1593` (`handle_text_message` invalid word notice)
    - `noitu.rs:1898` (`MpWizardStep::Lang` — directly from `raw_text`)
  - Impact: If a user enters `<` or `>` in `/taophong`, `/setlang`, or the multiplayer wizard, Telegram's HTML parser will reject the outgoing message with `400 Bad Request: can't parse entities`.
- **Other fields:** User names (`user_mention` in `state.rs:19-25`), `start_word`, `last_syllable`, `word`, `query`, `ans`, `vtv.target`, `q.category`, `q.question`, `correct_opt`, `prompt`, `ct.correct`, `fq.correct`, and `guess` are properly escaped via `html_escape`.

---

## 3. `disable_web_page_preview` / Link Preview Parity

- **Python (`telegram_bot.py`):** Explicitly sets `disable_web_page_preview=True` on **7** calls containing URLs (`join_url`, `web_url`, `wiki_url`):
  - `L296` (`cmd_start` welcome)
  - `L406` (`cmd_noitu` lobby)
  - `L727` (`_execute_create_public_room`)
  - `L1031` (`cmd_tratu` Wikipedia link)
  - `L1465` (`nt_getlink` callback)
  - `L1554, L1563` (`mp_list` callback)
- **Rust (`zenne-telegram`):** **0** occurrences of `disable_web_page_preview` or `link_preview_options` across `lib.rs`, `noitu.rs`, `minigames.rs`, `state.rs`.

---

## 4. 4096-Character Message Splitting

- **Python (`telegram_bot.py`):** **No** 4096-char splitting helper exists. However, in `cmd_ask` (`L1394-1401`), `thinking_msg.edit_text(...)` is wrapped in `try...except Exception as ex`, so if Kira's reply exceeds 4096 chars, the user still receives an error edit.
- **Rust (`zenne-telegram`):** **No** 4096-char splitting helper exists anywhere in `zenne-telegram/src/`.
  - Critical impact in `noitu.rs:1158-1176` (`cmd_ask`): `reply_text` is sent/edited via `let _ = bot.edit_message_text(...)` without chunking or fallback. If AI Kira's escaped response exceeds 4096 characters, Telegram rejects the edit and the error is silently swallowed—leaving the `"🤔 Trợ lý AI Kira đang suy nghĩ câu trả lời..."` placeholder message frozen indefinitely.

---

## 5. Swallowed Telegram API Errors (`let _ =`)

- **Rust (`zenne-telegram`):** **129** Telegram API calls (plus 1 unrelated `let _ = already_used;` at `noitu.rs:1470`) discard `Result` via `let _ =`:
  - `lib.rs`: **1** (`L106`)
  - `minigames.rs`: **49**
  - `noitu.rs`: **79**
  - `state.rs`: **0**

---

## 6. `error_handler` Parity

- **Python (`telegram_bot.py:1856-1866`, registered at `L1917`):** Logs `exc_info=context.error` and notifies the user (`"⚠️ Thao tác gặp sự cố..."` / `"⚠️ Đã xảy ra lỗi..."`).
- **Rust (`lib.rs:144-148`):** `Dispatcher::builder` has **no custom `.error_handler(...)`**, and handlers return `Ok(())` unconditionally after swallowing errors with `let _ =`.
