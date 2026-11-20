# Forensic Audit Report — ID 49

Audit of minigame scoring, replay attacks, and anti-cheat invariants across Rust (`zenne-noitu-rs`) and Python (`gdrive-bot-telegram`).

---

## 1. Web-Side Findings (`zenne-api`, `zenne-core` vs `bot.py`)

### 1.1 Unauthenticated `/pub/game/ttt` & `/pub/game/ms` — Missing Auth & Session Ownership (Critical)
- **Rust**: `ah_pub_game_ttt` (`crates/zenne-api/src/lib.rs:726-738`, `pub_games.rs:125-202`) and `ah_pub_game_ms` (`lib.rs:740-752`, `pub_games.rs:251-335`) **do not call `check_admin_auth`** and **do not verify any `Authorization: Bearer` web token**.
- **Python**: `_ah_pub_game_ttt` (`bot.py:1579`) and `_ah_pub_game_ms` (`bot.py:1669`) enforce `_chk(req)` (`X-API-Key`).
- **Impact**: Any unauthenticated caller can `POST /pub/game/ttt` or `POST /pub/game/ms` with an arbitrary `user_id` in JSON, hijacking or resetting another user's streak/board. `handle_ms` also has no minimum elapsed time check before awarding up to `1000` points (`pub_games.rs:321`).

### 1.2 `sanity_check_score` Anti-Cheat Regressions (High)
- **Rust**: `pub_games::sanity_check_score` (`pub_games.rs:337-348`) only checks `score >= 0` and upper bounds for 4 games (`2048 <= 500_000`, `snake <= 10_000`, `minesweeper <= 1_000`, `tictactoe <= 1_000`), falling back to `_ => score <= 1_000_000`.
- **Python**: `_sanity_check_score` (`bot.py:171-209`) enforces `elapsed >= 0.2s`, `2048` (`% 4 == 0`, `<= 200_000`, `<= elapsed * 500 + 500`), `snake` (`% 10 == 0`, `<= 5_000`, `<= elapsed * 100 + 100`), `vuatiengviet`/`duoihinh <= 100`, `wordle <= 120`, `typing <= 350`, `trieuphu <= 150_000_000`, `quiz <= elapsed * 80 + 500`.

### 1.3 Missing Game Allowlist & Case-Normalization Bypass on `/pub/game/score` & `/pub/game/session/start` (High)
- Rust `ah_pub_game_score` (`lib.rs:455-520`) and `ah_pub_game_session_start` (`lib.rs:580-596`) accept any `game` string and do not normalize `body.game` (`.trim().to_lowercase()`). Submitting `"AFK"` bypasses `body.game == "afk"` elapsed (`lib.rs:495`) and delta (`lib.rs:509`) checks.

### 1.4 Monotonic Nonces, Replay Vectors & Memory Exhaustion (Medium–High)
- Rust `used_nonces` (`lib.rs:27, 504-513`): in-memory `DashMap<String, i64>`, never evicted/expired (unbounded memory growth) and lost on restart (allowing `afk` `session_token` replay within 3600s TTL).
- Python `_USED_GAME_SESSION_SCORES` (`bot.py:160, 4504-4505`): `.clear()` at `> 20000` allows replay after flooding.

---

## 2. Bot-Side Findings (`zenne-games`, `zenne-discord`, `zenne-telegram`, `zenne-twitch`, `zenne-revolt` vs Python)
1. **Cumulative vs High-Score Mode Broken in Rust (`db.rs:653-683` & `zenne-telegram`)**: `game_submit_score` hardcodes `cumulative = (game == "afk")`; `game_submit_score_mode` is never called.
2. **Discord, Twitch & Revolt Minigames Do Not Persist Scores**: Zero `game_submit_score` calls across `zenne-discord`, `zenne-twitch`, and `zenne-revolt`.
3. **Bầu Cua Unbounded Bet / Infinite Score Farm**: `bc_bet:<animal>` (`minigames.rs:464-473, 832-850`) adds `+20` per click with no per-user bet cap and no subtraction of lost bets.
4. **Session Ownership & Missing Score Submission in Triệu Phú**: Telegram `tp_5050`, `tp_aud`, `tp_stop` (`minigames.rs:1039-1142`) and `zenne-games/src/trieuphu.rs:145-177` omit `player_id` ownership checks; `tp_stop` omits DB score submission.
