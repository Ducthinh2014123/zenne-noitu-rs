# Forensic Audit: Discord Prefix Commands (Legacy Python vs. Rust `zenne-discord`)

**Target Output Path:** `docs/migration-70-agent/72_prefix_command_forensic_audit.md`  
**Audited Sources:**
- Legacy Python: `C:\Users\Ai Hoi\Downloads\gdrive-bot-source\gdrive-bot\` (`bot.py` + 26 `*_system.py` modules)
- Current Rust: `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-discord\src\` (`lib.rs`, `extra_cmds.rs`, `music.rs`, `noitu.rs`, `workers.rs`)
- Related Recovery Surface: `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-api\src\auth_routes.rs`, `C:\Users\Ai Hoi\Downloads\zenne-noitu-deploy-vercel\pages\auth\reset-password.jsx`

---

## 1. Executive Summary

- **Legacy Python Discord Prefix Commands:** **63** `@bot.command` definitions (**26** in `bot.py`, **37** across 26 `*_system.py` modules). Zero `@commands.command`, `@commands.group`, `@bot.group`, `@bot.hybrid_command`, or dynamic `bot.add_command` registrations were found.
- **Rust `zenne-discord` Commands:** **60** `#[poise::command]` definitions (**19** in `lib.rs`, **8** in `music.rs`, **33** in `extra_cmds.rs`).
  - **All 60** are registered in `all_commands()` (`lib.rs:679-742`) — **0 unregistered/dead commands**.
  - **59** are dual `#[poise::command(slash_command, prefix_command)]`; **1** (`gopthemtu` at `lib.rs:260`) is `prefix_command` only.
- **Missing in Rust (from Python prefix commands):**
  - **Completely missing (3):** `!wikiquiz` (`wiki_quiz_system.py:125`), `!khoahoc` (`quiz_system.py:262`), `!uno_hand` (`uno_system.py:233`).
  - **Removed / Merged / Renamed without legacy primary name (9):** `!help_noitu` (replaced by `!help`), `!batdautro` (merged into `!start` as alias), `!tutruong` (merged into `!danhsachtu` as alias), `!toan` (replaced by `!quizmath`), `!loop` (renamed to `!rloop`), `!duoihinh` (renamed to `!emoji`), `!flagquiz` (renamed to `!flag`), `!vuatiengviet` (renamed to `!vtv`), `!quizz` (renamed to `!quiz`), `!setngonngu` (renamed to `!setlang` with `setngonngu` as alias).
  - **Missing legacy aliases:** Extensive alias loss across migrated commands (see Matrix in Section 5).
- **New in Rust (not Python prefix commands):** `!help` (`lib.rs:27`), `!thamgiagiaidau` (`extra_cmds.rs:712`), `!duyettu` (`extra_cmds.rs:743`).
- **`admin_recovery` / `resetpassword_admin` Finding:** No command named `admin_recovery` exists in Python, Rust, or Web. In Python, Admin Recovery Token generation is a **slash-only** command `@bot.tree.command(name="resetpassword_admin")` (`bot.py:5151-5178`), paired with Web UI (`reset-password.jsx:829, 1042, 1094`) and Rust API `handle_recovery_token_verify` (`auth_routes.rs:978`). **Rust `zenne-discord` has NO `resetpassword_admin` (slash or prefix) command**, leaving Admin Recovery Token generation unimplemented in the Rust Discord bot.

---

## 2. Prefix Parsing, Registration & Dispatch Mechanics

### 2.1 Legacy Python (`bot.py`)
- **Bot Initialization (`bot.py:2265`):** `bot = commands.Bot(command_prefix="!", intents=intents, help_command=None, proxy=DISCORD_PROXY)`
- **Custom `on_message` Dispatch (`bot.py:2631-2645`):** Calls `touch_afk_presence`, `bot.process_commands`, skips `"!"`/`"/"` in Nối Từ. 12 minigame modules use `@bot.listen("on_message")` for unprefixed answers.
- **Error Handling (`bot.py:2596-2610`):** Typo suggestions + missing arg messages.

### 2.2 Rust `zenne-discord` (`crates/zenne-discord/src/`)
- **Framework Configuration (`lib.rs:633-644`):** `prefix: Some("!".into())`, `all_commands()` registers all 60 commands.
- **Music Workers (`workers.rs:48-95`):** `GUILDS | GUILD_VOICE_STATES` only; no prefix commands.

