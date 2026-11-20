# Forensic Audit Report — ID 12

**Scope:** Parity audit of `vuatiengviet`, `cadao`, `chinhta`, and `flag_quiz` between Python (`gdrive-bot`) and Rust (`zenne-games`, `zenne-discord`, `zenne-telegram`, `zenne-twitch`, `zenne-revolt`).

---

## 1. Executive Summary
Rust `zenne-games` implements stripped-down, stateless-timeout, text-only versions of all four minigames with **major parity gaps** against Python:
1. **DB Persistence & Cumulative Scoring Broken/Missing:** `zenne-discord`, `zenne-twitch`, and `zenne-revolt` never persist scores. `zenne-telegram` calls `AppDb::game_submit_score` (`crates/zenne-core/src/db.rs:653-656`), which hardcodes `cumulative = (game == "afk")` (`false` for all 4 games), causing `MAX(best_score, ...)` instead of cumulative addition (`db.rs:677`)—contradicting Python (`cumulative=True`) and Telegram's own `"+20 điểm tích lũy!"` UI (`noitu.rs:1748`).
2. **Discord UX & Input Mode:** Python uses `on_message` listeners (`vuatiengviet`, `cadao`) + interactive `discord.ui.Button` views (`chinhta`, `flag_quiz`) + rich embeds. `zenne-discord` uses `ctx.say()` plain text, requires `!cmd <guess>` arguments, has no `FullEvent::Message` listener for these games (`zenne-discord/src/lib.rs:511-543`), and overwrites active rounds on bare `!cmd`.
3. **Zero Timeouts & Hints in Core:** `zenne-games` has no round timers, no hint system, and no answer-masking/explanation metadata.

---

## 2. Detailed Parity Matrix

### A. Question Banks / Items
| Game | Python (`gdrive-bot`) | Rust (`zenne-games` & `zenne-telegram`) | Gap |
| :--- | :--- | :--- | :--- |
| `vuatiengviet` | `_VTV_WORDS` (`vuatiengviet_system.py:20-48`): **88** items (words + multi-word idioms). | `VTV_QUESTIONS` (`zenne-games/src/vuatiengviet.rs:5-18`): **12** 2-syllable words; pre-scrambled tuple field `.0` is unused (`line 34`). | -76 items; no idioms/phrases. |
| `cadao` | `_CADAO_DATABASE` (`cadao_game_system.py:14-135`): **20** dicts (`masked`, `answer`, `full`, `meaning`). | `CADAO_BANK` (`zenne-games/src/cadao.rs:5-27`): **12** tuples (`prompt`, `answer`). `zenne-telegram/src/minigames.rs:127-168` bypasses `zenne-games` with its own **8**-item `CADAO_CHOICES`. | -8/-12 items; missing `full` & `meaning`. |
| `chinhta` | `_CHINHTA_DATABASE` (`chinhta_game_system.py:13-86`): **12** 4-option MCQs (`question`, `options`, `correct`, `explanation`). | `CHINHTA_BANK` (`zenne-games/src/chinhta.rs:5-18`): **12** 2-option tuples (`right`, `wrong`, `correct`). Only 1 item (`Chẩn đoán`) overlaps with Python. | 2 options vs 4; different bank; missing `explanation`. |
| `flag_quiz` | `_COUNTRIES_DATA` (`flag_quiz_system.py:13-119`): **15** dicts (`name`, `flag`, `capital`, `continent`, `clue`). | `FLAG_BANK` (`zenne-games/src/flag_quiz.rs:5-22`): **16** tuples (`flag`, accepted aliases). `zenne-telegram/src/minigames.rs:170-209` bypasses `zenne-games` with **10**-item `FLAG_CHOICES`. | Missing `capital`, `continent`, `clue`; bank mismatch (`Ai Cập` missing in Rust; `Trung Quốc`/`Singapore` absent in Python). |

### B. Scrambling, Hint & Answer Logic
- **`vuatiengviet` Scrambling:**
  - Python `scramble_vietnamese()` (`vuatiengviet_system.py:54-66`) shuffles characters **per word** (up to 10 retries to avoid identity) and joins words with `"  •  "`.
  - Rust `VuaTiengVietManager::start()` (`vuatiengviet.rs:32-51`) strips all spaces (`filter(|c| !c.is_whitespace())`), shuffles globally without an identity check, and joins with `" / "`, destroying word boundaries.
- **Hints & Stop/Giveup:**
  - Python `VTVHintButton` (`vuatiengviet_system.py:69-98`) grants up to 2 hints revealing random unrevealed characters + masked template + length; `VTVStopButton` (`lines 101-114`) ends the round and reveals the word.
  - Rust `zenne-games`, Discord, Twitch, and Revolt have **no hints or stop buttons**. Telegram (`minigames.rs:274-275, 774-803`) has `vtv_hint` (unlimited clicks, only shows first char) and `vtv_giveup`.
- **Answer Normalization & Leakage:**
  - Python `vuatiengviet_system.py:204` and `cadao_game_system.py:216` collapse internal whitespace (`" ".join(content.split())`). Rust `zenne-games` (`vuatiengviet.rs:55`, `cadao.rs:56`, `chinhta.rs:51`, `flag_quiz.rs:40`) only calls `.trim().to_lowercase()`, failing on double spaces (though Telegram VTV at `noitu.rs:1732-1736` collapses whitespace).
  - Rust Discord `chinhta` (`zenne-discord/src/extra_cmds.rs:401`) leaks the correct answer on a wrong guess without closing the session.
  - Rust Telegram `cadao`/`chinhta`/`flag` (`minigames.rs:862, 893, 924`) remove the active session on the first wrong click (1-strike), whereas Python `ChinhTaButtonView` (`chinhta_game_system.py:149-150`) and `FlagChoiceView` (`flag_quiz_system.py:181-182`) keep the round open until a correct answer or timeout.

