# Forensic Audit Report — ID `04`

## Scope
- **Python Reference**: `C:\Users\Ai Hoi\Downloads\archive-2026-09-23T144146Z\bot.py` (`db_lb_*`, `db_tournament_*`, `c_diem`, `c_top`, `TournamentJoinView`, `c_tochucgiaidau`, `c_ketthucgiaidau`, `c_xemgiaidau`)
- **Rust Port**:
  - `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-discord\src\lib.rs` (`top`, `stop`, `all_commands`)
  - `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-discord\src\extra_cmds.rs` (`diem`, `tochucgiaidau`, `xemgiaidau`, `ketthucgiaidau`, `thamgiagiaidau`)
  - `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-discord\src\noitu.rs` (`handle_word_message` callers)
  - `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-core\src\db.rs` (`lb_*`, `tournament_*`)

## Status
**NON-PARITY / CRITICAL GAPS** — Core tournament scoring, end-of-tournament ranking, admin authorization, streak resets, game win/play tracking, and `!top streak` are broken or unimplemented in `zenne-discord`.

---

## Findings

### 1. Tournaments (`db_tournament_*`, `TournamentJoinView`, `tochucgiaidau`, `xemgiaidau`, `ketthucgiaidau`)
1. **Tournament word scoring is dead code (`CRITICAL`)**:
   - `bot.py:1935-1937` calls `db_tournament_add_word(gid, uid)` on every valid word (`bot.py:1068-1088`).
   - `db.rs:1138-1158` implements `tournament_add_word(guild_id, user_id)`, but it is **never called** anywhere in `zenne-discord` (`noitu.rs:139` only calls `app.db.lb_add_word`). All `words_scored` remain `0`.
2. **Missing Admin authorization on tournament management (`CRITICAL`)**:
   - `bot.py:2561` (`c_tochucgiaidau`) and `bot.py:2625` (`c_ketthucgiaidau`) enforce `is_admin(ctx)`.
   - `extra_cmds.rs:245-268` (`tochucgiaidau`) and `extra_cmds.rs:302-315` (`ketthucgiaidau`) have **no permission check** — any user can create or end a guild's tournament.
3. **Missing active-tournament guard & raw SQL in `tochucgiaidau` (`HIGH`)**:
   - `bot.py:2568-2574` calls `db_tournament_get_active(gid)` (`bot.py:1036-1044`) to block duplicate active tournaments, and `bot.py:1023-1034` (`db_tournament_create`) + `bot.py:1046-1053` (`db_tournament_set_msg`) return `tid` and persist `message_id`.
   - `db.rs` lacks `tournament_create`, `tournament_get_active`, `tournament_set_msg`, and `tournament_end`. `extra_cmds.rs:253-260` executes raw `INSERT INTO tournaments` without checking for an existing active tournament (allowing multiple concurrent `'active'` rows per guild) and ignores DB errors (`let _ = ...`).
4. **Argument parsing & missing aliases on tournament commands (`MEDIUM`)**:
   - `bot.py:2554,2583-2585` (`c_tochucgiaidau`, aliases `tournament`, `giaidau`, `td`) parses `<Ten giai> | <Phan thuong>`. `extra_cmds.rs:244-249` has no aliases and takes `(name: String, prize: Option<String>)`, which breaks unquoted multi-word names in prefix mode (`!tochucgiaidau Giai He`).
   - `bot.py:2622` (`c_ketthucgiaidau`, aliases `endtournament`, `ketgiai`, `kg`) and `bot.py:2663` (`c_xemgiaidau`, aliases `tournament_info`, `gd`) have no aliases in `extra_cmds.rs:271,301`.
5. **No `TournamentJoinView` button UI (`MEDIUM`)**:
   - `bot.py:2528-2551,2617` attaches a persistent `TournamentJoinView` button (`timeout=None`).
   - `extra_cmds.rs:261-266` sends plain text with no button component; joining is instead a separate command `thamgiagiaidau` (`extra_cmds.rs:690-718`, alias `joingd`) which is not mentioned in the `tochucgiaidau` announcement.
