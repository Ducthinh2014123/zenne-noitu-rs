# Forensic Audit Report — ID 13

**Scope:** Minigame parity audit for `wordle`, `anagram`, `catword`, `tamsao` across Python (`gdrive-bot`) and Rust (`zenne-games`, `zenne-discord`, `zenne-telegram`, `zenne-twitch`, `zenne-revolt`).

---

## 1. Platform Coverage Matrix

| Minigame | Py Discord | Rs Discord | Py Telegram | Rs Telegram | Py/Rs Twitch & Revolt |
| :--- | :---: | :---: | :---: | :---: | :---: |
| **wordle** | ✅ (`wordle_system.py`) | ✅ (`zenne-discord/src/lib.rs:358`) | ✅ (`telegram_bot.py:914,1368`) | ✅ (`zenne-telegram/src/minigames.rs:654`, `noitu.rs:1761`) | ❌ Not implemented |
| **anagram** | ✅ (`anagram_game_system.py`) | ⚠️ Divergent (`zenne-discord/src/lib.rs:390`) | ❌ | ❌ | ❌ |
| **catword** | ✅ (`catword_system.py`) | ⚠️ Divergent (`zenne-discord/src/extra_cmds.rs:634`) | ❌ | ❌ | ❌ |
| **tamsao** | ✅ (`tamsao_system.py`) | ⚠️ Divergent (`zenne-discord/src/extra_cmds.rs:658`) | ❌ | ❌ | ❌ |

---

## 2. Detailed Game-by-Game Parity Findings

### A. `wordle`
* **Word Lists (VI vs EN):**
  * **Py Discord** (`wordle_system.py:21-43, 128-131`): Supports `en` (`_WORDLE_WORDS_EN`, 45 × 5-letter EN words) and `vi` (`_WORDLE_5_LETTERS_VI`, 38 unaccented VI words). `_WORDLE_WORDS_VI` (`L21-27`) is dead/unused code. **Bug:** `_WORDLE_5_LETTERS_VI` contains 3-, 4-, and 6-letter words (`"NAM"`, `"BAP"` = 3; `"NGHI"`, `"THOI"`, `"VIET"`, etc. = 4; `"THUONG"`, `"CHUYEN"`, `"NGHIEN"` = 6), violating the 5-letter promise.
  * **Py Telegram** (`telegram_bot.py:1370, 1379`): Hardcodes `_WORDLE_5_LETTERS_VI` only (even though `_WORDLE_WORDS_EN` is imported at `L100`) and tells users the word has 5 letters even when a 3/4/6-letter word is picked.
  * **Rs Core / Discord / Telegram** (`zenne-games/src/wordle.rs:5-10`): `WORDLE_WORDS` has 40 × 5-letter EN words only. **Zero VI support.**
* **Guess Validation:**
  * **Py Discord** (`wordle_system.py:163-165`): Checks `len(guess) != len(self.target_word)` only (no alphabetic/dictionary check).
  * **Py Telegram** (`telegram_bot.py:917`): Checks `len(guess) == len(wd["target"])` only; silently ignores wrong-length messages.
  * **Rs Core** (`wordle.rs:46-49`): Enforces `guess.len() == 5 && guess.chars().all(|c| c.is_ascii_alphabetic())`, returning `Err(...)` otherwise.
  * **Rs Telegram** (`zenne-telegram/src/noitu.rs:1763`): Same ASCII-alpha + length check, silently ignores invalid chat messages. *(Note: `noitu.rs:1764-1798` duplicates tile/session logic instead of calling `WordleManager::guess`.)*
* **Green/Yellow/Gray Tile Logic:**
  * **Parity:** Both Py (`_evaluate_guess`, `wordle_system.py:46-67`) and Rs (`wordle.rs:54-76`, `noitu.rs:1764-1785`) implement standard 2-pass Wordle evaluation (Pass 1: exact `🟩` + consume target char; Pass 2: misplaced `🟨` + consume target char, else `⬛`). Duplicate letters are handled accurately in all versions.
