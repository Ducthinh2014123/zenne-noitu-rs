# Forensic Audit Report — ID 53

**Scope Audited:** `zenne-core/src/{auth.rs,config.rs}`, `zenne-api/src/{lib.rs,auth_routes.rs}`, `bot.py`, `zenne-noitu-deploy-vercel/pages/api/auth/steam-callback.js`.

---

## 1. Key Derivation, Secret Management & Token Interoperability
1. **Key Derivation Mismatch (`sha256(API_KEY + "::web-game-token")` vs raw `api_key`):** `bot.py:107-108` derives HMAC keys via `sha256(API_KEY + "::web-game-token")`, whereas `config.rs:69-71` defaults `web_token_secret` and `game_session_secret` directly to raw `api_key`, breaking domain separation and Python-Rust token compatibility.
2. **Hardcoded Weak Default Secrets:** `"Duc thinh 1234@"` in `bot.py:77` & `config.rs:69`; `'fallback'` in `steam-callback.js:76`.
3. **Web Token Payload Schema Mismatch:** `bot.py:112` uses `"un"`, `auth.rs:11-15` uses `"username"`.

---

## 2. Constant-Time Verification
- **Constant-Time:** `verify_web_token` (`auth.rs:29`), `verify_game_session_claims` (`auth.rs:61`), and Python `hmac.compare_digest`.
- **Non-Constant-Time:** `check_admin_auth` (`lib.rs:33,39`), `_chk` (`bot.py:4172`), `_verify_pw` (`bot.py:5266`), `stored_hash != code_hash` (`auth_routes.rs:461,890`), and backup code equality (`auth_routes.rs:71`).

---

## 3. Random Number Generation
- `auth.rs:42` (`sign_game_session` nonce) and `auth_routes.rs:15,20` (`gen_6_digit_otp`, `gen_backup_codes`) use `rand::thread_rng()` instead of `rand::rngs::OsRng`.

---

## 4. `bcrypt` vs `PBKDF2-HMAC-SHA256` Password Migration
- `bot.py:5257-5268` stores `PBKDF2-HMAC-SHA256` (`260,000` iters, `salt$hex`); `auth.rs:98-104` only supports `bcrypt` (cost 10). All existing Python-created users fail login/password-change/disable-2FA in Rust until PBKDF2 verification + re-hash on login is added.

---

## 5. Additional Critical Findings
- `ah_auth_oauth` (`lib.rs:922-978`) lacks `check_admin_auth` or OAuth signature verification, allowing unauthenticated user upsert + 7-day token minting.
- `ah_pub_game_score` (`lib.rs:460`) and `ah_pub_game_session_start` (`lib.rs:585`) fail open when `api_key.is_empty()`.