6. **`xemgiaidau` omits players and leaderboard (`HIGH`)**:
   - `bot.py:2672-2685` calls `db_tournament_players(t["id"])` (`bot.py:1090-1102`) to show participant count and top-10 rankings.
   - `db.rs:1160-1175` implements `tournament_players`, but it is **never called**; `extra_cmds.rs:272-298` only prints `id`, `name`, `prize`, and status.
7. **`ketthucgiaidau` does not check active state or announce winners (`HIGH`)**:
   - `bot.py:2630-2660` checks `db_tournament_get_active`, calls `db_tournament_end` (`bot.py:1104-1120`), and posts the ranked scoreboard + winner + prize.
   - `extra_cmds.rs:302-315` runs `UPDATE tournaments SET status = 'ended' ...` without checking `rows_affected` (prints success even when no tournament is active) and does not query or announce rankings/winner.

### 2. Leaderboards (`db_lb_*`, `top`, `diem`)
1. **Streak never resets in `zenne-discord` (`CRITICAL`)**:
   - `bot.py:1859,1885,1914` calls `db_lb_reset_streak(gid, uid)` (`bot.py:723-734`) on wrong chain, invalid language, or Wikipedia miss.
   - `db.rs:576-585` implements `lb_reset_streak`, and `db.rs:559-574` (`lb_add_word`) increments `current_streak` in SQLite. However, `noitu.rs:79-119` **never calls `lb_reset_streak`** on invalid/wrong-chain words, so `current_streak` and `max_streak` grow unboundedly on every valid word regardless of mistakes.
2. **`lb_add_game` is never called in `zenne-discord` (`HIGH`)**:
   - `db.rs:611-627` implements `lb_add_game`, but neither `lib.rs:98-104` (`stop`) nor `noitu.rs:161-175` (bot game end) calls it. `games_played` and `games_won` stay `0`.
3. **`top` command lacks `streak` mode, tiebreakers, and stats (`MEDIUM`)**:
   - `bot.py:2470-2522` (`c_top`, aliases `lb`, `bxh`, `xephang`) supports `mode="words"` (`db_lb_get_top`) and `mode="streak"` (`db_lb_streak_top`), showing `total_words`, `max_streak`, `games_played`, `games_won`.
   - `lib.rs:169-194` (`top`, alias `lb` only) has no `mode` parameter, never calls `db.rs:629-651` (`lb_streak_top`), and omits `games_played`/`games_won`.
   - `db.rs:592` (`lb_get_top`) and `db.rs:634` (`lb_streak_top`) lack secondary `ORDER BY` tiebreakers (`bot.py:767,782` use `ORDER BY total_words DESC, max_streak DESC` and `max_streak DESC, total_words DESC`).
4. **`diem` command differences (`LOW`)**:
   - `bot.py:2261-2274` (`c_diem`, aliases `score`, `scores`) checks active game existence and sends an embed with language + `used_words` count in the footer.
   - `extra_cmds.rs:66-85` (`diem`, alias `score` only) does not check `r.active` (can show stale scores from a stopped room) and sends plain text without language/word-count footer.

---

## Recommended Tests
1. **Tournament lifecycle & scoring integration test (`zenne-core` + `zenne-discord`)**:
   - Verify non-admin cannot run `tochucgiaidau` / `ketthucgiaidau`.
   - Verify duplicate `tochucgiaidau` in the same guild is rejected while one is active.
   - Play valid words in `noitu::handle_word_message` during an active tournament and assert `tournament_players.words_scored` increments and `xemgiaidau` / `ketthucgiaidau` display the ranked winner.
   - Verify `ketthucgiaidau` reports an error when no active tournament exists.
2. **Leaderboard streak & game stats test**:
   - Submit valid words then a wrong-chain or invalid word in `noitu::handle_word_message`; assert `leaderboard.current_streak` resets to `0` while `max_streak` is preserved.
   - Finish/stop a game and assert `games_played` and `games_won` increment via `lb_add_game`.
   - Verify `top` supports both `words` and `streak` modes with deterministic tie-breaking.
3. **`diem` room-state test**:
   - Stop a room (`!stop`) and run `!diem`; assert it reports no active game rather than stale scores.