* **Attempts & Timeouts:**
  * `max_attempts = 6` everywhere.
  * **Timeout:** Py Discord `WordleView` (`wordle_system.py:121`) has `timeout=180`s. Py Telegram, Rs Discord, and Rs Telegram have **no timeout**.
* **Scoring:**
  * **Py Discord** (`wordle_system.py:175, 179`): `max(20, (7 - attempts) * 20)` (20–120 pts, matches `bot.py:200-202` sanity cap of 120) → `db_game_submit_score("wordle", ...)`.
  * **Py Telegram** (`telegram_bot.py:925-926`): `max(10, (7 - attempts) * 10)` (10–60 pts) → `db_game_submit_score`.
  * **Rs Telegram** (`noitu.rs:1802-1803`): `((7 - attempts) * 10).max(10)` (10–60 pts) → `st.app.db.game_submit_score`.
  * **Rs Discord** (`zenne-discord/src/lib.rs:358-387`): **Missing score submission completely.**
* **UI / Commands:**
  * **Py Discord** (`wordle_system.py:70-139, 248-297`): `/wordle [en|vi]` & `!wordle [en|vi]`, author-locked `WordleView` with `WordleGuessButton`, `WordleGiveUpButton`, and `WordleGuessModal`.
  * **Rs Discord** (`zenne-discord/src/lib.rs:357-387`): Text-only `/wordle [guess]` & `!wordle [guess]`, channel-shared, no modal/buttons/give-up.
  * **Py & Rs Telegram:** `/wordle` & `/doantu`, free-text guesses in chat.

---

### B. `anagram` — **Complete Mechanic Divergence**
* **Py Discord** (`anagram_game_system.py`): **60s multiplayer letter-bag word-forming game** ("Đổi Chữ Lấy Điểm").
  * Uses 6 × 7-letter banks (`_SAMPLE_LETTER_BANKS`, `L14-21`).
  * Validation (`L130-144`): `len >= 2`, no spaces, not in `used_words`, multiset subset of `bank["letters"]`. **No dictionary check**; comment `# Bỏ dấu cơ bản để so sánh` (`L138`) is unimplemented (so accented VI input fails against ASCII bank letters).
  * Scoring & Timeout (`L53, L76, L147-149`): 60s (`asyncio.sleep(60)`), `len(word) * 10` pts per word, submitted to DB at round end.
  * UI/Commands (`L97-117`): `/anagram`, `!anagram` (aliases `doichu`, `ghepchu`), passive `on_message` listener with `✅` reaction.
* **Rs Core / Discord** (`zenne-games/src/anagram.rs`, `zenne-discord/src/lib.rs:390-410`): **Single-answer phrase unscramble** ("Đảo Chữ").
  * Picks 1 of 12 VI phrases + hint (`ANAGRAM_BANK`, `L5-18`), shuffles non-whitespace chars (`L39-46`).
  * Validation (`check_answer`, `L61-72`): Exact case-insensitive match (`g == sess.answer.to_lowercase()`), diacritic-sensitive.
  * **Missing:** No timeout, no scoring/DB submit, no `on_message` listener (requires `/anagram <guess>` or `!anagram <guess>` per attempt).

---

### C. `catword` — **Complete Mechanic Divergence**
* **Py Discord** (`catword_system.py`): **90s multiplayer Categorized Word Chain** ("Nối Từ Theo Chủ Đề").
  * 4 categories (`animals`, `countries`, `food`, `colors` in `_CATEGORIES`, `L14-61`).
  * Validation (`L185-196`): `len <= 50`, `_get_first_syllable(word) == game["last_syllable"]`, not in `used_words`. **Does not validate category membership or dictionary validity.**
  * Scoring & Timeout (`L115, L137, L201`): 90s (`asyncio.sleep(90)`), `+15` pts per word, submitted to DB at round end.
  * UI/Commands (`L155-174`): `/catword`, `!catword [cat]` (aliases `noitu_cd`, `noituchude`, `ntcd`), passive `on_message` listener.
