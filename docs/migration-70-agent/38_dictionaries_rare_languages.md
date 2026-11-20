# Forensic Audit Report — ID 38

## Scope & Inspected Files
- **Rust**:
  - `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-core\src\wiki.rs` (lines 1–235)
  - `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-core\src\i18n.rs` (lines 1–115)
  - `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-core\src\db.rs` (lines 328–357, 463–545)
  - `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-core\Cargo.toml` (lines 1–30)
  - `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-discord\src\lib.rs` (lines 106–140)
  - `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-discord\src\noitu.rs` (lines 49–150)
  - `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-api\src\ws_game.rs` (lines 332–420)
- **Python**:
  - `C:\Users\Ai Hoi\Downloads\gdrive-bot-source\gdrive-bot\bot.py` (lines 218–310, 1833–2255, 2346–2375, 2640–2740)
- **Shared Catalog**:
  - `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\languages.json` (byte-for-byte identical to `C:\Users\Ai Hoi\Downloads\gdrive-bot-source\gdrive-bot\languages.json`, 320 entries, 28,170 bytes)

---

## 1. `languages.json` & Rare / Regional Language Handling
- **Catalog structure**: 320 entries (`code`, `name_en`, `name_native`), no duplicate codes, no empty `name_native`.
- **Critical Mismatch — 138 BCP-47 Regional Codes**: `languages.json` includes regionalized codes (`ar-AE`, `vi-VN`, `en-US`, `zh-CN`, `zh-TW`, `de-DE`, `ru-RU`, `ja-JP`, `ko-KR`, `th-TH`, `he-IL`, etc.).
  - **Wikipedia/Wiktionary lookup failure**: Neither Rust (`wiki.rs:96-99, 118-121, 146-149`) nor Python (`bot.py:2116, 2139, 2190, 2211`) strips `-region` before building `https://{lang}.wikipedia.org/...` or `https://{lang}.wiktionary.org/...`. Any regional code selected from `!listlang` fails HTTP lookup.
  - **Syllable-splitting mismatch**: Both Rust (`wiki.rs:20, 32`; `db.rs:520`) and Python (`bot.py:2346, 2363`; `bot.py:1910, 1956`) check exact `"vi"`. Setting `lang = "vi-vn"` silently switches Vietnamese from space-separated syllable chaining to single-character chaining.
  - **Rust `i18n::t` fallback gap**: Python `t()` (`bot.py:259`) strips `-region` (`base = code.split("-")[0]`), whereas Rust `i18n::t` (`zenne-core/src/i18n.rs:74-110`) does exact matching only, so `setuilang vi-vn` passes `is_known_ui_lang` (`i18n.rs:24-26`) but falls back to English UI strings.
- **Unvalidated `setlang` in Rust**: `zenne-discord/src/lib.rs:108-119` writes any arbitrary `code` directly to `db.set_lang` without checking `LANG_MAP` or Wikipedia availability (whereas Python has `_sync_validate` at `bot.py:2144-2148`).
- **Closed/Non-existent Wikis & Fallback Bug**: Codes in `languages.json` without active Wikipedias (e.g., `aa` Afar, `ae` Avestan, or `xx-YY` tags) fail `random_word` (`wiki.rs:140-185`; `bot.py:2202-2230`) and fall back to **English** words (`"apple"`, `"elephant"`, etc.) regardless of the configured language.

---

## 2. `_DIACRITIC_LANGS` & `_NATIVE_CHARS`
- **Python (`bot.py:1845-1854, 1872-1898`)**:
  - `_DIACRITIC_LANGS = {"vi", "ja", "ko", "zh", "ar", "ru", "el", "th", "hi", "uk", "he"}` (`bot.py:1845-1848`).
  - `_NATIVE_CHARS` (`bot.py:1851-1854`) **only defines `"vi"`**.
  - **Bug in Python**: `has_native_chars(word, lang)` (`bot.py:1872-1877`) returns `True` if `lang not in _NATIVE_CHARS` (`bot.py:1874-1875`). Therefore, `_DIACRITIC_LANGS` is a **complete no-op** for `ja`, `ko`, `zh`, `ar`, `ru`, `el`, `th`, `hi`, `uk`, and `he` — pure-ASCII English words pass `is_valid_for_lang` in those 10 languages.
  - **NFD bug in Python `"vi"`**: `normalize_word` (`bot.py:1833-1842`) does not normalize to NFC, so NFD Vietnamese (e.g., `o` + `\u0300`) fails `has_native_chars` (`bot.py:1877`) because `_NATIVE_CHARS["vi"]` only contains precomposed characters.
  - **Diacritic-less VI words**: Valid Vietnamese words without diacritics (e.g., `"xe"`, `"ba"`, `"con"`, `"ca"`) are rejected by `_NATIVE_CHARS["vi"]` unless pre-approved in `word_bank`.
