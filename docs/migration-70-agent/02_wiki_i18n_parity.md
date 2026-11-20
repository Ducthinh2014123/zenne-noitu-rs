# Forensic Audit Report — ID `02`

## Scope
Forensic comparison of word validation, normalization, chain logic, continuation lookup, and 300+ language / i18n support:
- **Python**: `C:\Users\Ai Hoi\Downloads\wiki_quiz_system\bot.py` (`normalize_word`, `is_valid_for_lang`, `is_latin_scientific`, `has_native_chars`, `_chain_unit`, `valid_chain`, `check_word`, `find_continuations`, `t`, `is_known_ui_lang`), `i18n_data.py`, `languages.json`
- **Rust**: `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-core\src\wiki.rs`, `i18n.rs`, `languages.json`

## Status
**PARTIAL PARITY — SIGNIFICANT GAPS IN RUST**
`languages.json` (320 entries) is identical, and basic `vi` space-syllable vs non-`vi` char chaining matches, but Rust diverges on Unicode normalization, scientific-name/diacritic filtering, Wikipedia-vs-Wiktionary lookup order, live continuation search, and 12 of 14 UI locales.

## Findings

### 1. Unicode, Control Chars, Case & Diacritics
- **Missing control-char stripping & NFC normalization in Rust**:
  - Python `normalize_word` (`bot.py:2125-2134`) strips Unicode `Cc`/`Cf`/`Cs` control/format/surrogate chars and preserves case; `_chain_unit` (`bot.py:2602`) normalizes to `NFC` before splitting/indexing.
  - Rust `normalize_word` (`wiki.rs:7-13`) only collapses whitespace and calls `.to_lowercase()`. Neither `normalize_word` nor `first_syl`/`last_syl`/`valid_chain` (`wiki.rs:15-46`) strips `Cc`/`Cf`/`Cs` or normalizes to `NFC`. NFD input (e.g. macOS/iOS `"hoa\u{0300}"`) breaks `valid_chain` and causes `w.chars().next()` / `next_back()` (`wiki.rs:23, 35-38`) to extract combining marks instead of base letters.
- **Lowercasing breaks case-sensitive checks in Rust**:
  - Rust `normalize_word` (`wiki.rs:12`) lowercases immediately, which would prevent case-based scientific-name detection and forces lowercase titles on case-sensitive Wiktionary lookups (e.g. German nouns on `de.wiktionary.org`).
- **Byte-length vs char-length check**:
  - Python checks `len(word) > 80` in characters (`bot.py:2880`), whereas Rust `is_valid_for_lang` checks `w.len() > 80` in UTF-8 bytes (`wiki.rs:50`), rejecting valid multi-byte Vietnamese/CJK/Arabic words under 80 chars.
- **Python `_NATIVE_CHARS["vi"]` corruption (`bot.py:2144`)**:
  - Lowercase `ị` is corrupted to replacement chars in `_NATIVE_CHARS["vi"]` (uppercase `Ị` at `bot.py:2145` is intact), so lowercase `vi` words whose only diacritic is `ị` (e.g. `bị`, `vị`) fail `has_native_chars` (`bot.py:2164-2169`). Also `_DIACRITIC_LANGS` (`bot.py:2137-2140`) lists 11 scripts, but `_NATIVE_CHARS` only defines `"vi"`, so `has_native_chars` returns `True` for the other 10 (`bot.py:2166-2167`).

### 2. Scientific-Name Rejection & Language Validation
- **Missing in Rust `is_valid_for_lang` (`wiki.rs:48-55`)**:
  - Python `is_valid_for_lang` (`bot.py:2171-2190`) rejects binomial/trinomial Latin scientific names via `is_latin_scientific` (`bot.py:2151-2162`) and rejects diacritic-less ASCII words in `_DIACRITIC_LANGS` via `has_native_chars` (`bot.py:2184-2189`), unless `in_bank=True`.
  - Rust `is_valid_for_lang` (`wiki.rs:48-55`) ignores `_lang` completely and only checks `!w.is_empty() && w.len() <= 80 && w.chars().any(|c| c.is_alphabetic())`. Pure-ASCII non-Vietnamese words and scientific names like `"Homo sapiens"` pass pre-validation.