* **Rs Core / Discord** (`zenne-games/src/catword.rs`, `zenne-discord/src/extra_cmds.rs:634-654`): **Masked-word guessing** ("Cắt Chữ").
  * Picks 1 of 8 VI phrases (`CATWORD_BANK`, `L5-14`), reveals chars where `c == ' ' || i == 0 || i % 3 == 0`, masks rest with `_` (`L34-44`).
  * Validation (`check_answer`, `L54-64`): Exact case-insensitive match.
  * **Missing:** No timeout, no scoring/DB submit, no `on_message` listener (requires `/catword <guess>` or `!catword <guess>`).

---

### D. `tamsao` — **Complete Mechanic Divergence**
* **Py Discord** (`tamsao_system.py`): **35s Chain Translation Comedy** ("Tam Sao Thất Bản").
  * 6 hardcoded 5-step translation chains (`_TSB_ROUNDS`, `L15-88`) with garbled VI output.
  * Validation (`L170`): `any(ans in content or content in ans for ans in answers)`. **Bug:** `content in ans` allows any 1-character substring (e.g., `"a"`) to win immediately.
  * Scoring & Timeout (`L122, L173, L177`): 35s (`asyncio.sleep(35)`), `max(20, int(100 - elapsed * 2))` pts submitted to DB.
  * UI/Commands (`L137-155`): `/tamsao`, `!tamsao` (aliases `tamcao`, `dich`, `chaintranslate`), passive `on_message` listener.
* **Rs Core / Discord** (`zenne-games/src/tamsao.rs`, `zenne-discord/src/extra_cmds.rs:658-678`): **Verbatim phrase typing test**.
  * Picks 1 of 6 VI proverbs (`PHRASES`, `L5-12`) and displays it directly; user must type it back verbatim (`check`, `L39-48`).
  * **Missing:** No timeout, no scoring/DB submit, no `on_message` listener (requires `/tamsao <typed>` or `!tamsao <typed>`).

---

## 3. Recommended Tests

Currently `zenne-games/src/lib.rs:93-95` only has a 3-line win smoke test for `wordle` and **zero tests** for `anagram`, `catword`, `tamsao`. Add:

1. **Rust `zenne-games` Unit Tests (`wordle.rs`, `anagram.rs`, `catword.rs`, `tamsao.rs`):**
   * `wordle_duplicate_letter_tiles`: Verify target `"APPLE"` vs `"PAPER"` → `"🟨🟨🟩⬛⬛"` and `"PIZZA"` → `"🟨⬛⬛⬛⬛"` (no over-counting `🟨`).
   * `wordle_validation_and_exhaustion`: Reject `<5`/`>5` chars and non-ASCII (`"ÁPPLE"`); verify 6 wrong guesses set `game_over == true` and remove session from `DashMap`.
   * `anagram_shuffle_and_check`: Assert `scrambled` has same non-space char multiset as `answer`, `check_answer` is case-insensitive and removes session only on `true`.
   * `catword_masking_and_check`: Assert `masked` preserves spaces and indices `i % 3 == 0`; `check_answer` preserves session on `false` and clears on `true`.
   * `tamsao_exact_match_and_cleanup`: Assert partial/substring input returns `(false, orig)` and retains session; exact trimmed match returns `(true, orig)` and clears session.
2. **Python Regression / Data Audit Tests (`pytest`):**
   * `test_wordle_vi_word_lengths`: Assert every entry in `_WORDLE_5_LETTERS_VI` has `len(w) == 5` (currently fails on 24/38 entries).
   * `test_evaluate_guess_duplicates`: Verify `_evaluate_guess("APPLE", "PAPER")` and `_evaluate_guess("PIZZA", "ZZZZZ")`.
   * `test_tamsao_no_single_char_exploit`: Verify 1-char message `"a"` does not match `_TSB_ROUNDS` answers once `content in ans` is fixed.
3. **Integration / Parity Tests:**
   * Verify `zenne-discord` submits scores to DB on `wordle` win (and `anagram`/`catword`/`tamsao` once scoring is added), and verify `zenne-telegram` delegates to `WordleManager::guess` instead of duplicating tile logic.
