# 71. Discord Slash Command Forensic Parity Audit (Python vs Rust)

**Audit Scope:**
- **Legacy Python Source:** `C:\Users\Ai Hoi\Downloads\gdrive-bot-source\gdrive-bot\` (`bot.py`, `music_system.py`, and 24 `*_system.py` modules), plus top-level `C:\Users\Ai Hoi\Downloads\gdrive-bot-source\bot.py` and `C:\Users\Ai Hoi\Downloads\gdrive-bot-source\music_system.py`.
- **Current Rust Implementation:** `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-discord\src\` (`lib.rs`, `extra_cmds.rs`, `music.rs`, `noitu.rs`, `workers.rs`) and `zenne-api/src/auth_routes.rs`.
- **Web Frontend:** `C:\Users\Ai Hoi\Downloads\zenne-noitu-deploy-vercel\` (`pages/auth/reset-password.jsx`, `pages/auth/login.jsx`, `pages/account/index.jsx`, `pages/api/auth/recovery/token/verify.js`).

---

## 1. Executive Summary

| Metric | Python (`gdrive-bot/`) | Rust (`zenne-discord/src/`) |
|---|---|---|
| Total `@bot.tree.command` slash commands | **52** (18 in `bot.py`, 9 in `music_system.py`, 25 in `*_system.py`) | — |
| Total `#[poise::command(slash_command, ...)]` | — | **59** (18 in `lib.rs`, 8 in `music.rs`, 33 in `extra_cmds.rs`) |
| Total `#[poise::command(prefix_command)]` (no `slash_command`) | — | **1** (`gopthemtu` at `lib.rs:260`) |
| Total commands registered in `all_commands()` | — | **60** (`lib.rs:679-742`) |
| Unregistered `#[poise::command]` handlers in Rust | — | **0** |
| Python slash commands **missing** in Rust | **3** (`/resetpassword_admin`, `/wikiquiz`, and `/uno` is a stub in both but diverges) | — |
| Python slash commands **renamed** in Rust | **12** (see Section 4) | — |
| Rust-only slash commands (promoted from Python prefix-only) | — | **11** (`setnoitu`, `thamgia`, `batdaulai`, `ngonngu_hientai`, `cachestats`, `xemgiaidau`, `ketthucgiaidau`, `thamgiagiaidau`, `quizmath`, `help`, and split `quiz`/`quizmath`) |

**Critical Findings:**
1. **`/resetpassword_admin` (`admin_recovery`) is completely missing in Rust**, breaking the Web UI's "Admin Recovery" flow (`pages/auth/reset-password.jsx:986-1100`).
2. **`/wikiquiz` (`wiki_quiz_system.py:113`) is completely missing in Rust.**
3. **`/revoke_2fa` (`extra_cmds.rs:340-362`) has 4 regressions:** tighter permission check (Super Admin only vs Super Admin + Guild Admin), case-sensitive SQL lookup, fails to clear `backup_codes`, and always reports success publicly even when the user does not exist or has 2FA disabled.
4. **`/gopthemtu` (`lib.rs:260`) is registered as `prefix_command` only** (missing `slash_command`).
5. **12 command-name mismatches** between Python and Rust (e.g., `/noitu_start` → `/start`, `/noitu_stop` → `/stop`, `/stop` music → `/mstop`, `/loop` → `/rloop`, `/vuatiengviet` → `/vtv`, `/quizz` → `/quiz`, `/duoihinh` → `/emoji`, `/flagquiz` → `/flag`, `/kiemduyettu` → `/duyettu`).
6. **Multiple parameter/schema regressions:** `/afk` lost `action` choices; `/wordle` lost `ngon_ngu` (`en`/`vi`) choice; `/quiz` lost `mon` and `cap` choices; `/top` lost `mode` (`words`/`streak`) choice; `/taoroom` replaced `(lang, time_limit)` with `(name)`; `/battleship` and `/connect4` replaced `opponent: discord.User` with solo coordinate/column input; `/duyettu` replaced `guild_id: str` interactive `ApproveView` with `(id: i64, approve: Option<bool>)`.

