# Forensic Audit Report — ID 50

**Scope:** Leaderboards, rewards, achievements/milestones, and economy/scoring consistency across Python (`C:\Users\Ai Hoi\Downloads\archive-2026-09-23T144146Z\bot.py`), Rust (`C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\*`), and Web (`C:\Users\Ai Hoi\Downloads\zenne-noitu-deploy-vercel\pages\*`).

---

## 1. Negative vs. Positive `user_id` Conventions & Cross-Platform Collisions
- **Intended Convention:** `uid < 0` -> `web_users WHERE id = -uid`; `uid > 0` -> bot platform user ID.
- **CRITICAL BUG — Web Multiplayer (`ws_game.rs`) Logs Positive `user_id`:**
  - `pages/api/pub/game-token.js:18` sends positive `session.user.id`, `zenne-api/src/lib.rs:986-1020` signs it directly, and `ws_game.rs:223,261,457` calls `lb_add_word(0, p_uid)` with positive `p_uid` (unlike Python `bot.py:3752` which negated `-player["user_id"]`).
  - `resolve_display_user` (`db.rs:906`) fails to match `web_users`, and `enrich_discord_users` (`lib.rs:347`) queries Discord `/users/{uid}` with web IDs.
- **Cross-Platform Positive ID Collision:** `leaderboard` and `game_scores` have no `platform` column, so Discord and Telegram both write positive IDs. `zenne-twitch` and `zenne-revolt` only read `lb_get_top(0, 5)` and never write scores.

---

## 2. Streak Resets (`current_streak` & `max_streak`)
- **Python (`bot.py`):** `db_lb_reset_streak` (`L723-734`) executes `INSERT ... ON CONFLICT DO NOTHING` (no-op for existing rows).
- **Rust (`db.rs` & Consumers):**
  - `zenne-discord/src/noitu.rs` and `zenne-api/src/ws_game.rs` **never** call `lb_reset_streak`.
  - `zenne-telegram/src/noitu.rs` omits `lb_reset_streak` on already-used words (`L1524-1540`) and turn timeouts (`L650-715`).

---

## 3. Win / Play Counters (`games_played`, `games_won`, `plays`)
- `db.rs` `lb_add_game` (`L611-627`) is **dead code** in Rust — never called in Discord, Telegram, Twitch, Revolt, or WS. `games_played` and `games_won` remain `0` for all Rust-played games.
- `game_submit_score` (`db.rs:653-656`) hardcodes `cumulative = (game == "afk")`, capping `best_score` for Telegram minigames (`taixiu`, `quiz`, `cadao`, `chinhta`, `flagquiz`, `vuatiengviet`) at single-round constants.

---

## 4. Tiebreakers
- `lb_get_top` (`db.rs:592`) and `lb_streak_top` (`db.rs:634`) omit secondary tiebreakers (`max_streak DESC` / `total_words DESC`, `user_id ASC`).
- `game_top` (`db.rs:690`), `pub_game_leaderboard` (`db.rs:883`), and `tournament_players` (`db.rs:1165`) omit tiebreakers (`plays ASC, user_id ASC` / `joined_at ASC, user_id ASC`).

---

## 5. Milestones / Achievements (`milestones`)
- `u100`/`u1k`: Rust falls back to `COUNT(DISTINCT user_id)` when `live.user_count == 0`, mixing guild member counts with distinct players.
- `g1k`: Stalled in Rust because `lb_add_game` is never called.

---

## 6. Tournament Scoring (`tournaments`, `tournament_players`)
- `db.rs` `tournament_add_word` (`L1138-1158`) and `tournament_players` (`L1160-1175`) are **never called** in `zenne-discord`.
- `tournaments.status` defaults to `'open'` in schema while `extra_cmds.rs` / `db.rs` query `status = 'active'`.
- `tournament_add_word` auto-inserts unregistered users into `tournament_players` without checking `tournament_join`.
