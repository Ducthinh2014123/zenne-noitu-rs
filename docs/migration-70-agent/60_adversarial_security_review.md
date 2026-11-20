# Forensic Audit Report — ID 60

Independent adversarial review of `zenne-noitu-rs` (`crates/zenne-api/src/{auth_routes.rs,lib.rs,ws_game.rs,pub_games.rs}`, `crates/zenne-core/src/{auth.rs,config.rs,db.rs,cache.rs}`) and `zenne-noitu-deploy-vercel` (`pages/api/auth/[...nextauth].js`, `lib/db.js`).

---

## Executive Summary

| # | Finding | Verdict | Severity |
|---|---------|---------|----------|
| 1 | 2FA Bypass | **CONFIRMED** (3 vectors) | **CRITICAL** |
| 2 | Unauthenticated Token Minting | **CONFIRMED** (`/pub/auth/oauth`, `/pub/auth/verify-2fa`, `/pub/auth/register/email/verify`) | **CRITICAL** |
| 3 | PBKDF2 Legacy Lockout | **CONFIRMED** (bcrypt-only verification) | **MEDIUM / HIGH (Availability)** |
| 4 | Unauthenticated TTT/MS | **CONFIRMED** | **HIGH** |
| 5 | Empty `API_KEY` Bypass | **PARTIALLY REFUTED / NUANCED** | **LOW (in default env) / HIGH (code pattern)** |
| 6 | WS `uid=0` | **CONFIRMED** (with DB write caveat) | **MEDIUM** |
| 7 | Hardcoded Secrets | **CONFIRMED** (Turso RW JWT, `API_KEY`, FB, Turnstile) | **CRITICAL** |
| 8 | Recovery Brute-Force | **CONFIRMED** (unbounded 2FA brute-force on reset + OTP resend reset) | **HIGH** |

---

## Key Verified Abuse Scenarios
1. **2FA Bypass (`auth_routes.rs:211-242`, `705-727`, `lib.rs:852-910`):** Omitting `totp_code` on `/pub/auth/login` still mints a 7-day `token`; `disable-2fa` accepts password alone without TOTP; `/pub/auth/verify-2fa` is unauthenticated and unthrottled.
2. **Unauthenticated Token Minting (`lib.rs:922-978`, `auth_routes.rs:405-413`):** `/pub/auth/oauth` upserts by `email` without auth or provider verification and mints a 7-day `token`; `/pub/auth/register/email/verify` mints a 7-day `token` for any already-verified user without checking `body.code`.
3. **PBKDF2 Legacy Lockout (`auth.rs:102-104`):** `bcrypt::verify` rejects all Python-created `salt$hex` PBKDF2-HMAC-SHA256 passwords.
4. **Unauthenticated TTT/MS (`lib.rs:726-752`, `pub_games.rs:125-335`):** Arbitrary `user_id` accepted in JSON without auth; writes directly to `game_scores`.
5. **Empty `API_KEY` Bypass (`config.rs:69`, `lib.rs:30-44, 460, 585`):** `from_env()` falls back to `"Duc thinh 1234@"`, masking the latent `"" == ""` bug in `check_admin_auth` and fail-open checks in `ah_pub_game_score` / `ah_pub_game_session_start`.
6. **WS `uid=0` (`ws_game.rs:223, 244, 456`):** Bypasses duplicate-user check (`token_uid != 0`) allowing room flooding (though `if p_uid != 0` skips DB leaderboard writes).
7. **Hardcoded Secrets (`lib/db.js:4-5`, `config.rs:69-71`, `[...nextauth].js:18, 147-148`).**
8. **Recovery Brute-Force (`auth_routes.rs:1164-1180`, `758`, `cache.rs:91-93`):** Unbounded 2FA brute-force on `reset-password` and OTP attempt-counter reset via `recovery/request`.