---

## 3. Forensic Investigation: `admin_recovery` & Related Recovery Commands

| Surface | Symbol / Route | Type | Status & Details |
| :--- | :--- | :--- | :--- |
| **Python Discord** | `slash_resetpassword_admin` (`bot.py:5151-5178`) | `@bot.tree.command(name="resetpassword_admin")` | **Slash-only**. Checks `_is_bot_admin(interaction)`. Generates CSPRNG token, stores hash in `password_recovery` (`type="recovery_token"`, `source="admin"`, `TTL=480s`), replies ephemerally. |
| **Python Discord** | `cmd_revoke_2fa` (`bot.py:5218-5251`) & `slash_revoke_2fa` (`bot.py:5181-5215`) | Both Prefix & Slash | Checks `is_super_admin` or guild `administrator`. Clears `totp_enabled=0, totp_secret=NULL, backup_codes=NULL`. |
| **Web Frontend** | `pages/auth/reset-password.jsx:829, 1042, 1094` | Next.js UI | References `"Admin Recovery Token"` and submits to `/api/auth/recovery/token/verify`. |
| **Rust API** | `handle_recovery_token_verify` (`auth_routes.rs:978-1020`) | Axum HTTP | Verifies `password_recovery` tokens (`type = 'recovery_token'`). |
| **Rust Discord** | `extra_cmds::revoke_2fa` (`extra_cmds.rs:340-362`) | Dual Slash + Prefix | Super Admin only, case-sensitive SQL, does not clear `backup_codes`. |
| **Rust Discord** | `resetpassword_admin` / `admin_recovery` | **MISSING** | **Not implemented** anywhere in `zenne-discord/src/`. |

---

## 4. Collisions & Behavioral Divergences (Prefix vs. Slash)

1. **`!stop` vs `/stop` (Music vs. Nối Từ):** Python `/stop` is Music, `!stop` is Nối Từ. Rust `/stop` & `!stop` are Nối Từ, `/mstop` & `!mstop` are Music.
2. **`!gopthemtu` (`lib.rs:260`):** `prefix_command` only in Rust.
3. **No Intra-Registry Collisions in Rust:** All 60 command names and aliases in `all_commands()` are distinct.

---

## 5. Complete Python-to-Rust Prefix Command Parity Matrix

### 5.1 Core, Nối Từ, AI & Admin (`bot.py` vs `lib.rs` / `extra_cmds.rs`)

