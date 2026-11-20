# Forensic Audit Report — ID 16

## Executive Summary
Cross-language parity audit of 7 minigames (`battleship`, `connect4`, `memory`, `typing`, `afk`, `sudoku`, `uno`) comparing Python (`C:\Users\Ai Hoi\Downloads\gdrive-bot-source\gdrive-bot\`) and Rust (`C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\`).

**Platform Coverage Gap:**
- **Python**: All 7 games are implemented for Discord (`discord.py` slash + prefix commands, Views/Buttons/Modals, `on_message` listeners, background tasks) with DB scoring.
- **Rust (`zenne-games` + `zenne-discord`)**: Only Discord exposes commands for these 7 games (`zenne-discord/src/lib.rs`, `extra_cmds.rs`). **`zenne-telegram`, `zenne-twitch`, and `zenne-revolt` have 0 implementations** of any of these 7 games.
- **Critical Rust Regressions**:
  1. `connect4` in `zenne-discord` has an **off-by-one bug** (`c.saturating_sub(1)` passed to 1-based `drop_piece`), rejecting column `1` and shifting `2..=7` to `1..=6`.
  2. `sudoku` and `uno` in `zenne-games` are **non-playable stubs** (only `start()` exists; no guess/play/draw/win logic).
  3. **Zero DB scoring** in `zenne-discord` for all 7 games (`game_submit_score` is never called).
  4. **Zero interactive UI/buttons** and **zero timeouts** across all 7 Rust Discord implementations.

---

## Per-Minigame Parity Matrix

### 1. `battleship`
- **Python**: `battleship_system.py` (`register_battleship_game`, L88–281)
- **Rust**: `zenne-games/src/battleship.rs` (`BattleshipManager`, L16–117) + `zenne-discord/src/lib.rs` (`battleship`, L415–441)
- **Board State**:
  - *Python*: 1v1 PvP (`_ACTIVE_BATTLESHIP` keyed by `min-max` user IDs, L97), 5×5 grids per player (`grid_p1`/`grid_p2`, `attack_p1`/`attack_p2`), 3 multi-cell ships (`SHIPS`: 3, 2, 2 = 7 cells, L15) placed via `_place_ships_randomly` (L41–65), secret grids DM'd to both players (L121–134).
  - *Rust*: Single-player/shared-channel (`DashMap<u64, BattleshipGame>` keyed by `channel_id`, L17), 4 single-cell ships (`ships.len() < 4`, L24–29), 12-shot budget (`shots_left: 12`, L35). No DM secret board.
- **Turn Enforcement**:
  - *Python*: Strictly enforces `game["turn"] == message.author.id` in `_on_battleship_message` (L207).
  - *Rust*: None; any user in the channel can call `!battleship A1` (`shoot`, L62–116).
- **Win/Draw Detection**:
  - *Python*: `_count_ships(target_grid) == 0` (L241–243) declares shooter winner. No shot limit.
  - *Rust*: `won = g.hits.len() == g.ships.len()` or `g.shots_left == 0` (L107–108), removing game from map.
- **UI/Timeouts/DB**:
  - *Python*: Text/Embed + DM; no timeout; `db_game_submit_score("battleship", uid, 150, cumulative=True)` (L249).
  - *Rust*: Text only; no timeout; no DB scoring.

### 2. `connect4`
- **Python**: `connect4_system.py` (`Connect4View`, L69–173; `register_connect4_game`, L175–217)
- **Rust**: `zenne-games/src/connect4.rs` (`Connect4Manager`, L13–120) + `zenne-discord/src/extra_cmds.rs` (`connect4`, L752–783)
- **Board State & Off-by-One Bug**:
  - Both use 6×7 `[u8; 7]; 6` / `List[List[int]]` boards.
  - **BUG in Rust `zenne-discord/src/extra_cmds.rs:756-757`**: `let idx = c.saturating_sub(1);` is passed to `Connect4Manager::drop_piece(ch, uid, idx)`, which itself expects `col_1_based` in `1..=7` and does `let col = col_1_based - 1;` (`connect4.rs:50-53`). Thus `!connect4 1` fails with `"Cột phải từ 1 đến 7!"`, and `2..=7` drop into columns `1..=6` (column 7 is unreachable).
- **Turn Enforcement**:
  - *Python*: PvP (`opponent` required, self/bot rejected at L180/L200); `Connect4View` checks `interaction.user.id != self.current_turn` (L98).
  - *Rust*: `extra_cmds.rs:776` calls `connect4.start(ch, uid, uid)` — **both players are set to `uid`**, so no opponent can ever join or play, and `uid` plays both 🔴 and 🟡 against themselves.
- **Win/Draw Detection**:
  - Both check 4-in-a-row (horizontal, vertical, both diagonals) and draw when top row is full (`board[0]` all non-zero: Python L140, Rust `connect4.rs:76`).
- **UI/Timeouts/DB**:
  - *Python*: 7 `discord.ui.Button` columns (`Connect4View`, `timeout=90.0`, L72), disables buttons on win/draw, submits `100` pts (`cumulative=True`, L120).
  - *Rust*: Text-only (`!connect4 <1-7>`), no timeout, no DB scoring.

### 3. `memory`
- **Python**: `memory_game_system.py` (`MemoryCardView`, L16–134; `register_memory_game`, L136–168)
- **Rust**: `zenne-games/src/memory.rs` (`MemoryManager`, L13–80) + `zenne-discord/src/lib.rs` (`memory`, L444–471)
- **Board State & Turn Enforcement**:
  - *Python*: 4×4 grid (`16` cards, 8 emoji pairs), per-user `MemoryCardView` (`interaction.user.id != self.user_id` rejected at L65), `self.busy` lock prevents race conditions during 1.2s mismatch reveal (`await asyncio.sleep(1.2)`, L123).
  - *Rust*: Channel-scoped `DashMap<u64, MemoryGame>` (`memory.rs:14`), no `user_id` ownership check (anyone in channel can flip), flips 2 cards at once (`flip(ch, a_1, b_1)`, L48–79) and renders `temp: Option<(usize, usize)>` inline.
- **Win/Draw Detection**:
  - Both win when all 16 cards are revealed/matched (Python `matches_found == 8` L96; Rust `g.revealed.iter().all(|&x| x)` L72).
- **UI/Timeouts/DB**:
  - *Python*: 16 interactive `discord.ui.Button`s (`row=idx//4`, `timeout=90.0`, L18); scores `max(30, int(150 - turns*5 - elapsed*0.5))` with `cumulative=True` (L98–100).
  - *Rust*: Text-only (`!memory 1 5`), no timeout, no DB scoring.

### 4. `typing`
- **Python**: `typing_test_system.py` (`TypingView`, `TypingModal`, `calculate_typing_stats`, L43–211)
- **Rust**: `zenne-games/src/connect4.rs` (`TypingTestManager`, L136–172) + `zenne-discord/src/extra_cmds.rs` (`typing`, L539–559)
- **Anti-Cheat & Prompt Match**:
  - *Python*: Timer starts only when user clicks `TypingStartButton` (`view.start_time = time.time()`, L112) and opens `TypingModal` (prevents pre-typing before modal opens, though copy-paste from embed is still possible). Uses `difflib.SequenceMatcher` for `accuracy` and `correct_chars`, standard `wpm = int((correct_chars / 5) / elapsed_minutes)` (L50–58), and **gates DB submission on `acc >= 60%`** (L139).
  - *Rust*: Starts timer at `!typing` prompt generation (`started_at: Utc::now()`, `connect4.rs:151`). `submit()` requires **exact string match** (`typed.trim() == sess.sentence`, L163) — no partial accuracy/CPM. Clamps minimum elapsed to `500ms` (L161) to prevent division-by-zero/infinite WPM, and counts whitespace words rather than `chars / 5` (L164–165). No `author_id` check (anyone in channel can submit).
- **UI/Timeouts/DB**:
  - *Python*: Button + Modal (`timeout=120`, L124); submits `wpm` (non-cumulative, L141).
  - *Rust*: Text-only (`!typing <câu>`); no timeout; no DB scoring.

### 5. `afk`
- **Python**: `afk_system.py` (`start_afk_task`, L49–101; `register_afk`, L104–350)
- **Rust**: `zenne-games/src/afk.rs` (`AfkManager`, L15–60) + `zenne-discord/src/lib.rs` (`afk` L250–264, `FullEvent::Message` L515–541)
- **AFK Tracking & Anti-Cheat**:
  - *Python*: Explicit session lifecycle (`start`, `stop`, `status`, `top`). Background `@tasks.loop(seconds=45)` checks `member.status == discord.Status.offline` with a **5-minute (300s) grace period** (`offline_since`, L66–70) before auto-stopping and submitting `elapsed` (if `>= 30s`, L78; manual stop requires `>= 10s`, L175). `touch_afk_presence` (L43–46) resets grace timer on activity.
  - *Rust*: Simple mention/auto-reply AFK status (`(guild_id, user_id) -> AfkEntry` with `reason` and `since_ts`, `afk.rs:17`). `FullEvent::Message` (`zenne-discord/src/lib.rs:516`) removes AFK when the user sends a message and notifies mentions (`L528–541`). **No background presence/offline check, no grace period, no `stop`/`status`/`top` subcommands**.
  - **Note on Web API (`zenne-api/src/lib.rs:494-518`)**: Web AFK endpoint enforces `body.score <= elapsed + 60` and monotonic nonce delta tracking (`submit_score = body.score - *prev`), and `zenne-core/src/db.rs:654` sets `cumulative = (game == "afk")`.
- **DB Scoring**:
  - *Python*: Submits `elapsed` seconds (`cumulative=True`, L80, L176, L291).
  - *Rust Discord*: None.

### 6. `sudoku`
- **Python**: `sudoku_system.py` (`register_sudoku_game`, L53–161)
- **Rust**: `zenne-games/src/battleship.rs` (`SudokuManager`, L120–136) + `zenne-discord/src/extra_cmds.rs` (`sudoku`, L682–687)
- **Board State & Gameplay**:
  - *Python*: 5 4×4 templates (`_SUDOKU_4x4_TEMPLATES`, L14–40). `_on_sudoku_message` (L119–160) listens for 16-digit answers, validates against `game["answer"]`, awards `max(50, int(200 - elapsed))` (`cumulative=True`, L143).
  - *Rust*: **Stub only**. `SudokuManager` has only 1 hardcoded 9×9 puzzle (`battleship.rs:126-131`) and `start()` inserts the solution into `self.puzzles`, **but there is no `submit`/`solve`/`check` method anywhere in `zenne-games` or `zenne-discord`**. The puzzle can never be solved or scored.
- **Timeouts**:
  - *Python*: 120s `asyncio.sleep(120)` timeout revealing solution (L86–99).
  - *Rust*: None.

### 7. `uno`
- **Python**: `uno_system.py` (`_build_deck`, `_can_play`, `UnoHandView`, `register_uno_game`, L18–276)
- **Rust**: `zenne-games/src/memory.rs` (`UnoManager`, L90–113) + `zenne-discord/src/extra_cmds.rs` (`uno`, L787–796)
- **Board State, Turn Enforcement & Win Detection**:
  - *Python*: Full 2–6 player game (`!uno @p2 ...`), 108-card deck (`0-9`, `Skip`, `Reverse`, `+2`, `Wild`, `Wild+4`), 7 cards dealt per player, non-Wild starting card, `UnoHandView` with playable card buttons + `Rút Bài` button, turn enforcement (`game["turn"] != uid`), special card effects (`+2`, `+4`, `Skip`, `Reverse`), win on empty hand (`200` pts, `cumulative=True`, L94–109).
  - *Rust*: **Stub only**. `UnoManager::start` (L95–112) inserts `players: vec![host]`, picks one random colored number `top_card`, and returns it. **No deck, no hands, no join, no play/draw, no turn rotation, no win detection, no DB scoring**.

---

## Recommended Tests

1. **Rust Unit Tests (`zenne-games`)**:
   - `connect4::tests::drop_piece_bounds_and_win`: Verify `drop_piece(ch, p1, 1)` and `drop_piece(ch, p1, 7)` succeed (and `0`/`8` return `Err`), plus horizontal/vertical/diagonal win and top-row draw detection.
   - `battleship::tests::shoot_hit_miss_duplicate_and_win`: Verify duplicate coordinate rejection, `shots_left` decrement, win on 4th hit, and loss at `shots_left == 0`.
   - `memory::tests::flip_match_mismatch_and_win`: Verify same-index rejection (`a == b`), already-revealed rejection, `moves` increment, and game removal on 8th match.
   - `typing::tests::submit_min_elapsed_clamp_and_exact_match`: Verify `500ms` floor prevents unbounded WPM, mismatch keeps session active, and exact match removes session.
   - `afk::tests::set_get_remove_and_duration`: Verify `set_afk` default reason, `remove_afk` returns entry once, and `format_duration` handles `>= 0`.
2. **Rust Integration / Regression Tests (`zenne-discord`)**:
   - Regression test for `extra_cmds::connect4` ensuring column `1..=7` is NOT decremented before calling `Connect4Manager::drop_piece` and supports a second player (`opponent: Option<serenity::User>`).
   - Integration test verifying `db.game_submit_score_mode(..., cumulative=true)` is invoked on win for `battleship`, `connect4`, `memory`, `sudoku`, `uno`, and `cumulative=false` for `typing`.
   - Regression test for `FullEvent::Message` + `!afk` prefix command interaction (ensuring `!afk` does not immediately self-trigger `remove_afk`).
3. **Python Unit Tests (`gdrive-bot`)**:
   - `typing_test_system.calculate_typing_stats`: Verify `acc < 60%` does not call `db_game_submit_score` and `elapsed` floor `0.01` min.
   - `uno_system._can_play` & `UnoHandView`: Verify color/value/wild matching, `Reverse` index recalculation, and `Skip` wrap-around.
   - `afk_system`: Mock `discord.Status.offline` across `45s` loop ticks to verify `300s` grace period expiration and `>= 30s` score submission.
