# Forensic Audit Report — ID 15

**Scope:** Parity audit of `taixiu`, `baucua`, `higher_lower`, and `mastermind` between Python (`gdrive-bot-source/gdrive-bot/`) and Rust (`zenne-noitu-rs/crates/{zenne-games, zenne-core, zenne-discord, zenne-telegram, zenne-twitch, zenne-revolt}/`).

---

## 1. Executive Summary (Critical Parity Gaps)

1. **DB Scoring Mode Bug (`zenne-core/src/db.rs:653-656`)**: `Database::game_submit_score` hardcodes `cumulative = (game == "afk")`. Consequently, `zenne-telegram` (`taixiu` and `baucua`) uses `MAX(best_score, excluded.best_score)` instead of accumulating points (`cumulative=True` in Python), capping `taixiu` lifetime score at `50` and `baucua` at single-round max.
2. **TaiXiu Triple ("Bão") Classification Bug (`zenne-telegram/src/minigames.rs:331`)**: `taixiu::roll_taixiu()` sets `is_tai = false` when `is_triple = true`, and `minigames.rs:331` maps `if r.is_tai { "tai" } else { "xiu" }`—ignoring `r.is_triple`. Every triple (including `6-6-6 = 18`) is paid out as **XỈU**.
3. **Stateless vs. Interactive Betting Divergence**: Rust `zenne-discord` (`lib.rs:267-298`), `zenne-twitch` (`lib.rs:281-291`), and `zenne-revolt` (`lib.rs:300-310`) implement `taixiu` and `baucua` as stateless instant rolls with **no betting pool, no payout math, and no DB scoring**, whereas Python Discord/Telegram run timed multi-player betting tables and Python Twitch/Revolt clamp bets (`5..=100`) and persist `+/-` scores.
4. **Higher/Lower Complete Mechanic Mismatch**: Python (`higher_lower_system.py:14-162`) compares Wikipedia monthly views from `_TOPICS_DATABASE` per user; Rust (`zenne-games/src/higher_lower.rs:17-52`) guesses `1..=100` integers per channel with no DB scoring.
5. **Mastermind Domain & Turn Mismatch**: Python (`mastermind_system.py:44-51`) generates **4 unique digits `0..=9`** with **8 turns** and **120s timeout**; Rust (`zenne-games/src/mastermind.rs:17-82`) generates **4 digits `1..=6` with replacement** and **10 turns**, with no timeout and no DB scoring.

---

## 2. Per-Game Parity Matrix

### A. `taixiu` (Tài Xỉu)
| Dimension | Python | Rust | Parity Status |
| :--- | :--- | :--- | :--- |
| **Core / Triple Rule** | Discord (`taixiu_system.py:94`) & Telegram (`telegram_bot.py:1196`): `total >= 11` -> Tài, else Xỉu (no triple rule). Twitch (`twitch_bot.py:815-823`) & Revolt (`revolt_bot.py:808-816`): `d1==d2==d3` -> `"bão"` (house wins). | `zenne-games/src/taixiu.rs:10-24` (`roll_taixiu`): `is_triple = d1==d2 && d2==d3`, `is_tai = total >= 11 && total <= 17 && !is_triple`. `zenne-telegram/src/minigames.rs:331` ignores `is_triple` (pays triples as Xỉu). | **BROKEN** (Telegram triple bug; Discord Python vs Rust rule mismatch) |
| **Betting & Payout** | Discord (`taixiu_system.py:74,100-109`): 15s window, `+80` win (DB), `-30` display only (not deducted). Telegram (`telegram_bot.py:1189,1204`): 20s window, `+50` win. Twitch/Revolt: `bet` clamped `5..=100` (default `10`), `+/-bet`. | Discord (`zenne-discord/src/lib.rs:267-282`): instant roll, no bet. Telegram (`minigames.rs:325,344`): 20s window, `+50` win. Twitch (`zenne-twitch/src/lib.rs:281-287`) & Revolt (`zenne-revolt/src/lib.rs:300-306`): instant roll, `tai\|xiu` guess text only, no bet/payout. | **DIVERGENT** |
| **DB Scoring** | `db_game_submit_score("taixiu", uid, pts, cumulative=True)` (Discord/TG); `"twitch"`/`"revolt"` (Twitch/Revolt). | Only `zenne-telegram/src/minigames.rs:344` calls `game_submit_score("taixiu", *uid, 50)` -> **non-cumulative (`MAX`)** via `db.rs:654`. Discord/Twitch/Revolt: none. | **BROKEN** |
| **Aliases** | Discord: `taixiu`, `tx`, `xucxac` (`taixiu_system.py:134`). TG/Twitch/Revolt: `taixiu`, `tx` (plus `t`/`x` choice). | Discord (`lib.rs:266`): `taixiu`, `tx` (missing `xucxac`). TG (`zenne-telegram/src/lib.rs:96`), Twitch (`lib.rs:281`), Revolt (`lib.rs:300`): `taixiu`, `tx` (Twitch/Revolt missing `t`/`x` choice). | **PARTIAL** |

