# Forensic Audit Report — ID 32

## 1. Discord (`bot.py` + `*_system.py` vs `zenne-discord`)

### 1.1 Commands Completely Missing in Rust (`zenne-discord`)
- **`/resetpassword_admin`** — `bot.py:5151` (`sl_resetpassword_admin`): Admin slash command to generate an Admin Recovery Token for web accounts. Missing in `zenne-discord`.
- **`!help_noitu`** (aliases: `helpnoitu`, `huongdan`) — `bot.py:3594` (`c_help`) & **`/noitu_help`** — `bot.py:4035`: Nối Từ-specific help commands. Missing in `zenne-discord` (only global `help` at `zenne-discord/src/lib.rs:28`).
- **`/noitu_setlang`** (`bot.py:3729`), **`/noitu_start`** (`bot.py:3803`), **`/noitu_stop`** (`bot.py:3832`), **`/noitu_score`** (`bot.py:3853`), **`/top_noitu`** (`bot.py:3944`): Slash variants for Nối Từ. Missing in `zenne-discord` (which exposes `setlang`, `start`, `stop`, `diem`, `top` instead).
- **`/kiemduyettu`** — `bot.py:3638` (`sl_kiemduyettu`): Missing as a primary slash command in `zenne-discord` (exists only as alias `kiemduyettu` on `duyettu` at `zenne-discord/src/extra_cmds.rs:721-722`).
- **`/wikiquiz` & `!wikiquiz`** (aliases: `dinhnghia`, `wiki`) — `wiki_quiz_system.py:113,125`: Completely missing in `zenne-discord`.
- **`!uno_hand`** (aliases: `unohand`, `bai`) — `uno_system.py:233`: Completely missing in `zenne-discord` (`zenne-discord/src/extra_cmds.rs:787` `uno` only prints an opening card).
- **`!toan`** — `quiz_system.py:248` (`cmd_toan`) & **`!khoahoc`** (alias: `khtn`) — `quiz_system.py:262` (`cmd_khoahoc`): Missing in `zenne-discord` (only `quiz` and `quizmath` exist at `zenne-discord/src/lib.rs:324,347`).
- **`/loop` & `!loop`** — `music_system.py:512,667`: Missing under name `loop` in `zenne-discord` (renamed to `rloop` at `zenne-discord/src/music.rs:733-734`).
- **`/stop` (music)** — `music_system.py:443`: In Python, slash `/stop` stops music while `/noitu_stop` stops Nối Từ. In `zenne-discord/src/lib.rs:97-98`, `/stop` stops Nối Từ, and music stop is `/mstop` (`zenne-discord/src/music.rs:617-618`).
- **Renamed primary minigame commands** (Python primary names missing in Rust):
  - `/duoihinh` & `!duoihinh` (`emoji_game_system.py:175,187`) -> Rust uses `emoji` (`zenne-discord/src/extra_cmds.rs:418`)
  - `/flagquiz` & `!flagquiz` (`flag_quiz_system.py:234,246`) -> Rust uses `flag` (`zenne-discord/src/extra_cmds.rs:442`)
  - `/quizz` & `!quizz` (`quiz_system.py:206,233`) -> Rust uses `quiz` (`zenne-discord/src/lib.rs:324`)
  - `/vuatiengviet` & `!vuatiengviet` (`vuatiengviet_system.py:173,185`) -> Rust uses `vtv` (`zenne-discord/src/lib.rs:301`)

