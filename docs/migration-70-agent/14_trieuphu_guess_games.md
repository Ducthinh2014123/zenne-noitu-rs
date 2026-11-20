# Forensic Audit Report — ID 14

## Executive Summary
Parity audit of `trieuphu`, `whoami`, `history_guess`, and `emoji` between the Python reference (`C:\Users\Ai Hoi\Downloads\gdrive-bot-source\gdrive-bot\`) and Rust (`C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\`) reveals **major functional regressions** in `zenne-games`, `zenne-discord`, and `zenne-telegram`, plus **0% parity** in `zenne-twitch` and `zenne-revolt` (none of the 4 minigames exist in either crate).

---

## 1. Platform Coverage Matrix
| Minigame | Python (`gdrive-bot`) | `zenne-games` | `zenne-discord` | `zenne-telegram` | `zenne-twitch` / `zenne-revolt` |
|---|---|---|---|---|---|
| `trieuphu` | `trieuphu_system.py` | `src/trieuphu.rs` | `extra_cmds::trieuphu` (`extra_cmds.rs:591-630`) | `minigames::cmd_trieuphu` (`minigames.rs:593-606`) | Absent |
| `whoami` | `whoami_system.py` | `src/whoami.rs` | `extra_cmds::whoami` (`extra_cmds.rs:487-507`) | Absent | Absent |
| `history_guess` | `history_guess_system.py` | `src/history_guess.rs` | `extra_cmds::history` (`extra_cmds.rs:466-483`) | Absent | Absent |
| `emoji` | `emoji_game_system.py` | `src/emoji.rs` | `extra_cmds::emoji` (`extra_cmds.rs:418-438`) | Absent | Absent |

---

## 2. Detailed Parity Comparison

### A. `trieuphu` (Ai Là Triệu Phú)
- **Question Banks (Tier Structure & Randomization):**
  - **Python** (`trieuphu_system.py:33-103`, `_load_question` at `175-180`): 15-tier dictionary `_TRIEUPHU_QUESTIONS` (tiers `1..=15`, 2–4 questions per tier), selecting `random.choice(q_pool)` for the current `step`.
  - **Rust Core** (`zenne-games/src/trieuphu.rs:19-105`, `start` at `131-143`, `answer` at `145-177`): Flat 15-element slice `QUESTIONS` with **no tier structure**; `QUESTIONS.choose(&mut rand::thread_rng())` picks randomly from the same flat pool at every level, allowing duplicate questions and easy questions at level 15.
  - **Rust Telegram** (`zenne-telegram/src/minigames.rs:34-125`, `cmd_trieuphu` at `593-606`, `tp_ans:` at `992`): Bypasses `zenne-games` with its own flat 15-item `TRIEUPHU_BANK`; **no randomization** — always starts at `TRIEUPHU_BANK[0]` and advances deterministically by index `TRIEUPHU_BANK[level.min(...)]`.
- **Lifelines (50:50, Audience, Call, Swap, Stop):**
  - **Python** (`trieuphu_system.py:166-171`, `use_lifeline` at `294-340`, `stop_game` at `342-366`): Full 4 lifelines (`5050`, `call`, `audience`, `swap`) + `TrieuPhuStopButton` (`139-148`) preserving earned money (`_MONEY_LADDER[current_step - 2]`).
  - **Rust Core & Discord** (`zenne-games/src/trieuphu.rs:125-178`, `zenne-discord/src/extra_cmds.rs:591-630`): **0 lifelines and no stop command**. Pure text `!trieuphu <A/B/C/D>` (any unrecognized input defaults to `D` / index `3` at `extra_cmds.rs:598`).
  - **Rust Telegram** (`zenne-telegram/src/minigames.rs:1039-1142`): Implements `50:50` (`tp_5050`), `audience` (`tp_aud`), and `stop` (`tp_stop`). **Missing** `call` (expert) and `swap` (change question).
- **Player Ownership Checks:**
  - **Python** (`trieuphu_system.py:120-122, 133-135, 145-147`): Enforces `interaction.user.id != view.author_id` on options, lifelines, and stop button.
  - **Rust Core & Discord** (`zenne-games/src/trieuphu.rs:13-17, 145-150`, `extra_cmds.rs:600`): `TrieuPhuSession` stores `player_id`, but `TrieuPhuManager::answer(channel_id, ans_idx)` **never checks `player_id`** — anyone in the channel can answer or hijack the session.
  - **Rust Telegram** (`minigames.rs:962-969`): Checks `uid != tp_ref.player_id` on `tp_ans:`, **but omits ownership checks** on `tp_5050` (`1039-1091`), `tp_aud` (`1093-1122`), and `tp_stop` (`1124-1142`) — any chat member can burn lifelines or stop another player's game.
- **Scoring (High Score vs Cumulative):**
  - **Python** (`trieuphu_system.py:242, 277, 352`; `bot.py:1397`): Calls `db_game_submit_score("trieuphu", self.author_id, amount)` with default `cumulative=False` (high score). Submits on win (150M), wrong answer (safe milestone >0), and stop (>0).
  - **Rust Discord** (`extra_cmds.rs:591-630`): **Never submits scores** to DB.
  - **Rust Telegram** (`minigames.rs:980, 1024`; `zenne-core/src/db.rs:653-656`): Calls `game_submit_score("trieuphu", uid, ...)` (`cumulative=false`, high score) on win (150M) and wrong answer (safe milestone, even when `0`). **Bug:** `tp_stop` (`1124-1142`) does **not** submit the saved prize to DB.
- **Timeouts:**
  - **Python** (`trieuphu_system.py:158`): `TrieuPhuView` has `timeout=180` (180s).
  - **Rust Core / Discord / Telegram**: **No timeout** mechanism; sessions remain in `DashMap` indefinitely.
- **Aliases:**
  - **Python** (`trieuphu_system.py:376, 385`): Slash `/trieuphu`; Prefix `!trieuphu` with aliases `["altp", "millionaire"]`.
  - **Rust Discord** (`extra_cmds.rs:590-591`): `trieuphu` only (**missing** `altp`, `millionaire`).
  - **Rust Telegram** (`zenne-telegram/src/lib.rs:40, 101`): `/trieuphu` and `/altp` (**missing** `millionaire`).

---

### B. `whoami` (Đoán Danh Nhân / Who Am I)
- **Question Banks & Gameplay:**
  - **Python** (`whoami_system.py:13-84, 91-156`): 7 historical/scientific figures (`_FAMOUS_PEOPLE`), each with `name`, `aliases`, and **4 progressive clues** revealed every 10s (`100 -> 75 -> 50 -> 25` points).
  - **Rust** (`zenne-games/src/whoami.rs:5-31`): Completely different bank (`WHOAMI_BANK`) of 7 **single-clue riddles** (animals/objects like `"con voi"`, `"cái ghế"`), no aliases list, no progressive clues. Answer matching (`whoami.rs:53`) uses `g == ans || (ans.ends_with(&g) && g.len() >= 3)` instead of Python's `content == name_lower or any(a in content for a in aliases)` (`whoami_system.py:195`).
- **Player Ownership & Interaction:**
  - **Python** (`whoami_system.py:177-224`): Channel-wide `on_message` race; anyone in channel can guess.
  - **Rust Discord** (`extra_cmds.rs:487-507`): Requires explicit `!whoami <đáp án>` command (no `on_message` listener), no ownership restriction.
- **Scoring:**
  - **Python** (`whoami_system.py:202, 206`): `max(25, 100 - clue_idx * 25)`, submitted with `cumulative=True`.
  - **Rust Discord** (`extra_cmds.rs:487-507`): **No score submission**.
- **Timeouts:**
  - **Python** (`whoami_system.py:115-156`): 40s total (4 × 10s intervals), then reveals answer and pops session.
  - **Rust Core / Discord**: **No timeout**.
- **Aliases:**
  - **Python** (`whoami_system.py:158, 170`): `/whoami`, `!whoami`, aliases `["danhnhan", "ailasi"]`.
  - **Rust Discord** (`extra_cmds.rs:486-487`): `whoami` only (**missing** `danhnhan`, `ailasi`).

---

### C. `history_guess` (Đoán Năm Lịch Sử)
- **Question Banks & Gameplay:**
  - **Python** (`history_guess_system.py:13-74, 81-166`): 12 historical events (`_HISTORY_EVENTS`) with `event`, `year: int`, and `detail`. Multi-player 35s collection window (`guesses[user_id] = year_val` for `0..=2100`, reacts `📝`), then picks the closest guess (`abs(guess - ans)`).
  - **Rust** (`zenne-games/src/history_guess.rs:5-23, 41-51`): Different 8-item `HISTORY_BANK` asking for **names, dates, or years** as strings (`"ngô quyền"`, `"02/09/1945"`). `check_answer` (`44`) uses `guess.trim().to_lowercase().contains(&ans)` and ends immediately on first match — **no proximity year guessing**.
- **Player Ownership:**
  - **Python**: Multi-player channel-wide collection (`on_message` at `187-207`).
  - **Rust Discord** (`extra_cmds.rs:466-483`): Explicit `!history <đáp án>`, first-match wins, no ownership check.
- **Scoring:**
  - **Python** (`history_guess_system.py:132-146`): Tiered by year delta (`diff==0 -> 100`, `<=2 -> 75`, `<=5 -> 40`, `else -> 15`), submitted under key `"history"` with `cumulative=True`.
  - **Rust Discord** (`extra_cmds.rs:466-483`): **No score submission**.
- **Timeouts:**
  - **Python** (`history_guess_system.py:103-125`): 35s round timer.
  - **Rust Core / Discord**: **No timeout**.
- **Aliases:**
  - **Python** (`history_guess_system.py:168, 180`): `/history`, `!history`, aliases `["lichsu", "namlichsu"]`.
  - **Rust Discord** (`extra_cmds.rs:465-466`): `history` only (**missing** `lichsu`, `namlichsu`).

---

### D. `emoji` (Đuổi Hình Bắt Chữ Emoji)
- **Question Banks & Hints:**
  - **Python** (`emoji_game_system.py:20-57, 62-97`): 31 puzzles (`_EMOJI_PUZZLES`) with `emoji`, `answer`, `topic`, and `hint`. Interactive `EmojiHintButton` provides 3 hint tiers (topic/length → detail hint → first letter), and `EmojiStopButton` ends the round.
  - **Rust** (`zenne-games/src/emoji.rs:5-18`): 12-item `EMOJI_BANK` (`(&str, &str)` tuples) with **no topic, no hint, no stop button**, and mostly different puzzles.
- **Player Ownership:**
  - **Python** (`emoji_game_system.py:66-112, 195-210`): Channel-wide `on_message` race; `EmojiHintButton` and `EmojiStopButton` are usable by anyone in the channel (no `author_id` check despite `EmojiView` storing `self.author_id` at `117`).
  - **Rust Discord** (`extra_cmds.rs:418-438`): Explicit `!emoji <đáp án>`, no ownership check.
- **Scoring:**
  - **Python** (`emoji_game_system.py:211-218`): Speed + hint-penalty formula `max(25, int(100 - elapsed*2 - hints_used*15))`, submitted under key `"duoihinh"` with `cumulative=True`.
  - **Rust Discord** (`extra_cmds.rs:418-438`): **No score submission**.
- **Timeouts:**
  - **Python** (`emoji_game_system.py:116, 161`): 35s round timer (`asyncio.sleep(35)`), 45s View timeout (`EmojiView(timeout=45)`).
  - **Rust Core / Discord**: **No timeout**.
- **Aliases:**
  - **Python** (`emoji_game_system.py:175, 187`): `/duoihinh`, `!duoihinh`, aliases `["dhbc", "emojiword"]`.
  - **Rust Discord** (`extra_cmds.rs:417-418`): `emoji` only (**missing** `duoihinh`, `dhbc`, `emojiword`).

---

## 3. Recommended Tests
1. **`zenne-games` Unit Tests** (currently **0 tests** in `trieuphu.rs`, `whoami.rs`, `history_guess.rs`, `emoji.rs`):
   - `trieuphu::tests::test_tier_progression_and_no_duplicates`: Verify tiered bank lookup (levels 1–15) and non-repeating questions per session.
   - `trieuphu::tests::test_player_ownership_enforced`: Verify `answer(channel_id, player_id, ans_idx)` rejects non-owner `player_id`.
   - `trieuphu::tests::test_lifelines_and_stop_safe_prize`: Verify `50:50` eliminates 2 wrong options, `swap` changes question at same tier, and `stop` returns `MONEY_LADDER[level - 2]`.
   - `whoami::tests::test_progressive_clues_and_alias_matching`: Verify 4-clue progression, timeout expiration, and alias normalization.
   - `history_guess::tests::test_year_proximity_scoring`: Verify 35s multi-user guess collection and delta-based scoring (`0`, `<=2`, `<=5`, `>5`).
   - `emoji::tests::test_hint_penalty_and_speed_scoring`: Verify 3-tier hint state and `max(25, 100 - elapsed*2 - hints*15)` calculation.
2. **`zenne-telegram` Callback & DB Tests** (`crates/zenne-telegram/src/minigames.rs`):
   - Verify `tp_5050`, `tp_aud`, and `tp_stop` reject callbacks where `q.from.id != tp.player_id`.
   - Verify `tp_stop` calls `db.game_submit_score("trieuphu", uid, earned)` when `earned > 0`.
3. **`zenne-discord` Command & Score Integration Tests** (`crates/zenne-discord/src/lib.rs`, `extra_cmds.rs`):
   - Extend `test_existing_command_registry_preserved` (`lib.rs:686`) to assert aliases (`altp`, `millionaire`, `danhnhan`, `ailasi`, `lichsu`, `namlichsu`, `duoihinh`, `dhbc`, `emojiword`) and verify DB score submission for all 4 minigames (`cumulative=true` for `whoami`, `history`, `duoihinh`; `cumulative=false` for `trieuphu`).
