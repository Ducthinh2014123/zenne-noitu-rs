# Forensic Audit Report — ID 68

## Workspace Context
- `Cargo.toml` has **no** `proptest`, `quickcheck`, `arbitrary`, or `cargo-fuzz` dependencies configured.

## Key Property & Fuzz Test Targets
1. **`zenne-core/src/wiki.rs`:** `normalize_word` idempotence & whitespace/control-char stripping, `first_syl`/`last_syl`/`valid_chain` NFC invariance (`valid_chain(nfc(a), nfd(b), "vi")`), `is_valid_for_lang` char-count vs byte-count, `urlencoding_simple` ASCII safety.
2. **`zenne-core/src/auth.rs`:** `sign_web_token`/`verify_web_token` round-trip & bit-flip tamper rejection, `sign_game_session`/`verify_game_session_claims` round-trip & `:` delimiter fuzzing, `verify_totp_code` arbitrary base32 fuzzing.
3. **`zenne-core/src/cache.rs`:** Stateful model-based test for `get`/`set`/`delete`/50k eviction and `rate_limited` exact count invariants.
4. **`zenne-games/src/*.rs`:**
   - `wordle.rs`: 5-emoji output, exact `🟩` invariant, letter multiplicity conservation (`#🟩 + #🟨 <= count(c, target)`).
   - `mastermind.rs`: `0 <= exact + partial <= 4`, per-digit multiplicity bound.
   - `connect4.rs`: gravity invariant, turn alternation, 4-in-a-row & full-top-row draw.
   - `battleship.rs`: disjoint `hits`/`misses`, `hits + misses + shots_left == 12`.
   - `memory.rs`, `taixiu.rs`, `baucua.rs`, `anagram.rs`, `vuatiengviet.rs`, `catword.rs`, `quiz.rs`, `afk.rs` (`format_duration` overflow on `i64::MIN`).
5. **`zenne-api/src/{pub_games.rs, ws_game.rs, auth_routes.rs}`:** `ttt` minimax non-loss invariant, `ms_flood` non-mine guarantee & idempotence, `sanity_check_score` boundary properties, `WsRoom` state machine invariants, `is_expired` fuzzing.
6. **`zenne-telegram/src/state.rs`:** `html_escape` safety (no raw `<`, `>`, `"`).
