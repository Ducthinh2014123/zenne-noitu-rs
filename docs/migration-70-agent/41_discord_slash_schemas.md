# Forensic Audit Report — ID 41

## 1. Scope & Inventory Summary
- **Python (`C:\Users\Ai Hoi\Downloads\gdrive-bot-source\gdrive-bot\`)**: **52** `@bot.tree.command` slash commands (`bot.py`: 18; `music_system.py`: 9; `*_system.py`: 25).
- **Rust (`C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-discord\src\`)**: **59** `#[poise::command(slash_command, ...)]` commands registered in `all_commands()` (`lib.rs:616-679`): `lib.rs` (18 slash + 1 prefix-only `gopthemtu` at `lib.rs:197`), `music.rs` (8), `extra_cmds.rs` (33).
- **Choices (`app_commands.choices` vs `poise::ChoiceParameter`)**: Python defines `app_commands.choices` on **5** commands (`afk`, `top_noitu`, `loop`, `quizz`, `wordle`). Rust defines **0** `ChoiceParameter` enums across all 59 slash commands.
- **Descriptions**: All 52 Python commands have `description=...` and 15/17 parameterized commands use `@app_commands.describe(...)`. In Rust, 9 minigame commands in `lib.rs:266-471` have no `///` command description, and only **3** parameters have `#[description = ...]` (`start::mode` `lib.rs:59`, `play::query` `music.rs:284`, `duyettu::approve` `extra_cmds.rs:725`).

---

## 2. Command Name Parity Mismatches

### A. Renamed Slash Commands (Python → Rust)
| Python Command & Location | Rust Command & Location | Notes |
|---|---|---|
| `noitu_start` (`bot.py:3803`, `sl_start`) | `start` (`lib.rs:52`, `start`) | Renamed; Rust adds `mode` param |
| `noitu_stop` (`bot.py:3832`, `sl_stop`) | `stop` (`lib.rs:97`, `stop`) | Collides with Python's music `/stop` |
| `noitu_setlang` (`bot.py:3729`, `sl_setlang`) | `setlang` (`lib.rs:107`, `setlang`) | Renamed |
| `noitu_score` (`bot.py:3853`, `sl_score`) | `diem` (`extra_cmds.rs:66`, `diem`) | Renamed |
| `top_noitu` (`bot.py:3944`, `sl_top`) | `top` (`lib.rs:169`, `top`) | Renamed; drops `mode` param/choices |
| `noitu_help` (`bot.py:4035`, `sl_help`) | `help` (`lib.rs:27`, `help`) | Renamed |
| `kiemduyettu` (`bot.py:3638`, `sl_kiemduyettu`) | `duyettu` (`extra_cmds.rs:721`, `duyettu`) | `kiemduyettu` is prefix alias only |
| `stop` (`music_system.py:443`, `sl_stop_music`) | `mstop` (`music.rs:617`, `mstop`) | Renamed to avoid Nối Từ `/stop` |
| `loop` (`music_system.py:512`, `sl_loop`) | `rloop` (`music.rs:733`, `rloop`) | Renamed; drops `mode` param/choices |
| `quizz` (`quiz_system.py:206`, `slash_quizz`) | `quiz` (`lib.rs:323`) + `quizmath` (`lib.rs:346`) | Split into 2 commands; drops `mon`/`cap` |
| `vuatiengviet` (`vuatiengviet_system.py:173`, `slash_vuatiengviet`) | `vtv` (`lib.rs:300`, `vtv`) | Renamed |
| `duoihinh` (`emoji_game_system.py:175`, `slash_duoihinh`) | `emoji` (`extra_cmds.rs:417`, `emoji`) | Renamed |
| `flagquiz` (`flag_quiz_system.py:234`, `slash_flagquiz`) | `flag` (`extra_cmds.rs:441`, `flag`) | Renamed |

### B. Missing in Rust (Python-only Slash Commands)
- `resetpassword_admin` (`bot.py:5151`, `slash_resetpassword_admin`) — `email: str` (required).
- `wikiquiz` (`wiki_quiz_system.py:113`, `slash_wikiquiz`) — 0 params.

### C. Extra in Rust (No `@bot.tree.command` in Python)
- `setnoitu` (`extra_cmds.rs:6`), `thamgia` (`extra_cmds.rs:20`), `batdaulai` (`extra_cmds.rs:43`), `ngonngu_hientai` (`extra_cmds.rs:117`), `cachestats` (`extra_cmds.rs:141`), `xemgiaidau` (`extra_cmds.rs:271`), `ketthucgiaidau` (`extra_cmds.rs:301`), `thamgiagiaidau` (`extra_cmds.rs:690`), `quizmath` (`lib.rs:346`).

---

## 3. Option Schema, Type, Optionality, Choices & Description Divergences

### A. Core / Admin / Nối Từ Commands
1. **`contukhong`** (`bot.py:3285-3287` `sl_contukhong` vs `extra_cmds.rs:88-89` `contukhong`):
   - **Optional vs Required**: Python `syllable: Optional[str] = None` (optional) vs Rust `syllable: String` (required).
   - **Descriptions**: Python has `@app_commands.describe(syllable=...)`; Rust has no `#[description]`.
2. **`kiemduyettu` vs `duyettu`** (`bot.py:3638-3645` vs `extra_cmds.rs:721-726`):
   - **Schema mismatch**: Python `guild_id: str = None` (optional `String`) vs Rust `id: i64` (required `Integer`) + `approve: Option<bool>` (optional `Boolean`, `#[description = "true = duyệt, false = từ chối"]`).
3. **`noitu_setlang` vs `setlang`** (`bot.py:3729-3731` vs `lib.rs:107-108`):
   - **Param name**: Python `lang_code: str` vs Rust `code: String`. Rust omits param description.
4. **`setuilang`** (`bot.py:3756-3758` `sl_set_ui_lang` vs `lib.rs:122-123` `setuilang`):
   - **Param name & Optionality**: Python `ma: str = None` (optional) vs Rust `code: String` (required). Rust omits param description.
5. **`listlang`** (`bot.py:3783-3785` `sl_list_lang` vs `extra_cmds.rs:131-132` `listlang`):
   - **Missing param**: Python `tukhoa: str = None` (optional `String` with description) vs Rust 0 params.
6. **`noitu_start` vs `start`** (`bot.py:3803-3804` vs `lib.rs:52-60`):
   - **Extra param in Rust**: Python 0 params vs Rust `mode: Option<String>` (`#[description = "Nhập 'bot' nếu muốn chơi với Bot"]`).
7. **`tochucgiaidau`** (`bot.py:3872-3881` `sl_tochucgiaidau` vs `extra_cmds.rs:244-249` `tochucgiaidau`):
   - **Param names**: Python `ten_giai: str`, `phan_thuong: str = "Khong co (chi vinh danh)"` vs Rust `name: String`, `prize: Option<String>`. Rust omits param descriptions.
8. **`top_noitu` vs `top`** (`bot.py:3944-3950` vs `lib.rs:169-170`):
   - **Missing param & choices**: Python `mode: str = "words"` with `@app_commands.choices` (`"words"`, `"streak"`) vs Rust 0 params.
9. **`tratu`** (`bot.py:3985-3987` vs `lib.rs:137-138`):
   - Both `word: str` / `word: String` (required), but Rust omits `@app_commands.describe(word="Tu can kiem tra")`.
10. **`ask`** (`bot.py:4100-4102` `sl_ask` vs `lib.rs:223-224` `ask`):
    - **Param name**: Python `cau_hoi: str` vs Rust `prompt: String`. Rust omits param description.
11. **`danhsachtu`** (`bot.py:4903-4905` `slash_danhsachtu` vs `extra_cmds.rs:153-154` `danhsachtu`):
    - **Missing param**: Python `trang: int = 1` (optional `Integer` with description) vs Rust 0 params.
12. **`taoroom`** (`bot.py:4937-4939` `slash_taoroom` vs `extra_cmds.rs:178-179` `taoroom`):
    - **Schema mismatch**: Python `lang: str = "vi"`, `time_limit: int = 30` (both optional, with descriptions) vs Rust `name: Option<String>` (no description).
13. **`revoke_2fa`** (`bot.py:5181-5183` `slash_revoke_2fa` vs `extra_cmds.rs:318-319` `revoke_2fa`):
    - **Param name**: Python `target: str` vs Rust `identifier: String`. Rust omits param description.

### B. Music Commands (`music_system.py` vs `music.rs`)
14. **`play`** (`music_system.py:392-394` vs `music.rs:281-287`): Parity on `query` (required `String`, both have param descriptions).
15. **`volume`** (`music_system.py:502-504` vs `music.rs:709-710`): Both `level` (required `Integer`), but Python clamps `0–100` (with param description) vs Rust allows `0–150` (no param description).
16. **`loop` vs `rloop`** (`music_system.py:512-519` vs `music.rs:733-734`):
    - **Missing param & choices**: Python `mode: app_commands.Choice[str]` (required, choices: `off`, `current`, `all`) vs Rust 0 params (cycles mode).
17. **`pause`, `resume`, `skip`, `stop`/`mstop`, `queue`, `nowplaying`** (`music_system.py:414-501`, `music.rs:560-706`, `extra_cmds.rs:343`): All 0 params in both.

### C. AFK & Minigames (`*_system.py` vs `lib.rs` & `extra_cmds.rs`)
18. **`afk`** (`afk_system.py:113-121` vs `lib.rs:249-250`):
    - **Schema & choices mismatch**: Python `action: str = "status"` with 4 choices (`start`, `stop`, `status`, `top`) and param description vs Rust `reason: Option<String>` (no choices, no description).
19. **`quizz` vs `quiz`/`quizmath`** (`quiz_system.py:206-222` vs `lib.rs:323-355`):
    - **Schema & choices mismatch**: Python `mon: str = "toan"` (choices `toan`, `khoahoc`), `cap: int = 1` (choices `1`, `2`, `3`) vs Rust `quiz` has `answer: Option<String>` and `quizmath` has 0 params (no descriptions, no choices).
20. **`wordle`** (`wordle_system.py:248-254` vs `lib.rs:357-358`):
    - **Schema & choices mismatch**: Python `ngon_ngu: str = "en"` (choices `en`, `vi`) vs Rust `guess: Option<String>` (no choices, no command/param description).
21. **`battleship`** (`battleship_system.py:91-92` vs `lib.rs:412-413`):
    - **Type & optionality mismatch**: Python `opponent: discord.User` (required `User`) vs Rust `coord: Option<String>` (optional `String`, no command/param description).
22. **`connect4`** (`connect4_system.py:178-179` vs `extra_cmds.rs:751-752`):
    - **Type & optionality mismatch**: Python `opponent: discord.User` (required `User`) vs Rust `col: Option<usize>` (optional `Integer`, no param description).
23. **Interactive UI (0-param) in Python vs Text-Arg (`Option<String>` / `Option<usize>`) in Rust**:
    - `anagram`, `memory`, `vuatiengviet`/`vtv`, `cadao`, `chinhta`, `duoihinh`/`emoji`, `flagquiz`/`flag`, `history`, `whoami`, `mastermind`, `typing`, `higherlower`, `trieuphu`, `catword`, `tamsao`.

### D. Exact Parity Commands (Command Name, 0 Params, Both Have Descriptions)
- `multiplayer` (`bot.py:5121` vs `extra_cmds.rs:215`), `pause`, `resume`, `skip`, `queue`, `nowplaying`, `sudoku` (`sudoku_system.py:101` vs `extra_cmds.rs:681`), `uno` (`uno_system.py:269` vs `extra_cmds.rs:786`).
- `taixiu` (`taixiu_system.py:124` vs `lib.rs:266`) and `baucua` (`baucua_system.py:169` vs `lib.rs:284`) match on name and 0 params, but Rust omits the `///` command description.
