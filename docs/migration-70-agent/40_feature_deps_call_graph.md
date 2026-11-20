# Forensic Audit Report — ID 40

## 1. Python Module Dependency & Call-Graph Map (`C:\Users\Ai Hoi\Downloads\gdrive-bot-source\gdrive-bot\`)

### Entrypoint / Orchestrator: `run_bots.py`
- **`run_all()` (`run_bots.py:L49-112`)**: Spawns up to 4 independent OS subprocesses (`subprocess.Popen`) for `bot.py` (`L61-63`), `telegram_bot.py` (`L66-68`), `twitch_bot.py` (`L71-77`, gated on `TWITCH_OAUTH_TOKEN`), and `revolt_bot.py` (`L80-86`, gated on `REVOLT_BOT_TOKEN`).
- **Single-bot mode (`run_bots.py:L129-163`)**: Dynamically imports and invokes:
  - `run_telegram()` (`L129-132`) -> `import telegram_bot; telegram_bot.main()`
  - `run_discord()` (`L134-137`) -> `import bot; bot.bot.run(bot.DISCORD_TOKEN)`
  - `run_twitch()` (`L139-142`) -> `import twitch_bot; twitch_bot.main()`
  - `run_revolt()` (`L144-147`) -> `import revolt_bot; revolt_bot.main()`

### Core Hub & Discord Bot: `bot.py`
`bot.py` acts as both the Discord bot + `aiohttp` Web/Auth API (`L5652-6328`) + shared DB/Word-Chain/AI core, and wires all feature modules via dependency injection:
- **Imports (`bot.py:L30-56, L218`)**:
  - `afk_system`: `register_afk`, `start_afk_task`, `touch_afk_presence`, `format_duration` (`L30`)
  - `quiz_system`: `register_quiz` (`L31`)
  - `quiz_engine`: `get_quiz_question` (`L32`)
  - 24 minigame `*_system.py` modules (`register_vtv`, `register_wordle`, `register_emoji_game`, `register_typing_test`, `register_trieuphu`, `register_cadao_game`, `register_chinhta_game`, `register_flag_quiz`, `register_history_game`, `register_whoami_game`, `register_mastermind_game`, `register_baucua_game`, `register_higher_lower_game`, `register_wiki_quiz`, `register_anagram_game`, `register_connect4_game`, `register_memory_game`, `register_tamsao_game`, `register_taixiu_game`, `register_catword_game`, `register_battleship_game`, `register_uno_game`, `register_sudoku_game`) (`L33-55`)
  - `music_system`: `register_music`, `init_lavalink` (`L56`)
  - `i18n_data`: `TRANSLATIONS as _UI_TRANSLATIONS` (`L218`, used at `L261`)
- **Call Graph from `bot.py` into feature modules**:
  - **Command & DB-callback injection (`bot.py:L2268-2293`)**:
    - `register_afk(bot, db_game_submit_score, db_game_user_score, db_game_top)` (`L2268`)
    - `register_vtv(...)` (`L2270`) and `register_emoji_game(...)` (`L2272`) receive `(bot, db_game_submit_score, db_game_user_score, db_game_top)`
    - 22 other `register_*(bot, db_game_submit_score, db_game_user_score)` calls (`L2269, L2271, L2273-2292`)
    - `register_music(bot)` (`L2293`)
  - **Lifecycle & message hooks**:
    - `await init_lavalink(bot)` (`L2587` in `on_ready`)
    - `start_afk_task(bot, db_game_submit_score)` (`L2591` in `on_ready`)
    - `touch_afk_presence(message.author.id)` (`L2635` in `on_message`)

### Quiz Engine Subgraph
- `quiz_engine.py:L11-13` imports `quiz_helpers.make_distractors`, `quiz_math.ProceduralMath`, `quiz_science.ProceduralScience`.
- `quiz_math.py:L12` and `quiz_science.py:L15` both import `quiz_helpers.make_distractors`.
- `quiz_system.py:L15` imports `quiz_engine.get_quiz_question`.
- None of the `*_system.py` or `music_system.py` modules directly `import bot`; callbacks flow strictly top-down via `register_*(bot, db_*)` arguments.

### Cross-Platform Bots (`telegram_bot.py`, `twitch_bot.py`, `revolt_bot.py`)
All three import shared DB, word-chain, public-room, and AI functions from `bot.py`, plus raw data/helpers from minigame modules:
- **`telegram_bot.py`**: Imports 35+ shared helpers from `bot.py` (`L55-91`), plus `quiz_engine`, `vuatiengviet_system`, `cadao_game_system`, `chinhta_game_system`, `flag_quiz_system`, `trieuphu_system`, `wordle_system` (`L94-100`).
- **`twitch_bot.py`**: Imports shared DB/word-chain/AI helpers from `bot.py` (`L42-62`), plus `quiz_engine`, `vuatiengviet_system`, `cadao_game_system`, `chinhta_game_system`, `flag_quiz_system` (`L65-69`).
- **`revolt_bot.py`**: Identical imports to `twitch_bot.py`.