### B. `baucua` (Bầu Cua)
| Dimension | Python | Rust | Parity Status |
| :--- | :--- | :--- | :--- |
| **Faces & Display** | `nai`, `bau` (`🎃`), `ga`, `ca`, `cua`, `tom` (`baucua_system.py:13-20`, `telegram_bot.py:1215-1222`). | `BauCuaFace` (`zenne-games/src/baucua.rs:4-46`): `Bau` uses `"🍐"` (`baucua.rs:39`), while `zenne-telegram/src/minigames.rs:379` uses `"🎃"`. | **MINOR DRIFT** |
| **Betting & Payout** | Discord (`baucua_system.py:52,126-142`): 20s window (+2s roll), `20`/click; `net_gain = user_win - user_loss`, DB updated only if `net_gain > 0`. TG (`telegram_bot.py:1253,1262`): 25s window, `20`/click; gross `sum(bet_amt * hits)` (unmatched bets NOT subtracted). Twitch/Revolt: `bet` `5..=100`, `bet * matches` or `-bet`. | Discord (`lib.rs:284-298`), Twitch (`lib.rs:288-291`), Revolt (`lib.rs:307-310`): instant roll only, **ignores animal/bet arguments**, no payout. Telegram (`minigames.rs:388-495`): 25s window, `20`/click, gross `amt * hits`. | **DIVERGENT** |
| **DB Scoring** | `cumulative=True` on Discord (`baucua_system.py:139`), TG (`telegram_bot.py:1264`), Twitch (`twitch_bot.py:870,876`), Revolt (`revolt_bot.py:867,870`). | Only `zenne-telegram/src/minigames.rs:472` submits `"baucua"` -> **non-cumulative (`MAX`)** via `db.rs:654`. Discord/Twitch/Revolt: none. | **BROKEN** |
| **Aliases** | Discord: `baucua`, `tomca`, `bc` (`baucua_system.py:181`). TG/Twitch/Revolt: `baucua`, `bc`. | Discord (`lib.rs:284`): `baucua`, `bc` (missing `tomca`). TG/Twitch/Revolt: `baucua`, `bc`. | **PARTIAL** |

### C. `higher_lower` (Higher or Lower)
| Dimension | Python (`higher_lower_system.py`) | Rust (`zenne-games/src/higher_lower.rs`, `zenne-discord/src/extra_cmds.rs`) | Parity Status |
| :--- | :--- | :--- | :--- |
| **Mechanic** | Wikipedia topic views from `_TOPICS_DATABASE` (20 entries, lines 14–35); per-user `HigherLowerView` (40s timeout, lines 39–49). | Random `1..=100` integer (`higher_lower.rs:18,33`); per-channel `DashMap<u64, HigherLowerSession>` (lines 12–14), no timeout, any user can hijack/overwrite. | **MISMATCH** |
| **Tie & Input** | Tie (`vb == va`) wins (`line 71`). Button-driven. | Tie (`next == prev`) wins (`line 34`). **Input bug (`extra_cmds.rs:566`)**: unrecognized `choice` silently defaults to `false` (Lower) instead of erroring. | **BUG** |
| **DB Scoring** | `pts = new_streak * 25` submitted every correct step (`cumulative=True`, lines 78–81). | None. | **MISSING** |
| **Aliases** | `higherlower`, `hl`, `sokeo`, `caothap` (`line 159`). | `higherlower` only (`extra_cmds.rs:562`), no aliases. | **MISSING** |

