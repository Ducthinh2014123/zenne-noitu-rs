# Forensic Audit Report — ID 37

**Scope**: Unicode, multilingual handling, and text normalization audit comparing Python (`C:\Users\Ai Hoi\Downloads\gdrive-bot-telegram\{bot.py,i18n_data.py,languages.json,telegram_bot.py}`) vs. Rust (`C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\{zenne-core,zenne-telegram,zenne-discord,zenne-games}`).

---

## 1. NFC / NFD Normalization
- **Python (`bot.py`)**:
  - `normalize_word(raw)` (`bot.py:1827-1836`) does **not** call `unicodedata.normalize("NFC", ...)`.
  - `_chain_unit(w, lang, want_last)` (`bot.py:2342-2360`) calls `_udata.normalize("NFC", w.strip())` (`bot.py:2354`) before splitting syllables/chars.
  - **Defect**: `has_native_chars(word, lang)` (`bot.py:1866-1871`) checks `ch in _NATIVE_CHARS["vi"]` (`bot.py:1845-1848`), which contains only precomposed NFC codepoints. Because `normalize_word` skips NFC, NFD-decomposed Vietnamese input (e.g. from macOS/iOS or decomposed Wikipedia titles) fails `is_valid_for_lang` (`bot.py:1886-1891`).
- **Rust (`zenne-core`, `zenne-games`)**:
  - `zenne-core/Cargo.toml:1-30` has **no** `unicode-normalization` dependency.
  - `wiki::normalize_word` (`zenne-core/src/wiki.rs:7-13`), `first_syl` (`wiki.rs:15-25`), `last_syl` (`wiki.rs:27-40`), and all `zenne-games` guess comparisons perform **zero** NFC/NFD normalization. NFD vs. NFC strings fail equality in `valid_chain` (`wiki.rs:42-46`), `used_words` sets, and mini-game answer checks.

## 2. Vietnamese Diacritic Stripping / Matching & Language Validation
- **No diacritic-stripping fuzzy matcher** exists in either codebase (`bot.py`, `*_system.py`, `zenne-core`, `zenne-games`). Exact diacritic matching is used everywhere.
- **Python (`bot.py`)**:
  - Enforces Vietnamese diacritics in word-chain via `_DIACRITIC_LANGS` (`bot.py:1839-1842`), `_NATIVE_CHARS["vi"]` (`bot.py:1845-1848`), `has_native_chars` (`bot.py:1866-1871`), and `is_valid_for_lang` (`bot.py:1873-1892`, also rejecting ASCII binomial names via `is_latin_scientific` at `bot.py:1857-1864`).
- **Rust (`zenne-core/src/wiki.rs`)**:
  - `is_valid_for_lang` (`zenne-core/src/wiki.rs:48-55`) **dropped** `_DIACRITIC_LANGS`, `_NATIVE_CHARS`, and `is_latin_scientific`—it only checks `!w.is_empty()`, `w.len() <= 80` (**byte length**, not char count, so valid 27+ char Vietnamese words are rejected), and `w.chars().any(|c| c.is_alphabetic())`.
- **Rust (`zenne-games`)**:
  - All Vietnamese mini-games (`anagram.rs:63-64`, `cadao.rs:56`, `catword.rs:56`, `chinhta.rs:51`, `tamsao.rs:41`, `vuatiengviet.rs:55`, `whoami.rs:52-53`) compare `.trim().to_lowercase()` directly without NFC normalization or diacritic tolerance. Note `whoami.rs:53` also uses byte length `g.len() >= 3`.

## 3. `Cc` / `Cf` / `Cs` Control Characters & Whitespace
- **Python (`bot.py`)**:
  - `normalize_word` (`bot.py:1827-1836`) explicitly strips Unicode categories `("Cc", "Cf", "Cs")` via `_udata.category(ch)` (`bot.py:1830-1833`), removing zero-width joiners/spaces (`Cf`), surrogates (`Cs`), and C0/C1 controls including `\n`/`\r` (`Cc`).
  - Whitespace collapse uses `_re.sub(r"[ \t]+", " ", cleaned).strip()` (`bot.py:1835`), which leaves Unicode `Zs` spaces like `\u00a0` (NBSP) uncollapsed. Because `\n` (`Cc`) is already stripped, the later check `"\n" in word` (`bot.py:2641`, `telegram_bot.py:760`) is dead code and `"a\nb"` silently merges into `"ab"`.