---

## 2. Rust Workspace Crate Dependency Graph (`C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\`)

- **Root `Cargo.toml:L62-70`** (`src/main.rs:L25-82`): Depends on all 8 workspace crates (`zenne-core`, `zenne-games`, `zenne-music`, `zenne-api`, `zenne-discord`, `zenne-telegram`, `zenne-twitch`, `zenne-revolt`).
- **`zenne-core`** (`crates/zenne-core/Cargo.toml`): Leaf crate (`auth`, `cache`, `config`, `db`, `i18n`, `kira`, `wiki`, `AppContext`).
- **`zenne-games`** (`crates/zenne-games/Cargo.toml:L7`): Declares `zenne-core`. Exposes `GameHub` + 22 modules.
- **`zenne-music`** (`crates/zenne-music/Cargo.toml:L7`): Declares `zenne-core`. Exposes `MusicManager` and `multiplay::MultiPlayManager`.
- **`zenne-api`**, **`zenne-discord`**, **`zenne-telegram`**, **`zenne-twitch`**, **`zenne-revolt`**: Depend on `zenne-core` + `zenne-games` (and `zenne-music` for `zenne-discord`).

---

## 3. Missing Cross-Module Integrations & Discrepancies in Rust

1. **Missing `wiki_quiz` Module in `zenne-games` & `zenne-discord` (Dead Catalog Entry in `zenne-api`)**:
   - **Python**: `bot.py:L46` imports `register_wiki_quiz` from `wiki_quiz_system.py` and registers it at `bot.py:L2283`.
   - **Rust**: `crates/zenne-api/src/lib.rs:L433` still advertises `{"id": "wikiquiz", "name": "Đoán Định Nghĩa Wiki", ...}` in `/pub/games`, **but `crates/zenne-games/src/lib.rs` has no `wiki_quiz` module** and `zenne-discord` has no `wikiquiz` command.

2. **Missing AFK Background Task (`start_afk_task`) & Presence Tracking (`touch_afk_presence`)**:
   - **Python**: `bot.py:L30` imports `start_afk_task` and `touch_afk_presence` from `afk_system.py`; calls `start_afk_task(bot, db_game_submit_score)` in `on_ready` (`bot.py:L2591`) and `touch_afk_presence(message.author.id)` in `on_message` (`bot.py:L2635`).
   - **Rust**: `crates/zenne-games/src/afk.rs:L25-59` only implements manual `set_afk`, `remove_afk`, `get_afk`, `format_duration`. `crates/zenne-discord/src/lib.rs:L516-541` only clears AFK on message and notifies on mention — there is **no `touch_afk_presence` idle tracker** and **no `start_afk_task` periodic background task** submitting AFK scores to `db`.

3. **Missing `flag_quiz` Integration in `zenne-telegram`**:
   - **Python**: `telegram_bot.py:L98` imports `_COUNTRIES_DATA` from `flag_quiz_system.py` and uses it at `telegram_bot.py:L1313, L1315`.
   - **Rust**: `zenne-telegram` does **not** use `games.flag_quiz` anywhere (`crates/zenne-telegram/src/*.rs` only integrates `baucua`, `cadao`, `chinhta`, `quiz`, `taixiu`, `trieuphu`, `vuatiengviet`, `wordle`), whereas `zenne-discord/src/extra_cmds.rs:L445,L455`, `zenne-twitch/src/lib.rs:L102,L309`, and `zenne-revolt/src/lib.rs:L129,L328` all wire `games.flag_quiz`.

4. **Three Unused `Cargo.toml` Cross-Crate Dependencies (Declared but Zero Source Usage)**:
   - `crates/zenne-api/Cargo.toml:L8` declares `zenne-games = { path = "../zenne-games" }`, **but `crates/zenne-api/src/*.rs` never imports or calls `zenne_games`**.
   - `crates/zenne-music/Cargo.toml:L7` declares `zenne-core = { path = "../zenne-core" }`, **but `crates/zenne-music/src/*.rs` never imports or calls `zenne_core`**.
   - `crates/zenne-games/Cargo.toml:L7` declares `zenne-core = { path = "../zenne-core" }`, **but `crates/zenne-games/src/*.rs` never imports or calls `zenne_core`**.

5. **Non-Discord `i18n` Parity**:
   - `zenne_core::i18n` (`t`, `lang_display_name`, `is_known_ui_lang`) is integrated only in `zenne-discord` (`crates/zenne-discord/src/lib.rs:L114,L126-131`, `extra_cmds.rs:L123`, `noitu.rs:L81,L96,L111-112`) and is not wired into `zenne-telegram`, `zenne-twitch`, or `zenne-revolt` (matching Python where only `bot.py:L218` imports `i18n_data`).
