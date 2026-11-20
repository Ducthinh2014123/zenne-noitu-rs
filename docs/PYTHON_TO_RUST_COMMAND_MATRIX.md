# Python → Rust Command Matrix (Discord, Telegram, Twitch, Revolt)

## 1. Discord Commands (`bot.py` / `*_system.py` vs `zenne-discord`)

| Command | Aliases | Python Handler | Rust Handler | Arguments | Status |
|---|---|---|---|---|---|
| `help` / `help_noitu` | `helpnoitu`, `huongdan` | `bot.py:3594,4035` | `lib.rs:27` | — | Implemented |
| `setnoitu` | — | `bot.py:2783` | `extra_cmds.rs:7` | — | Partial (channel not enforced in `noitu.rs`) |
| `setlang` / `setngonngu` | `lang`, `ngonngu` | `bot.py:2797,3729` | `lib.rs:107` | `code` | Implemented |
| `ngonngu_hientai` | `currentlang`, `xemlang` | `bot.py:2844` | `extra_cmds.rs:119` | — | Implemented |
| `setuilang` | `uilang` | `bot.py:2854,3756` | `lib.rs:121` | `code` | Implemented |
| `listlang` | `langlist` | `bot.py:2874,3783` | `extra_cmds.rs:133` | `[query]` | Partial (missing search query) |
| `start` / `batdautro` | `bdt`, `startgame` | `bot.py:2898,2951,3803` | `lib.rs:56` | `[mode]` | Partial (missing turn timer) |
| `thamgia` | `join`, `tg` | `bot.py:2937` | `extra_cmds.rs:19` | — | Implemented |
| `stop` | `end` | `bot.py:2986,3832` | `lib.rs:97` | — | Implemented |
| `gopthemtu` | `addword`, `gtt` | `bot.py:3009` | `lib.rs:197` | `word` | Partial (prefix-only, no slash) |
| `batdaulai` | `newword` | `bot.py:3047` | `extra_cmds.rs:39` | — | Implemented |
| `diem` | `score`, `scores` | `bot.py:3071,3853` | `lib.rs` -> `extra_cmds.rs:62` | — | Implemented |
| `danhsachtu` / `tutruong` | `wordbank`, `bank` | `bot.py:3087,4886` | `extra_cmds.rs:153` | — | Buggy (`pub_wordbank("", 1, 20)` args swapped) |
| `tratu` | `check`, `kt`, `tra` | `bot.py:3108,3985` | `lib.rs:141` | `word` | Implemented |
| `contukhong` | `ctk` | `bot.py:3190,3285` | `extra_cmds.rs:84` | `syllable` | Implemented |
| `top` | `lb`, `bxh`, `xephang` | `bot.py:3327,3944` | `lib.rs:165` | — | Implemented |
| `cachestats` | `cache`, `cs` | `bot.py:3547` | `extra_cmds.rs:143` | — | Implemented |
| `tochucgiaidau` | `tournament`, `giaidau`, `td` | `bot.py:3411,3872` | `extra_cmds.rs:237` | `name`, `[prize]` | Partial (missing `thamgiagiaidau` & `tournament_players`) |
| `thamgiagiaidau` | `joingd` | `bot.py:3385` (Button) | — | — | Missing |
| `xemgiaidau` | `gd` | `bot.py:3520` | `extra_cmds.rs:260` | — | Implemented |
| `ketthucgiaidau` | `endtournament`, `kg` | `bot.py:3479` | `extra_cmds.rs:288` | — | Partial (missing permission check & winner announcement) |
| `kiemduyettu` / `duyettu` | — | `bot.py:3638` | — | `id`, `approve` | Missing |
| `taoroom` | `taophong`, `webgame` | `bot.py:4921` | `extra_cmds.rs:168` | `[name]` | Implemented |
| `multiplayer` | `mp`, `publicrooms` | `bot.py:5101` | `extra_cmds.rs:206` | — | Implemented |
| `revoke_2fa` | — | `bot.py:5181,5218` | `extra_cmds.rs:306` | `identifier` | Implemented |
| `ask` | `hoi`, `ai`, `hoiai` | `bot.py:4121` | `lib.rs:223` | `question` | Implemented |
| `afk` | — | `afk_system.py` | `lib.rs:245` | `[reason]` | Implemented |
| `play`..`rloop`, `nowplaying` | `p`, `q`, `fs`, `vol`, `np` | `music_system.py` | `music.rs`, `extra_cmds.rs:328` | `query` / `level` | Implemented (MultiPlay 10 workers) |
| `taixiu`, `baucua`, `vtv`, `quiz`, `quizmath`, `wordle`, `anagram`, `battleship`, `memory` | various | `*_system.py` | `lib.rs:267-470` | `[guess]` | Implemented |
| `cadao`, `chinhta`, `emoji`, `flag`, `history`, `whoami`, `higherlower`, `trieuphu`, `catword`, `tamsao`, `sudoku` | various | `*_system.py` | `extra_cmds.rs:350-640` | `[guess]` | Implemented (except `sudoku` solve) |
| `mastermind`, `typing` | `mm`, `gochu` | `*_system.py` | `extra_cmds.rs:483,509` | `[input]` | Buggy (tuple destructuring swapped) |
| `connect4`, `uno` | `c4`, `bai` | `connect4_system.py`, `uno_system.py` | — | `[col]` / `[card]` | Missing |

## 2. Telegram Commands (`telegram_bot.py` vs `zenne-telegram`)

All 24 `BotCommand`s + aliases (`start` with `join_<token>`, `help`, `multiplayer`/`mp`, `taophong`, `noitu`/`start_noitu`, `solo`/`botplay`, `batdau`/`play`/`bdt`, `stop`/`end`/`dung`, `diem`/`score`/`scores`, `top`/`bxh`/`leaderboard`, `tratu`/`check`/`tra`, `contukhong`/`ctk`, `gopthemtu`/`addword`/`gtt`, `setlang`/`lang`, `quiz`/`quizz`/`toan`/`khoahoc`, `vtv`/`vuatiengviet`, `taixiu`/`tx`, `baucua`/`bc`, `cadao`/`tucngu`, `chinhta`/`soiloichinhta`, `flag`/`flagquiz`/`quocky`, `trieuphu`/`altp`, `wordle`/`doantu`, `ask`/`ai`/`kira`, `mygame`/`profile`) and all inline callback groups (`btn_quick_*`, `nt_*`, `mp_*`, `qz_*`, `vtv_*`, `tx_bet`, `bc_bet`, `cd_ans`, `ct_ans`, `fq_ans`, `tp_*`) are implemented in `crates/zenne-telegram/src/{lib,noitu,minigames,state}.rs`.

## 3. Twitch & Revolt Commands (`twitch_bot.py` / `revolt_bot.py` vs `zenne-twitch` / `zenne-revolt`)

| Command | Legacy Python | Rust Status | Missing Action |
|---|---|---|---|
| `help`, `noitu`, `stop`, `diem`, `tratu`, `contukhong`, `taixiu`, `baucua`, `vtv`, `quiz`, `cadao`, `chinhta`, `flag`, `ask` | Implemented | Implemented | Add turn timers, DB persistence (`lb_add_word`, `game_submit_score`), wrong-answer feedback |
| `taophong` (`create`), `phong` (`rooms`), `top` (`bxh`, `leaderboard`) | `twitch_bot.py:903,931,963`, `revolt_bot.py:898,931,960` | Missing | Implement `taophong`, `phong`, `top` in both crates |