### 1.2 Prefix Command Aliases Missing in Rust (`zenne-discord`)
- `!anagram` (`anagram_game_system.py:109`): missing `doichu`, `ghepchu` (`zenne-discord/src/lib.rs:390`)
- `!ask` (`bot.py:4121`): missing `hoi`, `ai`, `hoiai` (`zenne-discord/src/lib.rs:224`)
- `!battleship` (`battleship_system.py:150`): missing `thuyachien`, `naval` (`zenne-discord/src/lib.rs:413`)
- `!baucua` (`baucua_system.py:181`): missing `tomca` (`zenne-discord/src/lib.rs:285`)
- `!cachestats` (`bot.py:3547`): missing `cache`, `cs` (`zenne-discord/src/extra_cmds.rs:142`)
- `!cadao` (`cadao_game_system.py:199`): missing `tucngu`, `thanhngu` (`zenne-discord/src/extra_cmds.rs:370`)
- `!catword` (`catword_system.py:165`): missing `noitu_cd`, `noituchude`, `ntcd` (`zenne-discord/src/extra_cmds.rs:634`)
- `!chinhta` (`chinhta_game_system.py:205`): missing `soiloict`, `ct` (`zenne-discord/src/extra_cmds.rs:394`)
- `!connect4` (`connect4_system.py:198`): missing `noi4` (`zenne-discord/src/extra_cmds.rs:752`)
- `!contukhong` (`bot.py:3190`): missing `kiemtranoi`, `noiduockhong` (`zenne-discord/src/extra_cmds.rs:89`)
- `!danhsachtu` (`bot.py:4886`) & `!tutruong` (`bot.py:3087`): missing `khotudu`, `frombank`, `bank` (`zenne-discord/src/extra_cmds.rs:154`)
- `!diem` (`bot.py:3071`): missing `scores` (`zenne-discord/src/extra_cmds.rs:67`)
- `!duoihinh` (`emoji_game_system.py:187`): missing `duoihinh`, `dhbc`, `emojiword` (`zenne-discord/src/extra_cmds.rs:418`)
- `!flagquiz` (`flag_quiz_system.py:246`): missing `flagquiz`, `co`, `doanco` (`zenne-discord/src/extra_cmds.rs:442`)
- `!gopthemtu` (`bot.py:3009`): missing `addword`, `gtt` (`zenne-discord/src/lib.rs:198`)
- `!higherlower` (`higher_lower_system.py:159`): missing `hl`, `sokeo`, `caothap` (`zenne-discord/src/extra_cmds.rs:563`)
- `!history` (`history_guess_system.py:180`): missing `lichsu`, `namlichsu` (`zenne-discord/src/extra_cmds.rs:466`)
- `!ketthucgiaidau` (`bot.py:3479`): missing `endtournament`, `ketgiai`, `kg` (`zenne-discord/src/extra_cmds.rs:302`)
- `!listlang` (`bot.py:2874`): missing `langlist`, `tramalang`, `danhsachlang` (`zenne-discord/src/extra_cmds.rs:132`)
- `!mastermind` (`mastermind_system.py:102`): missing `matma`, `bekhoa`, `mm` (`zenne-discord/src/extra_cmds.rs:511`)
- `!memory` (`memory_game_system.py:154`): missing `latthe`, `trinho` (`zenne-discord/src/lib.rs:444`)
- `!mstop` (`music_system.py:595`): missing `disconnect` (`zenne-discord/src/music.rs:618`)
- `!multiplayer` (`bot.py:5101`): missing `publicrooms`, `choicongkhai` (`zenne-discord/src/extra_cmds.rs:216`)
- `!ngonngu_hientai` (`bot.py:2844`): missing `currentlang`, `xemlang` (`zenne-discord/src/extra_cmds.rs:118`)
- `!quizz` (`quiz_system.py:233`): missing `quizz`, `giaido`, `dautri` (`zenne-discord/src/lib.rs:324`)
- `!setngonngu` (`bot.py:2797`): missing `lang`, `ngonngu`, `language` (`zenne-discord/src/lib.rs:108`)
- `!setuilang` (`bot.py:2854`): missing `uilang`, `doingondngiaodien`, `ngugiaodien` (`zenne-discord/src/lib.rs:123`)
- `!skip` (`music_system.py:585`): missing `fs` (`zenne-discord/src/music.rs:599`)
- `!stop` (`bot.py:2986`): missing `end` (`zenne-discord/src/lib.rs:98`)
- `!sudoku` (`sudoku_system.py:111`): missing `sdk` (`zenne-discord/src/extra_cmds.rs:682`)
- `!taixiu` (`taixiu_system.py:134`): missing `xucxac` (`zenne-discord/src/lib.rs:267`)
- `!tamsao` (`tamsao_system.py:147`): missing `tamcao`, `dich`, `chaintranslate` (`zenne-discord/src/extra_cmds.rs:658`)
- `!taoroom` (`bot.py:4921`): missing `taophong`, `webgame`, `choinoitu` (`zenne-discord/src/extra_cmds.rs:179`)
- `!thamgia` (`bot.py:2937`): missing `tg` (`zenne-discord/src/extra_cmds.rs:21`)
- `!tochucgiaidau` (`bot.py:3411`): missing `tournament`, `giaidau`, `td` (`zenne-discord/src/extra_cmds.rs:245`)
- `!top` (`bot.py:3327`): missing `bxh`, `xephang` (`zenne-discord/src/lib.rs:170`)
- `!tratu` (`bot.py:3108`): missing `check`, `kt`, `tra` (`zenne-discord/src/lib.rs:138`)
- `!trieuphu` (`trieuphu_system.py:385`): missing `altp`, `millionaire` (`zenne-discord/src/extra_cmds.rs:591`)
- `!typing` (`typing_test_system.py:191`): missing `gochu`, `wpm` (`zenne-discord/src/extra_cmds.rs:539`)
- `!uno` (`uno_system.py:182`): missing `staruno`, `batdauuno` (`zenne-discord/src/extra_cmds.rs:787`)
- `!vuatiengviet` (`vuatiengviet_system.py:185`): missing `vuatiengviet`, `xapchu` (`zenne-discord/src/lib.rs:301`)
- `!whoami` (`whoami_system.py:170`): missing `danhnhan`, `ailasi` (`zenne-discord/src/extra_cmds.rs:487`)
- `!xemgiaidau` (`bot.py:3520`): missing `tournament_info`, `gd` (`zenne-discord/src/extra_cmds.rs:272`)