### 3. Wiktionary vs Wikipedia Order (`check_word`)
- **Reversed lookup order in Rust**:
  - Python `_sync_check` (`bot.py:2370-2381`) checks **Wiktionary first** (`_sync_check_wiktionary` at `bot.py:2352-2368`) for `_WIKTIONARY_PRIMARY` (`{"vi", "en", "fr", "de", "ja", "ko", "zh", "es", "pt", "it"}`, `bot.py:2350`), then falls back to Wikipedia.
  - Rust `check_word` (`wiki.rs:95-134`) queries **Wikipedia REST `/page/summary/` first** (`wiki.rs:96-116`) for all languages, and only falls back to Wiktionary (`wiki.rs:118-134`) if Wikipedia fails. This accepts encyclopedic proper nouns/species before dictionary entries.
- **Negative caching difference**:
  - Rust caches misses (`"0"`) in Redis/in-memory cache for 3600s (`wiki.rs:136`); Python only caches hits in `wiki_cache`.

### 4. Continuations (`find_continuations`)
- **Missing cache & live API fallback in Rust**:
  - Python `find_continuations` (`bot.py:2290-2326`) queries 3 tiers: `word_bank` -> `wiki_cache` -> live MediaWiki `prefixsearch` (`_sync_find_continuations`, `bot.py:2239-2287`, Wiktionary-first for `_WIKTIONARY_PRIMARY`, filtered by `bad_chars`, `is_latin_scientific`, and `has_native_chars`).
  - Rust `find_continuations` (`wiki.rs:187-201`) only queries `db.words_starting_with` (`word_bank`) and filters `used_words`; it has no `wiki_cache` or live Wikipedia/Wiktionary `prefixsearch` fallback.

### 5. `i18n` Locale Coverage & Catalog
- **Catalog parity**: `languages.json` is byte-identical in both repos (320 entries).
- **12 of 14 UI locales missing in Rust `i18n.rs`**:
  - Python `i18n_data.py:6-175` provides all 10 translation keys across **14 locales**: `vi`, `en`, `ja`, `ko`, `zh`, `fr`, `de`, `es`, `pt`, `ru`, `th`, `id`, `ar`, `tr`, with regional subtag fallback (`code -> base -> en -> vi` in `bot.py:251-264`).
  - Rust `i18n.rs:74-115` only implements **`vi` and `en`**. The remaining 12 locales pass `is_known_ui_lang` (`i18n.rs:24-43`) but silently fall back to the English `_` wildcard arm (`i18n.rs:97-106`), and regional tags (e.g. `zh-TW`) are not stripped to base language.

## Recommended Tests
1. **Unicode NFC/NFD & Control Chars**: Test `normalize_word` and `valid_chain` with NFD `"ho\u{0061}\u{0300} bi\u{0300}nh"` -> `"b\u{00ec}nh minh"` (`vi`), NFD non-`vi` words, and zero-width chars (`\u{200B}`, `\u{FEFF}`).
2. **Scientific Names & Diacritics**: Test `is_valid_for_lang("Homo sapiens", "vi")`, `is_valid_for_lang("E. coli", "en")`, `is_valid_for_lang("hello", "vi")`, and lowercase `"bị"` / `"vị"`.
3. **Byte vs Char Length**: Test 40-char Vietnamese/CJK strings (>80 UTF-8 bytes) against `is_valid_for_lang`.
4. **Wiktionary-First & Case**: Mock HTTP for `de` / `en` / `vi` to verify Wiktionary is queried before Wikipedia and German capitalized nouns (`"Haus"`) are not broken by premature lowercasing.
5. **i18n Coverage**: Assert all 14 locales (`vi`, `en`, `ja`, `ko`, `zh`, `fr`, `de`, `es`, `pt`, `ru`, `th`, `id`, `ar`, `tr`) and all 10 keys resolve to non-English/non-key strings in `i18n::t`, plus subtag fallback (`pt-BR` -> `pt`).