---

## 2. Deep-Dive: `admin_recovery` / `resetpassword_admin` / `revoke_2fa`

### 2.1 Python Legacy (`gdrive-bot/bot.py`)
- **Admin Password Recovery Slash Command (`gdrive-bot/bot.py:5151-5178` & `gdrive-bot-source/bot.py:5145-5172`):**
  - **Name:** `/resetpassword_admin` (no prefix alias; slash-only).
  - **Signature:** `async def slash_resetpassword_admin(interaction: discord.Interaction, email: str)`
  - **Permission Check:** `_is_bot_admin(interaction)` (`bot.py:5141-5148`) — allows Guild `administrator` permission OR `is_super_admin(interaction.user.id)`.
  - **Behavior:**
    1. Queries `SELECT id FROM web_users WHERE email=?` (lowercased, trimmed).
    2. Generates a one-time `admin_token = _gen_recovery_token()`, hashes it (`_hash_token`), and inserts into `password_recovery(user_id, token_hash, type, source, expires_at, attempts)` with `type="recovery_token"`, `source="admin"`.
    3. Sends an **ephemeral** embed containing the raw `admin_token` and instructions for the user to open `/auth/reset-password` on the web and select `'Admin Recovery'`.
- **Admin 2FA Revocation (`gdrive-bot/bot.py:5181-5215` slash, `5218+` prefix):**
  - **Name:** `/revoke_2fa` (and `!revoke_2fa`).
  - **Parameter:** `target: str` (Email or Username).
  - **Permission Check:** `is_super_admin(interaction.user.id) or _is_bot_admin(interaction)`.
  - **Behavior:**
    1. Queries `SELECT id, username, email, totp_enabled FROM web_users WHERE LOWER(email)=? OR LOWER(username)=?`.
    2. Returns an ephemeral error if not found, or an ephemeral notice if `totp_enabled` is already `0`.
    3. Executes `UPDATE web_users SET totp_enabled=0, totp_secret=NULL, backup_codes=NULL WHERE id=?`.
    4. Replies **ephemerally** with account details.

### 2.2 Web Frontend (`zenne-noitu-deploy-vercel`)
- **`pages/auth/reset-password.jsx`:**
  - Lines `157`, `829`, `839`, `986-1100`: Presents the `admin_contact` step instructing users who lost email access to contact Admin on Discord (`toilathangvnxd`), obtain an **Admin Recovery Token**, and paste it into the `"Nhập Admin Recovery Token"` input (or Console Recovery Script at lines `63-81`).
  - Submits token to `POST /api/auth/recovery/token/verify` (`pages/api/auth/recovery/token/verify.js:11`), which proxies to Rust `POST /pub/auth/recovery/token/verify`.
  - Lines `1432-1462`: Also instructs users who lost 2FA + backup codes to contact Admin on Discord to revoke 2FA.
- **`pages/auth/login.jsx:632-653` & `pages/account/index.jsx:149`:** Same Discord Admin contact flow for 2FA removal.

### 2.3 Rust Implementation (`zenne-noitu-rs`)
- **`/resetpassword_admin` / `admin_recovery`:**
  - **Discord (`crates/zenne-discord/src/`):** **NOT IMPLEMENTED.** Zero occurrences of `resetpassword_admin` or `admin_recovery`.
  - **HTTP API (`crates/zenne-api/src/auth_routes.rs:978-1045`):** `handle_recovery_token_verify` still queries `password_recovery WHERE token_hash = ?1 AND type IN ('recovery_token', 'reset_token') AND used_at IS NULL`, so the verification endpoint exists, **but `type = 'recovery_token'` is only ever inserted by `handle_recovery_email_verify` (`auth_routes.rs:959`)**. There is no admin slash command and no admin HTTP endpoint to mint an admin recovery token.
  - **Impact:** The Web UI's "Admin Recovery" workflow is **non-functional** in Rust.