### 1.3 Event Listeners, UI Callbacks & Reaction Handlers Missing in Rust (`zenne-discord`)
- **`on_raw_reaction_add` (`bot.py:2614`)**: `🙋` reaction on lobby message to join Nối Từ. Not handled in `zenne-discord/src/lib.rs:479-545`.
- **12 Minigame `@bot.listen("on_message")` passive answer handlers**: Present in `anagram_game_system.py:116`, `battleship_system.py:198`, `cadao_game_system.py:206`, `catword_system.py:173`, `emoji_game_system.py:195`, `history_guess_system.py:187`, `mastermind_system.py:109`, `sudoku_system.py:118`, `tamsao_system.py:154`, `vuatiengviet_system.py:193`, `whoami_system.py:177`, `wiki_quiz_system.py:132`. All missing in `zenne-discord/src/lib.rs:511-543` (`FullEvent::Message` only handles `afk` and `noitu::handle_word_message`). Note: `sudoku` in `zenne-discord/src/extra_cmds.rs:682` has no answer submission mechanism at all.
- **Discord UI Button/Modal `custom_id` interactions**: Missing in `zenne-discord` for `ReviewView` (`bot.py:2478,2511`), `LeaderboardView` (`bot.py:3390`), `MultiplayerView`/`Modal` (`bot.py:4981,5048,5053,5084`), `baucua` (`bc_*`, `baucua_system.py:35`), `chinhta` (`ct_*`, `chinhta_game_system.py:105`), `connect4` (`c4_col_*`, `connect4_system.py:87`), `emoji_game` (`emoji_hint`, `emoji_stop`, `emoji_game_system.py:64,101`), `flag_quiz` (`flg_*`, `flag_quiz_system.py:136`), `memory` (`mem_*`, `memory_game_system.py:58`), `music` (`btn_play_pause`, `btn_skip`, `btn_stop`, `btn_loop`, `music_system.py:175-201`), `quiz` (`quiz_opt_*`, `quiz_next`, `quiz_stop`, `quiz_system.py:26,44,61`), `taixiu` (`tx_tai`, `tx_xiu`, `taixiu_system.py:23,27`), `trieuphu` (`tp_ans_*`, `trieuphu_system.py:113`), `typing` (`typing_start`, `typing_test_system.py:104`), `uno` (`uno_play_*`, `uno_draw`, `uno_system.py:63,68`), `vuatiengviet` (`vtv_hint`, `vtv_stop`, `vuatiengviet_system.py:71,103`), `wordle` (`wordle_guess`, `wordle_giveup`, `wordle_system.py:91,103`).

### 1.4 Undocumented Commands in Discord Help
- In `zenne-discord/src/lib.rs:28-49` (`help()`), **24 registered commands** are omitted from the help embed: `setnoitu`, `thamgia`, `batdaulai`, `contukhong`, `ngonngu_hientai`, `cachestats`, `danhsachtu`, `taoroom`, `multiplayer`, `tochucgiaidau`, `xemgiaidau`, `ketthucgiaidau`, `thamgiagiaidau`, `duyettu`, `revoke_2fa`, `nowplaying`, `higherlower`, `trieuphu`, `catword`, `tamsao`, `sudoku`, `connect4`, `uno` (plus `rloop` is listed but `nowplaying` is not).
- In Python `bot.py:3594-3631` (`c_help`), `contukhong`, `ngonngu_hientai`, `setuilang`, `listlang`, `tratu`, `top`, `tochucgiaidau`, `ketthucgiaidau`, `xemgiaidau`, `cachestats`, `danhsachtu`, `taoroom`, `multiplayer`, `ask`, `resetpassword_admin`, `revoke_2fa`, and all `*_system.py` minigames/music/afk are undocumented.

---