- **Rust (`zenne-core/src/wiki.rs:48-55`)**:
  - Both `_DIACRITIC_LANGS` and `_NATIVE_CHARS` are **completely missing**.
  - `is_valid_for_lang(word: &str, _lang: &str)` (`wiki.rs:48-55`) ignores `_lang` completely and only checks `!w.is_empty() && w.len() <= 80 && w.chars().any(|c| c.is_alphabetic())`.
  - Pure-ASCII words (e.g., `"apple"`) are accepted in `vi`, `ja`, `ko`, `zh`, `ar`, `ru`, `th`, etc., as long as the corresponding Wikipedia has an article for that ASCII title.

---

## 3. `_WIKTIONARY_PRIMARY` & Lookup Order Parity
- **Python (`bot.py:2110, 2131-2142, 2202-2223, 1959-1961`)**:
  - `_WIKTIONARY_PRIMARY = {"vi", "en", "fr", "de", "ja", "ko", "zh", "es", "pt", "it"}`.
  - `_sync_check`, `_sync_random`, and `_sync_find_continuations` query **Wiktionary first** for these 10 languages, falling back to Wikipedia.
- **Rust (`zenne-core/src/wiki.rs:94-135, 146-159, 187-201`)**:
  - `_WIKTIONARY_PRIMARY` is **missing**.
  - `check_word` (`wiki.rs:94-135`) **inverts the order**: it always queries `https://{lang}.wikipedia.org/api/rest_v1/page/summary/{encoded}` first for all languages, and only falls back to `wiktionary.org` if Wikipedia returns 404/error.
  - `random_word` (`wiki.rs:146-159`) **never** queries Wiktionary (Wikipedia `page/random/summary` only).
  - `find_continuations` (`wiki.rs:187-201`) **never** queries live Wiktionary/Wikipedia `prefixsearch` (DB `word_bank` + `wiki_cache` only).

---

## 4. `is_latin_scientific`
- **Python (`bot.py:1859-1870, 1886, 1985, 2166, 2180`)**:
  - `_SCIENTIFIC_RE = r'^[A-Z][a-z]+(?:\.?)(?:\s[a-z][a-z\-]+)+$'` with `word.encode('ascii')` check.
- **Rust (`zenne-core/src/wiki.rs`)**:
  - **Completely missing**.
  - Rust `normalize_word` (`wiki.rs:7-13`) calls `.to_lowercase()`, which would break `^[A-Z]` matching if ported directly after `normalize_word`.

---

## 5. `normalize_word`, `valid_chain`, `first_syl`, `last_syl` & UTF-8 Byte-Length Bug
- **`normalize_word` divergence**:
  - Python (`bot.py:1833-1842`): strips Unicode categories `Cc`, `Cf`, `Cs`, collapses `[ \t]+`, preserves case, no NFC.
  - Rust (`wiki.rs:7-13`): collapses `split_whitespace()`, calls `.to_lowercase()`, **does NOT strip `Cc`/`Cf`/`Cs`**, **no NFC**.
- **Rust 80-byte length bug (`wiki.rs:50`)**:
  - `w.len() > 80` in Rust checks **UTF-8 bytes**, whereas Python `len(word) > 80` (`bot.py:2647`) checks **Unicode code points**. Multi-byte scripts are silently truncated to ~26 chars (3-byte scripts) or 40 chars (2-byte scripts).

---

## 6. Script & Language Validation Edge Cases (Python & Rust)
1. **Vietnamese (`vi`)**: Rust lacks NFC everywhere (`wiki.rs:7-46`).
2. **CJK (`zh`, `ja`, `ko`)**: Prolonged sound mark `ー` (`U+30FC`), `っ`, NFD dakuten/Jamo, no Traditional/Simplified folding.
3. **RTL / Arabic (`ar`, `fa`, `ur`) & Hebrew (`he`)**: Combining marks (`Harakat`/`Niqqud`) extracted as `last_syl`, bidi/ZWJ (`Cf`) retained in Rust, Hebrew Sofit forms don't chain to initial forms.
4. **Thai (`th`)**: Trailing above/below vowels and tone marks returned as `last_syl`.
5. **Russian (`ru`, `uk`)**: Words ending in `ь`, `ъ`, `ы` produce unchainable tails; no `ё` -> `е` folding.
6. **German (`de`)**: Rust `normalize_word` lowercases `"Hund"` -> `"hund"`, causing `de.wiktionary.org` lookup to fail.