### D. `mastermind` (Mastermind)
| Dimension | Python (`mastermind_system.py`) | Rust (`zenne-games/src/mastermind.rs`, `zenne-discord/src/extra_cmds.rs`) | Parity Status |
| :--- | :--- | :--- | :--- |
| **Secret & Turns** | 4 **unique** digits `0..=9` (`random.shuffle`, lines 44–46); **8 turns** (`line 51`); **120s** timeout (`line 72`). | 4 digits `1..=6` **with replacement** (`gen_range(1..=6)`, lines 19–24); **10 turns** (`line 82`); no timeout. | **MISMATCH** |
| **Exact / Partial Logic** | `_evaluate_mastermind` (lines 16–35): 2-pass (`zip` exact -> `s_unmatched.remove(g_char)`). Sound for unique & duplicate digits. | `MastermindManager::guess` (`mastermind.rs:51-74`): 2-pass with `used_s: [bool; 4]` & `used_g: [bool; 4]`. Mathematically sound for duplicate digits. | **PARITY OK** |
| **Input Parsing** | `on_message` listener (`lines 109-120`): requires `len(content) == 4 and content.isdigit()`. | `!mastermind <guess>` (`extra_cmds.rs:511`). **Bug (`mastermind.rs:39-45`)**: `filter_map(\|c\| c.to_digit(10))` silently strips non-digits (e.g., `"1a2b3c4"` -> `"1234"`). | **BUG** |
| **DB Scoring** | On win: `max(30, int(150 - curr_turn*15 - elapsed*0.5))`, `cumulative=True` (`lines 138-142`). | None. | **MISSING** |
| **Aliases** | `mastermind`, `matma`, `bekhoa`, `mm` (`line 102`). | `mastermind` only (`extra_cmds.rs:510`), no aliases. | **MISSING** |

---

## 3. RNG Fairness & Cooldowns / Rate Limits

- **RNG Fairness**: Python uses `random` (Mersenne Twister: `randint`, `choice`, `shuffle`, `sample`), which is non-cryptographic. Rust uses `rand::thread_rng()` (ChaCha12 CSPRNG in `rand 0.8`) with uniform `gen_range(1..=6)`, `gen_range(0..6)`, `gen_range(1..=100)`—unbiased and cryptographically strong.
- **Cooldowns / Rate Limits**:
  - **Table locks**: Both Python and Rust Telegram guard against concurrent tables per chat (`_ACTIVE_TAIXIU` / `active_taixiu`, `_ACTIVE_BAUCUA` / `active_baucua`). Note `zenne-telegram` `bc_bet:` (`minigames.rs:832-857`) lacks an `accepting` flag check inside the struct (unlike `TaiXiuTable.accepting`).
  - **Missing session guards in `zenne-games`**: `HigherLowerManager::start` (`higher_lower.rs:17-27`) and `MastermindManager::start` (`mastermind.rs:17-32`) unconditionally overwrite active channel sessions.
  - **Command rate limits**: Neither Python Twitch/Revolt nor any Rust bot crate enforces per-user command cooldowns on `!taixiu`/`!baucua`/`!higherlower`/`!mastermind` (Python Twitch/Revolt allow unbounded DB score farming via rapid commands; Python web API `bot.py:161, 4484` has `_RATE_LIMITED_GAMES` + `_sanity_check_score`).

---

## 4. Recommended Tests

Currently `zenne-games` has **zero** unit tests for `taixiu`, `baucua`, `higher_lower`, or `mastermind` (only `lib.rs:73` exists). Add:

1. **`zenne-games/src/taixiu.rs` + `zenne-telegram/src/minigames.rs`**:
   - Test `roll_taixiu()` invariants: `total == d1+d2+d3`, `is_triple => !is_tai`, `total == 18 => is_triple && !is_tai`, `total == 3 => is_triple && !is_tai`.
   - Test Telegram settlement logic on `TaiXiuResult { dice: [6,6,6], total: 18, is_tai: false, is_triple: true }` to verify triples do NOT pay out `"xiu"` bettors.
2. **`zenne-games/src/mastermind.rs`**:
   - **Duplicate-digit evaluation tests** (direct unit tests on evaluation helper):
     - `secret=[1,1,2,2]`, `guess="1111"` -> `exact=2, partial=0` (no double-count).
     - `secret=[1,2,3,4]`, `guess="1111"` -> `exact=1, partial=0`.
     - `secret=[1,1,2,3]`, `guess="2114"` -> `exact=1, partial=2`.
   - **Input validation test**: `guess(ch, "1a2b3c4")` and `guess(ch, "0123")` must return `Err(...)`.
   - **Turn limit test**: 10th wrong guess returns `over=true, won=false` and removes session from `DashMap`.
3. **`zenne-games/src/higher_lower.rs`**:
   - Test `guess` tie (`next == prev`) returns `won=true` and increments `streak`.
   - Test loss removes session from `DashMap` and returns `final_streak`.
4. **`zenne-core/src/db.rs`**:
   - Test `game_submit_score_mode("taixiu", uid, 50, true)` twice yields `best_score == 100, plays == 2` (and verify `game_submit_score` behavior vs `cumulative=True` expectation).
5. **Cross-platform command parity (`zenne-discord/src/lib.rs` & `extra_cmds.rs`)**:
   - Extend `test_existing_command_registry_preserved` to assert expected aliases (`tx`, `xucxac`, `bc`, `tomca`, `hl`, `sokeo`, `caothap`, `matma`, `bekhoa`, `mm`).