## 2. Telegram (`telegram_bot.py` vs `zenne-telegram`)

- **Command & Callback Parity**: All 25 `CommandHandler` entries (`telegram_bot.py:1884-1910`) and all `callback_data` handlers (`btn_quick_*`, `nt_*`, `mp_*`, `qz_*`, `vtv_*`, `tx_bet:*`, `bc_bet:*`, `cd_ans:*`, `ct_ans:*`, `fq_ans:*`, `tp_*`) are implemented in `zenne-telegram/src/lib.rs:71-104`, `noitu.rs:1179-1448`, and `minigames.rs:717-1145`.
- **Hidden / Undocumented in `BotCommand` Menu** (`telegram_bot.py:1820-1843` & `zenne-telegram/src/lib.rs:15-45`):
  - `/start` (and deep-link `/start join_<token>`, `telegram_bot.py:171`, `noitu.rs:94-99`) is omitted from `BotCommand`.
  - **28 command aliases** are hidden/undocumented in `BotCommand`: `mp`, `start_noitu`, `botplay`, `play`, `bdt`, `end`, `dung`, `score`, `scores`, `bxh`, `leaderboard`, `check`, `tra`, `ctk`, `addword`, `gtt`, `lang`, `quizz`, `toan`, `khoahoc`, `vuatiengviet`, `tx`, `bc`, `tucngu`, `soiloichinhta`, `flagquiz`, `quocky`, `altp`, `doantu`, `ai`, `kira`, `profile`.

---

## 3. Twitch (`twitch_bot.py` vs `zenne-twitch/src/lib.rs`)

### 3.1 Missing Command Aliases in Rust (`zenne-twitch/src/lib.rs:203-323`)
- `!help` (`twitch_bot.py:360`): missing aliases **`huongdan`**, **`lenh`** (`zenne-twitch/src/lib.rs:204`)
- `!noitu` (`twitch_bot.py:362`): missing alias **`play`** (`zenne-twitch/src/lib.rs:207`)
- `!stop` (`twitch_bot.py:364`): missing alias **`end`** (`zenne-twitch/src/lib.rs:225`)
- `!ask` (`twitch_bot.py:380`): missing aliases **`ai`**, **`kira`** (`zenne-twitch/src/lib.rs:312`)
- `!diem` (`twitch_bot.py:386`): missing alias **`point`** (`zenne-twitch/src/lib.rs:229`)

### 3.2 Undocumented in `!help`
- `zenne-twitch/src/lib.rs:205`: `!top`, `!taophong`, `!phong` are implemented (lines `235, 244, 264`) but omitted from the `!help` response string.

---

## 4. Revolt (`revolt_bot.py` vs `zenne-revolt/src/lib.rs`)

### 4.1 Missing Command Aliases in Rust (`zenne-revolt/src/lib.rs:224-344`)
- `!help` (`revolt_bot.py:327`): missing aliases **`huongdan`**, **`lenh`** (`zenne-revolt/src/lib.rs:225`)
- `!noitu` (`revolt_bot.py:329`): missing alias **`play`** (`zenne-revolt/src/lib.rs:226`)
- `!stop` (`revolt_bot.py:331`): missing alias **`end`** (`zenne-revolt/src/lib.rs:244`)
- `!ask` (`revolt_bot.py:347`): missing aliases **`ai`**, **`kira`** (`zenne-revolt/src/lib.rs:331`)
- `!diem` (`revolt_bot.py:353`): missing alias **`point`** (`zenne-revolt/src/lib.rs:248`)

### 4.2 Undocumented in `!help`
- `zenne-revolt/src/lib.rs:225`: `!top`, `!taophong`, `!phong` are implemented (lines `254, 263, 283`) but omitted from the `!help` response string.

---

## 5. HTTP API & WebSocket Routes (`bot.py` vs `zenne-api`)

- **All 44 Python HTTP/WS routes** (`bot.py:6298-6342`) are present in `zenne-api/src/lib.rs:60-128` (with `OPTIONS /{p:.*}` at `bot.py:6325` replaced by Tower `CorsLayer::permissive()` at `zenne-api/src/lib.rs:146`).
- **Undocumented / Rust-only alias routes** in `zenne-api/src/lib.rs:61,66,129-137`: `/api/health`, `/api/review_bulk`, `/api/pub/status`, `/api/pub/leaderboard`, `/api/pub/games`, `/api/pub/game/score`, `/api/pub/game/leaderboard`, `/api/pub/mp/rooms`, `/api/pub/auth/login`, `/api/pub/ai/ask`, `/ws/game`.