| Python Command | Python Location | Python Aliases | Rust Command | Rust Location | Rust Aliases | Parity Status |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `!setnoitu` | `bot.py:2783` | — | `setnoitu` | `extra_cmds.rs:6` | — | **Full** |
| `!setngonngu` | `bot.py:2797` | `setlang`, `lang`, `ngonngu`, `language` | `setlang` | `lib.rs:165` | `setngonngu` | **Partial** |
| `!ngonngu_hientai` | `bot.py:2844` | `currentlang`, `xemlang` | `ngonngu_hientai` | `extra_cmds.rs:129` | — | **Partial** |
| `!setuilang` | `bot.py:2854` | `uilang`, `doingondngiaodien`, `ngugiaodien` | `setuilang` | `lib.rs:185` | — | **Partial** |
| `!listlang` | `bot.py:2874` | `langlist`, `tramalang`, `danhsachlang` | `listlang` | `extra_cmds.rs:143` | — | **Partial** |
| `!start` | `bot.py:2898` | — | `start` | `lib.rs:70` | `batdautro`, `bdt`, `startgame` | **Divergent** |
| `!thamgia` | `bot.py:2937` | `join`, `tg` | `thamgia` | `extra_cmds.rs:25` | `join` | **Partial** |
| `!batdautro` | `bot.py:2951` | `bdt`, `startgame` | *(alias on `start`)* | `lib.rs:73` | — | **Merged** |
| `!stop` | `bot.py:2986` | `end` | `stop` | `lib.rs:125` | — | **Partial** |
| `!gopthemtu` | `bot.py:3009` | `addword`, `gtt` | `gopthemtu` | `lib.rs:260` | — | **Partial** |
| `!batdaulai` | `bot.py:3047` | `newword` | `batdaulai` | `extra_cmds.rs:48` | `newword` | **Full** |
| `!diem` | `bot.py:3071` | `score`, `scores` | `diem` | `extra_cmds.rs:78` | `score` | **Partial** |
| `!tutruong` | `bot.py:3087` | `wordbank`, `bank` | *(alias on `danhsachtu`)* | `extra_cmds.rs:165` | — | **Merged** |
| `!tratu` | `bot.py:3108` | `check`, `kt`, `tra` | `tratu` | `lib.rs:200` | — | **Partial** |
| `!contukhong` | `bot.py:3190` | `ctk`, `kiemtranoi`, `noiduockhong` | `contukhong` | `extra_cmds.rs:100` | `ctk` | **Partial** |
| `!top` | `bot.py:3327` | `lb`, `bxh`, `xephang` | `top` | `lib.rs:232` | `lb` | **Partial** |
| `!tochucgiaidau` | `bot.py:3411` | `tournament`, `giaidau`, `td` | `tochucgiaidau` | `extra_cmds.rs:256` | — | **Partial** |
| `!ketthucgiaidau` | `bot.py:3479` | `endtournament`, `ketgiai`, `kg` | `ketthucgiaidau` | `extra_cmds.rs:318` | — | **Partial** |
| `!xemgiaidau` | `bot.py:3520` | `tournament_info`, `gd` | `xemgiaidau` | `extra_cmds.rs:288` | — | **Partial** |
| `!cachestats` | `bot.py:3547` | `cache`, `cs` | `cachestats` | `extra_cmds.rs:153` | — | **Partial** |
| `!help_noitu` | `bot.py:3594` | `helpnoitu`, `huongdan` | `help` *(renamed)* | `lib.rs:27` | — | **Renamed/Replaced** |
| `!ask` | `bot.py:4121` | `hoi`, `ai`, `hoiai` | `ask` | `lib.rs:286` | — | **Partial** |
| `!danhsachtu` | `bot.py:4886` | `khotudu`, `frombank` | `danhsachtu` | `extra_cmds.rs:165` | `tutruong`, `wordbank` | **Partial** |
| `!taoroom` | `bot.py:4921` | `taophong`, `webgame`, `choinoitu` | `taoroom` | `extra_cmds.rs:190` | — | **Partial** |
| `!multiplayer` | `bot.py:5101` | `mp`, `publicrooms`, `choicongkhai` | `multiplayer` | `extra_cmds.rs:227` | `mp` | **Partial** |
| `!revoke_2fa` | `bot.py:5218` | — | `revoke_2fa` | `extra_cmds.rs:340` | — | **Partial** |
| *(UI button only)* | `bot.py:3390` | — | `thamgiagiaidau` | `extra_cmds.rs:712` | `joingd` | **New in Rust** |
| *(Slash-only `/kiemduyettu`)* | `bot.py:3638` | — | `duyettu` | `extra_cmds.rs:743` | `kiemduyettu` | **New Prefix in Rust** |

### 5.2 Music Commands (`music_system.py` vs `music.rs` / `extra_cmds.rs`)

| Python Command | Python Location | Python Aliases | Rust Command | Rust Location | Rust Aliases | Parity Status |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `!play` | `music_system.py:537` | `p` | `play` | `music.rs:281` | `p`, `sc` | **Full** |
| `!pause` | `music_system.py:563` | — | `pause` | `music.rs:560` | — | **Full** |
| `!resume` | `music_system.py:574` | — | `resume` | `music.rs:579` | — | **Full** |
| `!skip` | `music_system.py:585` | `next`, `fs` | `skip` | `music.rs:598` | `s`, `next` | **Partial** |
| `!mstop` | `music_system.py:595` | `leave`, `disconnect` | `mstop` | `music.rs:617` | `musicstop`, `dc`, `leave` | **Partial** |
| `!queue` | `music_system.py:605` | `q` | `queue` | `music.rs:665` | `q` | **Full** |
| `!nowplaying` | `music_system.py:633` | `np` | `nowplaying` | `extra_cmds.rs:365` | `np` | **Full** |
| `!volume` | `music_system.py:657` | `vol` | `volume` | `music.rs:709` | `vol` | **Full** |
| `!loop` | `music_system.py:667` | — | `rloop` | `music.rs:733` | — | **Renamed** |

### 5.3 Minigames & AFK (`*_system.py` vs `lib.rs` / `extra_cmds.rs`)

