# Forensic Audit Report — ID 11

**Scope:** Minigame parity audit for `quiz`, `quizmath`, `quiz_science`, and `wiki_quiz` across Python (`C:\Users\Ai Hoi\Downloads\gdrive-bot-source\gdrive-bot\`) and Rust (`C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\`).

---

## 1. Executive Summary & Parity Matrix

| Feature / Minigame | Python (`gdrive-bot`) | Rust (`zenne-noitu-rs`) | Parity Status |
| :--- | :--- | :--- | :--- |
| **`quiz` (General / Mixed)** | Procedural Math + Science (`quiz_engine.py`) | 8 static trivia questions (`GENERAL_QUIZ`, `quiz.rs:14-63`) | **Partial / Divergent** |
| **`quizmath` (`toan`)** | 16 procedural modes across 3 grade levels (`quiz_math.py`) | 1 simple mode (`+`, `-`, `×` on 2-digit ints, `quiz.rs:86-110`) | **Major Gap** |
| **`quiz_science` (`khoahoc`)** | 15 procedural modes across 3 grade levels (`quiz_science.py`) | **Missing** (no `start_science`; `/khoahoc` falls back to `start_general`) | **Missing** |
| **`wiki_quiz` (`wikiquiz`)** | 8 static masked Wiki definitions (`wiki_quiz_system.py:13-62`) | **Missing** (only metadata entry in `zenne-api/src/lib.rs:433`) | **Missing** |

---

## 2. Detailed Comparison

### 2.1 Question Banks & Counts
- **Python:**
  - `quiz_engine.py:16-40` (`get_quiz_question`): Dispatches to `ProceduralMath` (`quiz_math.py:15-335`, 16 modes: 5 L1 + 5 L2 + 6 L3) and `ProceduralScience` (`quiz_science.py:18-324`, 15 modes: 4 L1 + 6 L2 + 5 L3).
  - `wiki_quiz_system.py:13-62` (`_WIKI_DEFINITIONS`): 8 static entries (Internet, Mưa, Quang hợp, Hố đen, Đồng, DNA, Bóng đá, Sa mạc Sahara).
- **Rust:**
  - `zenne-games/src/quiz.rs:14-63` (`GENERAL_QUIZ`): 8 static questions.
  - `zenne-games/src/quiz.rs:86-110` (`QuizManager::start_math`): Single-tier `+`, `-`, `×` generator (`a ∈ [10,99]`, `b ∈ [2,30]`). No division, algebra, geometry, calculus, or science generators.
  - `zenne-api/src/lib.rs:433`: Lists `"wikiquiz"` in `/api/games` metadata, but no backend/bot implementation exists.

### 2.2 Level & Subject Filtering
- **Python:**
  - Discord (`quiz_system.py:206-274`): `/quizz [mon] [cap]`, `!quizz`, `!toan [cap]`, `!khoahoc [cap]` with `subject ∈ {toan, khoahoc}` and `level ∈ {1, 2, 3}`.
  - Telegram (`telegram_bot.py:1122-1131`): `/quiz [toan|khoahoc] [1|2|3]`.
  - **Python Bug Found:** `twitch_bot.py:594-596` and `revolt_bot.py:577-579` call `get_quiz_question("vi")` (defaults to `toan` L1) and read `q.get("correct", 0)` instead of `q["correct_index"]`, always marking option `A` as correct.
- **Rust:**
  - `QuizManager` (`quiz.rs:71-110`): Only `start_general` and `start_math`; no `level` or `science` support.
  - Discord (`zenne-discord/src/lib.rs:324-355`): Separate `!quiz [answer]` (general) and `!quizmath` (math); no level/science args.
  - Telegram (`zenne-telegram/src/minigames.rs:211-223`, `zenne-telegram/src/lib.rs:86-94`): Maps `toan`/`math` -> `start_math`, everything else (including `/khoahoc`) -> `start_general`; ignores level argument.
  - Twitch (`zenne-twitch/src/lib.rs:296-299`) & Revolt (`zenne-revolt/src/lib.rs:315-318`): `!quiz` only (`start_general`); no `!quizmath` or args.

### 2.3 Scoring & Cumulative Mode
- **Python:**
  - Discord `quiz` (`quiz_system.py:142-147, 174-175`): 10–40 pts/question × streak multiplier (`x1.5` at ≥3, `x2.0` at ≥5), submitted on stop with `cumulative=False` (High Score).
  - Discord `wikiquiz` (`wiki_quiz_system.py:150-155`): `max(20, int(100 - elapsed * 2))`, `cumulative=True`.
  - Telegram `quiz` (`telegram_bot.py:1576-1577`): `q_data.get("points", 10)`, `cumulative=True`.
  - Twitch (`twitch_bot.py:746`) & Revolt (`revolt_bot.py:742`): `+20` pts under `"twitch"` / `"revolt"`, `cumulative=True`.
- **Rust:**
  - `zenne-core/src/db.rs:653-683` (`game_submit_score`): Sets `cumulative = (game == "afk")`, so `"quiz"` uses `MAX(best_score, score)`.
  - Telegram (`zenne-telegram/src/minigames.rs:738`): Submits fixed `10` pts via `game_submit_score("quiz", uid, 10)` — **non-cumulative**, capping `best_score` at `10` permanently.
  - Discord (`zenne-discord/src/lib.rs:324-355`), Twitch (`zenne-twitch/src/lib.rs:112-128`), Revolt (`zenne-revolt/src/lib.rs:142-154`): **No score submission** on correct answers.

### 2.4 Timers & Timeouts
- **Python:** Discord `QuizView` (`quiz_system.py:82`, 90s view timeout; footer shows 12–35s `time_limit`); Discord `wikiquiz` (`wiki_quiz_system.py:94-111`, 40s `asyncio.sleep`); Twitch (`twitch_bot.py:609-617`, 35s) & Revolt (`revolt_bot.py:592-600`, 35s); Telegram (none).
- **Rust:** **No timers or timeouts** in `zenne-games/src/quiz.rs` or any platform crate.

### 2.5 Button / Callback UI
- **Python:** Discord `QuizView` uses `discord.ui.Button` (`quiz_opt_0..3`, `quiz_next`, `quiz_stop`, `quiz_system.py:18-71`); Telegram uses `InlineKeyboardMarkup` (`qz_ans:0..3`, `qz_stop`, `telegram_bot.py:1137-1139`); `wikiquiz`, Twitch, and Revolt use chat text.
- **Rust:**
  - Discord (`zenne-discord/src/lib.rs:338-352`): **Text-only** (`!quiz <A/B/C/D>`), missing Discord button UI, next/stop buttons, and streak session.
  - Telegram (`zenne-telegram/src/minigames.rs:226-247, 731-772`): Uses `InlineKeyboardMarkup` (`qz_ans:0..3`, `qz_stop`) — matches Python Telegram UI.
  - Twitch & Revolt: Text chat (`A/B/C/D` only; `!a` prefix not stripped unlike Python).

### 2.6 Explanation Display
- **Python:** `ProceduralMath` and `ProceduralScience` include `"explanation"` on every question; displayed on both right/wrong answers in Discord (`quiz_system.py:160-164`) and Telegram (`telegram_bot.py:1578-1592`).
- **Rust:** `QuizQuestion` (`zenne-games/src/quiz.rs:7-12`) lacks `explanation`, `points`, and `time_limit` fields. No Rust platform displays explanations.

### 2.7 Anti-Cheat & Duplicate Answer Guards
- **Python:**
  - Discord `QuizView` (`quiz_system.py:33-35, 124-126`): Author-only interaction lock + `self.answered` flag + disables buttons.
  - Discord `wikiquiz` (`wiki_quiz_system.py:116-118, 148-149`): Blocks concurrent rounds per channel and pops `_ACTIVE_WIKI_QUIZ` on first match (though `ans in content` allows substring false positives).
  - Telegram (`telegram_bot.py:1570-1573, 1594`): Pops `_ACTIVE_QUIZ` on first answer; alerts subsequent clickers.
  - API (`bot.py:193-196`): `_sanity_check_score` caps `"quiz"` at `elapsed * 80 + 500`.
- **Rust:**
  - `QuizManager::answer` (`quiz.rs:112-141`): Removes active question **only on correct answer** (`self.active.remove(&channel_id)`), allowing unlimited brute-force retries in Discord, Twitch, and Revolt.
  - Telegram (`zenne-telegram/src/minigames.rs:734`): Removes `active_quiz` on first click (correct or wrong), preventing duplicate callbacks.
  - No active-round guard on `start_general` / `start_math` (silently overwrites `active` map) and no author-only lock in Discord.

### 2.8 Aliases
- **Python:**
  - Discord `quiz`: `/quizz`, `!quizz`, `!quiz`, `!giaido`, `!dautri`, `!toan`, `!khoahoc`, `!khtn` (`quiz_system.py:206, 233, 248, 262`).
  - Discord `wikiquiz`: `/wikiquiz`, `!wikiquiz`, `!dinhnghia`, `!wiki` (`wiki_quiz_system.py:113, 125`).
  - Telegram: `/quiz`, `/quizz`, `/toan`, `/khoahoc` (`telegram_bot.py:1900`).
  - Twitch & Revolt: `!quiz`.
- **Rust:**
  - Discord: `/quiz`, `!quiz`, `/quizmath`, `!quizmath` (`zenne-discord/src/lib.rs:323-355`). Missing `quizz`, `giaido`, `dautri`, `toan`, `khoahoc`, `khtn`, and all `wikiquiz` aliases.
  - Telegram: `/quiz`, `/quizz`, `/toan`, `/khoahoc` (`zenne-telegram/src/lib.rs:86`).
  - Twitch & Revolt: `!quiz`.

---

## 3. Recommended Tests

1. **Unit Tests (`zenne-games/src/quiz.rs`):**
   - Test `QuizManager::start_math` produces 4 distinct options and `correct_index < 4` across 1,000 iterations.
   - Test `QuizManager::answer` behavior on wrong vs. right answers (verify single-attempt or intended retry semantics) and case-insensitive `A-D`/`1-4`/text matching.
   - Once `start_science`, `level` (1–3), and `WikiQuizManager` are implemented, add tests for every subject/level branch and explanation/points population.
2. **DB Scoring Test (`zenne-core/src/db.rs`):**
   - Verify `game_submit_score_mode("quiz", uid, 10, true)` increments `best_score` cumulatively (fixing Telegram's 10-pt cap) vs. `cumulative=false` for Discord session high-scores.
3. **Platform Integration / Regression Tests:**
   - **Python (`twitch_bot.py`, `revolt_bot.py`):** Test `_cmd_quiz` with `get_quiz_question` output to verify `correct_index` (not `"correct"`) is read and non-A answers are accepted.
   - **Rust (`zenne-telegram`, `zenne-discord`):** Test `/khoahoc` routes to science (not general trivia), `/quiz toan 2` respects level 2, and Discord button interactions enforce author-only + single-answer guards.