- **Rust (`zenne-core/src/wiki.rs`)**:
  - `wiki::normalize_word` (`zenne-core/src/wiki.rs:7-13`) uses `raw.split_whitespace().collect::<Vec<_>>().join(" ").trim().to_lowercase()`. This collapses all Unicode whitespace (including `\u00a0` and `\n`), **but does NOT strip `Cc` (e.g. `\x00`–`\x1f` non-whitespace controls) or `Cf` (e.g. `\u200b` ZWSP, `\u200c`, `\u200d`, `\ufeff` BOM)**. Zero-width characters bypass `valid_chain` and `used_words` deduplication.

## 4. Case Folding & Multilingual i18n
- **Python (`bot.py`, `i18n_data.py`, `languages.json`)**:
  - `normalize_word` (`bot.py:1827-1836`) docstring claims `lowercase`, but **does not lowercase**; callers must `.lower()` separately (`_chain_unit` at `bot.py:2356-2360`, `used_words` at `telegram_bot.py:782, 829`). Neither Python nor Rust uses `.casefold()`.
  - `i18n_data.py:6-175` provides `TRANSLATIONS` for **14 languages** (`vi, en, ja, ko, zh, fr, de, es, pt, ru, th, id, ar, tr`).
  - `bot.py:220-267`: loads 320-entry `languages.json`; `t(key, ui_lang, **kwargs)` (`bot.py:254-267`) strips regional subtags (`base = code.split("-")[0]`) and falls back `(code, base, "en", "vi")`.
- **Rust (`zenne-core/src/i18n.rs`, `wiki.rs`, `db.rs`)**:
  - `i18n.rs:24-43` (`is_known_ui_lang`) lists the same 14 hardcoded codes alongside `LANG_MAP` (`include_str!("../../../languages.json")`, `i18n.rs:12`), **but `i18n::t` (`i18n.rs:74-115`) only implements `"vi"` and `"en"`**, falling back all other 12 languages to English, and does not strip regional subtags (`en-US`).
  - `wiki::find_continuations` (`wiki.rs:198`) uses `u.eq_ignore_ascii_case(w)` instead of Unicode case-insensitive comparison.
  - `db::words_starting_with` (`zenne-core/src/db.rs:528, 542`) uses `word LIKE ?2` without `LOWER(word)` (unlike `bot.py:1917, 1933`) and without escaping SQL `LIKE` wildcards (`%`, `_`).

## 5. HTML / Markdown Escaping
- **Python (`telegram_bot.py`, `bot.py`)**:
  - `telegram_bot.py` uses `ParseMode.HTML` everywhere and `html.escape(...)` in `_user_mention` (`telegram_bot.py:160`), `cmd_ask` (`1397`), `cmd_tratu` (`1022, 1025`), etc.
  - **HTML-injection / entity-parse bugs**: `telegram_bot.py:773, 784, 820, 822` interpolate raw `word` and `game.last_syllable` into `<code>{word}</code>` **without `html.escape`**. Line `801` sends `lang_reason` from `bot.py:1881-1891` under `ParseMode.HTML` with raw `{word}` and Discord `**...**` markdown.
  - Discord (`bot.py`): no `discord.utils.escape_markdown` on `word` or `display_name` (e.g. `bot.py:2657`).
- **Rust (`zenne-telegram`, `zenne-core`)**:
  - `zenne-telegram/src/state.rs:12-17` defines `html_escape` (`&`, `<`, `>`, `"`). `zenne-telegram/src/noitu.rs` and `minigames.rs` consistently apply `html_escape(...)` to user words, syllables, names, URLs, and AI replies (`noitu.rs:1159, 1492, 1506, 1531, 1592`).
  - `zenne-core/src/i18n.rs:77-106` templates embed Discord Markdown (`**...**`, `` `...` ``), unsafe if rendered directly in Telegram HTML.

## 6. 2000 / 4096-Char Message Splitting
- **Discord (2000-char regular / 4096-char embed)**:
  - Python `bot.py:4101-4102` and `4124-4125` (`sl_ask` / `c_ask`) send an Embed and truncate `reply` at `len(reply) > 4000` (`reply[:3990] + "...\n*(Phản hồi quá dài đã được rút gọn)*"`).
  - Rust `zenne-discord/src/lib.rs:238-239` (`ask`) sends a regular message via `ctx.say` and truncates by char count: `ans.chars().take(1900).collect()`. (No multi-message splitting in either).
- **Telegram (4096-char limit)**:
  - **Missing in both**: Python `telegram_bot.py:1395-1399` (`cmd_ask`) and Rust `zenne-telegram/src/noitu.rs:1158-1175` (`cmd_ask`) send `html.escape(answer)` / `html_escape(&ans)` directly without splitting or truncating at 4096 chars. Long AI responses or HTML-entity expansion will fail with Telegram's `Message is too long` error.