| Python Command | Python Location | Python Aliases | Rust Command | Rust Location | Rust Aliases | Parity Status |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `!afk` | `afk_system.py:256` | — | `afk` | `lib.rs:312` | — | **Divergent** |
| `!anagram` | `anagram_game_system.py:109` | `doichu`, `ghepchu` | `anagram` | `lib.rs:452` | — | **Partial** |
| `!battleship` | `battleship_system.py:150` | `thuyachien`, `naval` | `battleship` | `lib.rs:475` | — | **Divergent** |
| `!baucua` | `baucua_system.py:181` | `tomca`, `bc` | `baucua` | `lib.rs:347` | `bc` | **Partial** |
| `!cadao` | `cadao_game_system.py:199` | `tucngu`, `thanhngu` | `cadao` | `extra_cmds.rs:391` | — | **Partial** |
| `!catword` | `catword_system.py:165` | `noitu_cd`, `noituchude`, `ntcd` | `catword` | `extra_cmds.rs:655` | — | **Partial** |
| `!chinhta` | `chinhta_game_system.py:205` | `soiloict`, `ct` | `chinhta` | `extra_cmds.rs:415` | — | **Partial** |
| `!connect4` | `connect4_system.py:198` | `c4`, `noi4` | `connect4` | `extra_cmds.rs:773` | `c4` | **Partial** |
| `!duoihinh` | `emoji_game_system.py:187` | `dhbc`, `emojiword` | `emoji` | `extra_cmds.rs:439` | — | **Renamed** |
| `!flagquiz` | `flag_quiz_system.py:246` | `co`, `doanco`, `flag` | `flag` | `extra_cmds.rs:463` | — | **Renamed** |
| `!higherlower` | `higher_lower_system.py:159` | `hl`, `sokeo`, `caothap` | `higherlower` | `extra_cmds.rs:584` | — | **Partial** |
| `!history` | `history_guess_system.py:180` | `lichsu`, `namlichsu` | `history` | `extra_cmds.rs:487` | — | **Partial** |
| `!mastermind` | `mastermind_system.py:102` | `matma`, `bekhoa`, `mm` | `mastermind` | `extra_cmds.rs:532` | — | **Partial** |
| `!memory` | `memory_game_system.py:154` | `latthe`, `trinho` | `memory` | `lib.rs:506` | — | **Partial** |
| `!quizz` | `quiz_system.py:233` | `quiz`, `giaido`, `dautri` | `quiz` | `lib.rs:386` | — | **Renamed** |
| `!toan` | `quiz_system.py:248` | — | `quizmath` | `lib.rs:409` | — | **Renamed/Replaced** |
| `!khoahoc` | `quiz_system.py:262` | `khtn` | — | — | — | **MISSING IN RUST** |
| `!sudoku` | `sudoku_system.py:111` | `sdk` | `sudoku` | `extra_cmds.rs:703` | — | **Partial** |
| `!taixiu` | `taixiu_system.py:134` | `tx`, `xucxac` | `taixiu` | `lib.rs:329` | `tx` | **Partial** |
| `!tamsao` | `tamsao_system.py:147` | `tamcao`, `dich`, `chaintranslate` | `tamsao` | `extra_cmds.rs:679` | — | **Partial** |
| `!trieuphu` | `trieuphu_system.py:385` | `altp`, `millionaire` | `trieuphu` | `extra_cmds.rs:612` | — | **Partial** |
| `!typing` | `typing_test_system.py:191` | `gochu`, `wpm` | `typing` | `extra_cmds.rs:560` | — | **Partial** |
| `!uno` | `uno_system.py:182` | `staruno`, `batdauuno` | `uno` | `extra_cmds.rs:812` | — | **Partial** |
| `!uno_hand` | `uno_system.py:233` | `unohand`, `bai` | — | — | — | **MISSING IN RUST** |
| `!vuatiengviet` | `vuatiengviet_system.py:185` | `vtv`, `xapchu` | `vtv` | `lib.rs:363` | — | **Renamed** |
| `!whoami` | `whoami_system.py:170` | `danhnhan`, `ailasi` | `whoami` | `extra_cmds.rs:508` | — | **Partial** |
| `!wikiquiz` | `wiki_quiz_system.py:125` | `dinhnghia`, `wiki` | — | — | — | **MISSING IN RUST** |
| `!wordle` | `wordle_system.py:275` | — | `wordle` | `lib.rs:420` | — | **Partial** |