- **`/revoke_2fa` (`crates/zenne-discord/src/extra_cmds.rs:340-362`, registered at `lib.rs:722`):**
  - **Parameter renamed:** `target: str` → `identifier: String`.
  - **Permission regression:** Only checks `super_admin_ids.contains(&uid)` (`extra_cmds.rs:343`); Guild `Administrator`s can no longer run it.
  - **SQL & Security regressions:**
    - Executes `UPDATE web_users SET totp_enabled = 0, totp_secret = NULL WHERE username = ?1 OR email = ?1` (`extra_cmds.rs:351`).
    - **Case-sensitive** (dropped `LOWER(...)`).
    - **Does NOT clear `backup_codes`** (leaves old backup codes intact in DB).
    - **No existence / status check:** Always responds `"✅ Đã gỡ 2FA cho tài khoản ..."` even if no row matched.
    - **Public response** (`ctx.say`) instead of ephemeral.

---

## 3. Complete Python-to-Rust Slash Command Parity Matrix

### 3.1 Core Nối Từ, Language, AI & Admin Commands (`gdrive-bot/bot.py`)

| # | Python Slash Command | Python Location | Rust Command (`zenne-discord`) | Rust Location | Parity Status & Behavioral Diffs |
|---|---|---|---|---|---|
| 1 | `/contukhong` `[syllable]` | `bot.py:3285` | `/contukhong` `<syllable>` | `extra_cmds.rs:100`, `lib.rs:712` | **Schema Mismatch:** Python `syllable` is `Optional[str]` (defaults to active game's `last_syllable`); Rust `syllable: String` is **required**. |
| 2 | `/kiemduyettu` `[guild_id]` | `bot.py:3638` | `/duyettu` `<id> [approve]` | `extra_cmds.rs:743`, `lib.rs:738` | **Renamed + Major Redesign:** Python lists up to 20 pending words with interactive Approve/Reject buttons (`ApproveView`); Rust requires manual `id: i64` and `approve: Option<bool>`. **Permission diff:** Python allows `super_admin` or guild `administrator`/`manage_channels`; Rust restricts to `super_admin_ids` only (`extra_cmds.rs:750`). |
| 3 | `/noitu_setlang` `<lang_code>` | `bot.py:3729` | `/setlang` `<code>` | `lib.rs:165`, `lib.rs:684` | **Renamed + Validation Diff:** Python validates `{lc}.wikipedia.org` via `validate_lang(lc)`; Rust accepts any string without Wikipedia validation. Rust checks `is_guild_admin`. |
| 4 | `/setuilang` `[ma]` | `bot.py:3756` | `/setuilang` `<code>` | `lib.rs:185`, `lib.rs:685` | **Schema & Permission Diff:** Python `ma` is optional (shows current UI lang if omitted) and requires `_is_bot_admin` to change; Rust `code: String` is **required** and has **no admin check**. |
| 5 | `/listlang` `[tukhoa]` | `bot.py:3783` | `/listlang` (no args) | `extra_cmds.rs:143`, `lib.rs:714` | **Missing Parameter & Truncation:** Python searches 300+ languages by `tukhoa` (ephemeral); Rust takes no args and prints a static 10-language message. |
| 6 | `/noitu_start` | `bot.py:3803` | `/start` `[mode]` | `lib.rs:70`, `lib.rs:682` | **Renamed + Behavior Change:** Python opens a joining lobby (`joining=True`); Rust immediately starts an active game and adds optional `mode` (`"bot"`). |
| 7 | `/noitu_stop` | `bot.py:3832` | `/stop` | `lib.rs:125`, `lib.rs:683` | **Renamed:** Matches host/admin check and stops active game; Rust also persists `lb_add_game` stats. |
| 8 | `/noitu_score` | `bot.py:3853` | `/diem` | `extra_cmds.rs:78`, `lib.rs:711` | **Renamed:** Equivalent in-memory room score display. |
| 9 | `/tochucgiaidau` `<ten_giai> [phan_thuong]` | `bot.py:3872` | `/tochucgiaidau` `<name> [prize]` | `extra_cmds.rs:256`, `lib.rs:719` | **Functional Regression:** Rust omits the `TournamentJoinView` button, omits duplicate-active-tournament check, and renames params. |
| 10 | `/top_noitu` `[mode]` | `bot.py:3944` | `/top` (no args) | `lib.rs:232`, `lib.rs:687` | **Renamed + Missing Parameter:** Python has `mode` choice (`words` / `streak`); Rust has no parameter and only shows `total_words` top 10. |
| 11 | `/tratu` `<word>` | `bot.py:3985` | `/tratu` `<word>` | `lib.rs:200`, `lib.rs:686` | **Parity (Minor Diff):** Both check word validity; Rust uses `#[rest]` and shows extract text. |
| 12 | `/noitu_help` | `bot.py:4035` | `/help` | `lib.rs:27`, `lib.rs:681` | **Renamed + Expanded:** Python shows Nối Từ help (ephemeral); Rust shows full bot command help (public). |
| 13 | `/ask` `<cau_hoi>` | `bot.py:4100` | `/ask` `<prompt>` | `lib.rs:286`, `lib.rs:689` | **Parameter Renamed + Output Diff:** `cau_hoi` → `prompt`; Python sends an embed up to 4000 chars; Rust sends plain text truncated to 1900 chars. |
| 14 | `/danhsachtu` `[trang]` | `bot.py:4903` | `/danhsachtu` (no args) | `extra_cmds.rs:165`, `lib.rs:716` | **Missing Parameter:** Python supports pagination (`trang: int = 1`, 10/page); Rust takes no args and hardcodes 20 latest words. |
| 15 | `/taoroom` `[lang] [time_limit]` | `bot.py:4937` | `/taoroom` `[name]` | `extra_cmds.rs:190`, `lib.rs:717` | **Schema Mismatch:** Python takes `(lang="vi", time_limit=30)`; Rust takes `(name: Option<String>)` and uses the guild's DB language. |
| 16 | `/multiplayer` | `bot.py:5121` | `/multiplayer` | `extra_cmds.rs:227`, `lib.rs:718` | **UI Regression:** Python sends interactive `MultiplayerMainView`; Rust only lists up to 10 open rooms as text. |
| 17 | `/resetpassword_admin` `<email>` | `bot.py:5151` | **MISSING** | — | **CRITICAL GAP:** Completely absent in Rust. |
| 18 | `/revoke_2fa` `<target>` | `bot.py:5181` | `/revoke_2fa` `<identifier>` | `extra_cmds.rs:340`, `lib.rs:722` | **Broken/Regressed:** Super-admin-only, case-sensitive SQL, does not clear `backup_codes`, no existence/status check, public reply. |

### 3.2 Music Commands (`gdrive-bot/music_system.py` & `gdrive-bot-source/music_system.py`)

| # | Python Slash Command | Python Location | Rust Command (`zenne-discord`) | Rust Location | Parity Status & Behavioral Diffs |
|---|---|---|---|---|---|
| 19 | `/play` `<query>` | `music_system.py:392` (`714`) | `/play` `<query>` | `music.rs:281`, `lib.rs:700` | **Parity:** SoundCloud-only check + MultiPlay worker routing in Rust. |
| 20 | `/pause` | `music_system.py:414` (`735`) | `/pause` | `music.rs:560`, `lib.rs:701` | **Parity** |
| 21 | `/resume` | `music_system.py:424` (`746`) | `/resume` | `music.rs:579`, `lib.rs:702` | **Parity** |
| 22 | `/skip` | `music_system.py:434` (`757`) | `/skip` | `music.rs:598`, `lib.rs:703` | **Parity** |
| 23 | `/stop` | `music_system.py:443` (`767`) | `/mstop` | `music.rs:617`, `lib.rs:704` | **Renamed:** Python slash `/stop` → Rust `/mstop`. |
| 24 | `/queue` | `music_system.py:452` (`779`) | `/queue` | `music.rs:665`, `lib.rs:705` | **Parity** |
| 25 | `/nowplaying` | `music_system.py:479` (`806`) | `/nowplaying` | `extra_cmds.rs:365`, `lib.rs:723` | **Degraded:** Rust omits progress bar, artwork thumbnail, and `MusicControlView` buttons. |
| 26 | `/volume` `<level>` | `music_system.py:502` (`830`) | `/volume` `<level>` | `music.rs:709`, `lib.rs:706` | **Range Diff:** `gdrive-bot` clamps `0..100`; `gdrive-bot-source` and Rust allow `0..150`. |
| 27 | `/loop` `[mode]` | `music_system.py:512` (`844`) | `/rloop` | `music.rs:733`, `lib.rs:707` | **Renamed + Schema Diff:** `gdrive-bot` takes `mode`; `gdrive-bot-source` and Rust cycle `0→1→2→0`. |

### 3.3 Minigame & Utility Commands (`gdrive-bot/*_system.py`)

| # | Python Slash Command | Python Location | Rust Command (`zenne-discord`) | Rust Location | Parity Status & Behavioral Diffs |
|---|---|---|---|---|---|
| 28 | `/afk` `[action]` | `afk_system.py:113` | `/afk` `[reason]` | `lib.rs:312`, `lib.rs:690` | **Schema & Behavior Mismatch** |
| 29 | `/anagram` | `anagram_game_system.py:97` | `/anagram` `[guess]` | `lib.rs:452`, `lib.rs:697` | **Interaction Model Diff** |
| 30 | `/battleship` `<opponent>` | `battleship_system.py:91` | `/battleship` `[coord]` | `lib.rs:475`, `lib.rs:698` | **Major Gameplay Diff** |
| 31 | `/baucua` | `baucua_system.py:169` | `/baucua` | `lib.rs:347`, `lib.rs:692` | **Simplified** |
| 32 | `/cadao` | `cadao_game_system.py:187` | `/cadao` `[guess]` | `extra_cmds.rs:391`, `lib.rs:724` | **Interaction Model Diff** |
| 33 | `/catword` | `catword_system.py:155` | `/catword` `[guess]` | `extra_cmds.rs:655`, `lib.rs:734` | **Interaction Model Diff** |
| 34 | `/chinhta` | `chinhta_game_system.py:193` | `/chinhta` `[guess]` | `extra_cmds.rs:415`, `lib.rs:725` | **Interaction Model Diff** |
| 35 | `/connect4` `<opponent>` | `connect4_system.py:178` | `/connect4` `[col]` | `extra_cmds.rs:773`, `lib.rs:739` | **Major Gameplay Diff** |
| 36 | `/duoihinh` | `emoji_game_system.py:175` | `/emoji` `[guess]` | `extra_cmds.rs:439`, `lib.rs:726` | **Renamed + Interaction Diff** |
| 37 | `/flagquiz` | `flag_quiz_system.py:234` | `/flag` `[guess]` | `extra_cmds.rs:463`, `lib.rs:727` | **Renamed + Interaction Diff** |
| 38 | `/higherlower` | `higher_lower_system.py:152` | `/higherlower` `[choice]` | `extra_cmds.rs:584`, `lib.rs:732` | **Gameplay Diff** |
| 39 | `/history` | `history_guess_system.py:168` | `/history` `[guess]` | `extra_cmds.rs:487`, `lib.rs:728` | **Interaction Model Diff** |
| 40 | `/mastermind` | `mastermind_system.py:90` | `/mastermind` `[guess]` | `extra_cmds.rs:532`, `lib.rs:730` | **Interaction Model Diff** |
| 41 | `/memory` | `memory_game_system.py:139` | `/memory` `[a] [b]` | `lib.rs:506`, `lib.rs:699` | **UI Diff** |
| 42 | `/quizz` `[mon] [cap]` | `quiz_system.py:206` | `/quiz` `[answer]` + `/quizmath` | `lib.rs:386`, `lib.rs:409` | **Renamed + Missing Parameters** |
| 43 | `/sudoku` | `sudoku_system.py:101` | `/sudoku` | `extra_cmds.rs:703`, `lib.rs:736` | **Stubbed in Rust** |
| 44 | `/taixiu` | `taixiu_system.py:124` | `/taixiu` | `lib.rs:329`, `lib.rs:691` | **Simplified** |
| 45 | `/tamsao` | `tamsao_system.py:137` | `/tamsao` `[typed]` | `extra_cmds.rs:679`, `lib.rs:735` | **Gameplay Diff** |
| 46 | `/trieuphu` | `trieuphu_system.py:376` | `/trieuphu` `[ans]` | `extra_cmds.rs:612`, `lib.rs:733` | **UI Diff** |
| 47 | `/typing` | `typing_test_system.py:170` | `/typing` `[typed]` | `extra_cmds.rs:560`, `lib.rs:731` | **UI Diff** |
| 48 | `/uno` | `uno_system.py:269` | `/uno` | `extra_cmds.rs:812`, `lib.rs:740` | **Stubbed in Both** |
| 49 | `/vuatiengviet` | `vuatiengviet_system.py:173` | `/vtv` `[guess]` | `lib.rs:363`, `lib.rs:693` | **Renamed + Interaction Diff** |
| 50 | `/whoami` | `whoami_system.py:158` | `/whoami` `[guess]` | `extra_cmds.rs:508`, `lib.rs:729` | **Interaction Model Diff** |
| 51 | `/wikiquiz` | `wiki_quiz_system.py:113` | **MISSING** | — | **MISSING IN RUST** |
| 52 | `/wordle` `[ngon_ngu]` | `wordle_system.py:248` | `/wordle` `[guess]` | `lib.rs:420`, `lib.rs:696` | **Missing Parameter + UI Diff** |

### 3.4 Rust-Only Slash Commands (Promoted from Python Prefix-Only)
1. `/setnoitu` (`extra_cmds.rs:6`, `lib.rs:708`)
2. `/thamgia` (`extra_cmds.rs:25`, `lib.rs:709`)
3. `/batdaulai` (`extra_cmds.rs:48`, `lib.rs:710`)
4. `/ngonngu_hientai` (`extra_cmds.rs:129`, `lib.rs:713`)
5. `/cachestats` (`extra_cmds.rs:153`, `lib.rs:715`)
6. `/xemgiaidau` (`extra_cmds.rs:288`, `lib.rs:720`)
7. `/ketthucgiaidau` (`extra_cmds.rs:318`, `lib.rs:721`)
8. `/thamgiagiaidau` (`extra_cmds.rs:712`, `lib.rs:737`)
9. `/quizmath` (`lib.rs:409`, `lib.rs:695`)

### 3.5 Prefix-Only Command in Rust
- `gopthemtu` (`lib.rs:260-283`, registered at `lib.rs:688`): `prefix_command` only.

---

## 4. Summary of Command-Name Mismatches

| Python Slash Name | Rust Slash Name | Location |
|---|---|---|
| `/noitu_start` | `/start` | `bot.py:3803` → `lib.rs:70` |
| `/noitu_stop` | `/stop` | `bot.py:3832` → `lib.rs:125` |
| `/noitu_setlang` | `/setlang` | `bot.py:3729` → `lib.rs:165` |
| `/noitu_score` | `/diem` | `bot.py:3853` → `extra_cmds.rs:78` |
| `/noitu_help` | `/help` | `bot.py:4035` → `lib.rs:27` |
| `/top_noitu` | `/top` | `bot.py:3944` → `lib.rs:232` |
| `/kiemduyettu` | `/duyettu` | `bot.py:3638` → `extra_cmds.rs:743` |
| `/stop` (music) | `/mstop` | `music_system.py:443` → `music.rs:617` |
| `/loop` (music) | `/rloop` | `music_system.py:512` → `music.rs:733` |
| `/quizz` | `/quiz` (+ `/quizmath`) | `quiz_system.py:206` → `lib.rs:386,409` |
| `/vuatiengviet` | `/vtv` | `vuatiengviet_system.py:173` → `lib.rs:363` |
| `/duoihinh` | `/emoji` | `emoji_game_system.py:175` → `extra_cmds.rs:439` |
| `/flagquiz` | `/flag` | `flag_quiz_system.py:234` → `extra_cmds.rs:463` |