### C. Scoring, Cumulative Mode & DB Persistence
- **Python (`bot.py:1397-1424` `db_game_submit_score`):**
  - All 4 games use speed-based scoring (`max(20|25, int(100 - elapsed * 2|2.5))`) and call `db_game_submit_score(..., cumulative=True)` (`vuatiengviet_system.py:216`, `cadao_game_system.py:226`, `chinhta_game_system.py:123`, `flag_quiz_system.py:154`).
- **Rust:**
  - `zenne-discord` (`lib.rs:300-321`, `extra_cmds.rs:370-462`), `zenne-twitch` (`lib.rs:76-109`), and `zenne-revolt` (`lib.rs:91-139`): **no `game_submit_score` calls**; no points awarded.
  - `zenne-telegram` (`noitu.rs:1740`, `minigames.rs:866, 898, 927`): awards flat points (`20` for VTV, `15` for others) via `st.app.db.game_submit_score(...)`, which delegates to `game_submit_score_mode(..., game == "afk")` (`zenne-core/src/db.rs:653-656`), silently forcing `cumulative = false` (`MAX` instead of `+`).

### D. Timeouts
- **Python:** `vuatiengviet` 30s round / 40s view (`vuatiengviet_system.py:118, 159`); `cadao` 40s (`cadao_game_system.py:167`); `chinhta` 30s (`chinhta_game_system.py:93`); `flag_quiz` 30s (`flag_quiz_system.py:126`). Expired rounds auto-pop and reveal answers.
- **Rust:** **No timeouts** in `zenne-games` or any platform crate (`zenne-discord`, `zenne-telegram`, `zenne-twitch`, `zenne-revolt`). Sessions remain in `DashMap` until answered or overwritten.

### E. UI / Formatting
- **Python Discord:** Rich `Embed`s with colors, footers, user thumbnails, `meaning`/`explanation`/`capital`/`continent`/`clue`, and `discord.ui.Button`s.
- **Rust Discord:** Plain `ctx.say(...)` strings (`lib.rs:315-319`, `extra_cmds.rs:384-460`); no embeds or buttons.
- **Rust Telegram:** HTML + `InlineKeyboardMarkup` (`minigames.rs:273-285, 515-590`), matching Python `telegram_bot.py` layout, minus `meaning`/`explanation`/`clue` fields.
- **Rust Twitch & Revolt:** Single-line chat prompts with passive chat-message matching (`zenne-twitch/src/lib.rs:77-109`, `zenne-revolt/src/lib.rs:92-139`).

### F. Command Aliases
| Game | Python Discord | Rust Discord | Rust Telegram | Rust Twitch / Revolt |
| :--- | :--- | :--- | :--- | :--- |
| `vuatiengviet` | `/vuatiengviet`, `!vuatiengviet`, `!vtv`, `!xapchu` (`vuatiengviet_system.py:173,185`) | `vtv` only (`lib.rs:300`) | `vtv`, `vuatiengviet` (`lib.rs:95`) | `vtv` only |
| `cadao` | `/cadao`, `!cadao`, `!tucngu`, `!thanhngu` (`cadao_game_system.py:187,199`) | `cadao` only (`extra_cmds.rs:369`) | `cadao`, `tucngu` (`lib.rs:98`) | `cadao` only |
| `chinhta` | `/chinhta`, `!chinhta`, `!soiloict`, `!ct` (`chinhta_game_system.py:193,205`) | `chinhta` only (`extra_cmds.rs:393`) | `chinhta`, `soiloichinhta` (`lib.rs:99`) | `chinhta` only |
| `flag_quiz` | `/flagquiz`, `!flagquiz`, `!co`, `!doanco`, `!flag` (`flag_quiz_system.py:234,246`) | `flag` only (`extra_cmds.rs:441`) | `flag`, `flagquiz`, `quocky` (`lib.rs:100`) | `flag` only |

---

## 3. Recommended Tests
Currently, `zenne-games` has **zero unit tests** for `vuatiengviet.rs`, `cadao.rs`, `chinhta.rs`, and `flag_quiz.rs` (`lib.rs:73` is the only `#[test]` in the crate). Add:
1. **`zenne-core/src/db.rs`:** `test_game_submit_score_cumulative_minigames` — assert submitting `20` then `25` for `"vuatiengviet"`, `"cadao"`, `"chinhta"`, `"flagquiz"` yields `best_score == 45` and `plays == 2` (currently fails due to `game == "afk"`).
2. **`zenne-games/src/vuatiengviet.rs`:**
   - `test_scramble_preserves_word_boundaries_and_differs` — verify multi-word phrases keep word boundaries and scrambled output != original for len > 1.
   - `test_check_answer_normalizes_whitespace_and_case` — verify `"  hẠnh   phúc  "` matches `"hạnh phúc"` and removes the session.
3. **`zenne-games/src/chinhta.rs`:** `test_wrong_guess_keeps_session_and_case_insensitive` — verify wrong guess returns `Some((false, _))` without deleting session, and correct guess removes it.
4. **`zenne-games/src/flag_quiz.rs`:** `test_flag_aliases_and_case_insensitive` — verify Vietnamese, unaccented, and English aliases (e.g., `"Việt Nam"`, `"vietnam"`, `"VIET NAM"`) all pass.
5. **`zenne-discord/src/lib.rs`:** Extend `test_existing_command_registry_preserved` (`lines 685-738`) to assert Discord command aliases (`vuatiengviet`, `xapchu`, `tucngu`, `thanhngu`, `soiloict`, `ct`, `flagquiz`, `co`, `doanco`) once added.
