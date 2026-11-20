# Forensic Audit Report — ID 25

**Scope:** Password Recovery & Reset parity audit (`/pub/auth/recovery/*`, forgot password, email OTP, admin recovery token, 2FA verification on reset, atomic `used_at IS NULL`, attempt lockout) across:
- **Python:** `C:\Users\Ai Hoi\Downloads\gdrive-bot-source\gdrive-bot\bot.py`
- **Rust:** `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-api\src\auth_routes.rs`, `zenne-api\src\lib.rs`, `zenne-core\src\db.rs`, `zenne-core\src\auth.rs`, `zenne-discord\src\extra_cmds.rs`
- **Web (Next.js):** `C:\Users\Ai Hoi\Downloads\zenne-noitu-deploy-vercel\pages\api\auth\recovery\*`, `lib\recoveryApi.js`, `pages\auth\reset-password.jsx`

---

## 1. Parity Matrix Summary

| Feature / Endpoint | Python (`bot.py`) | Rust (`zenne-noitu-rs`) | Web (`zenne-noitu-deploy-vercel`) | Status |
|---|---|---|---|---|
| `password_recovery` schema & indices | `bot.py:941-951, 991-993` | `db.rs:184-194, 229-231` | N/A | **MATCH** |
| `POST /pub/auth/recovery/request` | `_ah_recovery_request` (`bot.py:5982-6022`) | `handle_recovery_request` (`auth_routes.rs:737-798`) | `pages/api/auth/recovery/request.js:5-15` | **PARTIAL / DRIFT** |
| `POST /pub/auth/recovery/email/verify` | `_ah_recovery_email_verify` (`bot.py:6024-6072`) | `handle_recovery_email_verify` (`auth_routes.rs:807-940`) | `pages/api/auth/recovery/email/verify.js:5-15` | **PARTIAL / DRIFT** |
| `POST /pub/auth/recovery/token/verify` | `_ah_recovery_token_verify` (`bot.py:6074-6115`) | `handle_recovery_token_verify` (`auth_routes.rs:947-1038`) | `pages/api/auth/recovery/token/verify.js:6-19` | **PARTIAL / DRIFT** |
| `POST /pub/auth/recovery/status` | `_ah_recovery_status` (`bot.py:6117-6137`) | `handle_recovery_status` (`auth_routes.rs:1045-1085`) | `pages/api/auth/recovery/status.js:4-12` | **MATCH** |
| `POST /pub/auth/recovery/reset-password` | `_ah_recovery_reset_password` (`bot.py:6139-6204`) | `handle_recovery_reset_password` (`auth_routes.rs:1097-1220`) | `pages/api/auth/recovery/reset-password.js:5-20` | **PARTIAL / DRIFT** |
| Admin Recovery Token (`/resetpassword_admin`) | `slash_resetpassword_admin` (`bot.py:5151-5178`) | **MISSING** in `zenne-discord` | Uses `token/verify` | **MISSING IN RUST** |

---

## 2. Detailed Findings & Parity Gaps

### Critical / High Findings

1. **Admin Recovery Token Command Missing in Rust (`zenne-discord`)**
   - **Python:** `slash_resetpassword_admin` (`bot.py:5151-5178`) provides `/resetpassword_admin <email>` for bot admins to generate a one-time `recovery_token` (`source="admin"`, TTL `480s`) when a user loses email access.
   - **Rust:** Not ported. `zenne-discord/src/extra_cmds.rs:319` ports `revoke_2fa`, but `resetpassword_admin` is absent across all crates in `zenne-noitu-rs`.

2. **Missing Rate Limiting on `token/verify` and `reset-password` in Rust (2FA Brute-Force Risk)**
   - **Python:** Rate-limits by client IP: `recovery_token_verify:{ip}` (`10 / 600s`, `bot.py:6079`) and `recovery_reset_password:{ip}` (`10 / 600s`, `bot.py:6145`).
   - **Rust:** `handle_recovery_token_verify` (`auth_routes.rs:947-1038`) and `handle_recovery_reset_password` (`auth_routes.rs:1097-1220`) have **no rate limiting**. An attacker holding a valid `reset_session` can brute-force 6-digit TOTP codes (`totp_code`) on `/pub/auth/recovery/reset-password` without throttling or attempt lockout.

3. **Email/Account Enumeration in Rust `email/verify`**
   - **Python:** `_ah_recovery_email_verify` (`bot.py:6041, 6045-6047`) returns a generic `400` (`"Ma xac minh khong hop le hoac da het han."`) whether the email does not exist, the code is expired, locked out, or invalid.
   - **Rust:** `handle_recovery_email_verify` (`auth_routes.rs:830-836`) returns `400 {"error": "Tài khoản không tồn tại"}` when `find_user_by_username_or_email` returns `None`, leaking account existence.

4. **Email OTP Entropy & Transport Drift (`recovery/request`)**
   - **Python:** Generates a 128-char CSPRNG token (`_gen_recovery_code()`, `bot.py:5281-5283`) and sends via HTTP transactional API (`EMAIL_PROVIDER_URL` / `EMAIL_API_KEY` in `_send_recovery_email`, `bot.py:5328-5355`).
   - **Rust:** Generates a 6-digit numeric OTP (`gen_6_digit_otp()`, `auth_routes.rs:14-17`) and sends via SMTP (`send_smtp_email`, `auth_routes.rs:89-144`). If SMTP credentials are unset, `send_smtp_email` silently returns `Ok(false)` (`auth_routes.rs:98-100`).

### Medium / Low Findings

5. **Rate-Limit Key & Window Drift (`request` & `email/verify`)**
   - **Python:** Rate-limits by **IP** (`recovery_request:{ip}` 5/900s at `bot.py:5990`; `recovery_email_verify:{ip}` 10/600s at `bot.py:6029`).
   - **Rust:** Rate-limits by **identifier** (`rec_req:{ident}` 5/300s at `auth_routes.rs:758`; `rec_ev:{ident}` 10/300s at `auth_routes.rs:820`). Per-identifier limits allow an attacker to lock out a victim's email from requesting/verifying recovery codes, while allowing unlimited requests across different targets from one IP.
   - Also in `handle_recovery_request` (`auth_routes.rs:747-753`), empty `ident` returns `400` instead of `200 generic_ok` (masked to `200` by Next.js `pages/api/auth/recovery/request.js:11-13`).

6. **Backup Code Normalization Mismatch on 2FA Reset**
   - **Python:** `_verify_2fa_or_backup` (`bot.py:5521-5550`) normalizes backup codes by stripping `-` and uppercasing (`clean.replace("-", "").upper()`).
   - **Rust:** `verify_totp_or_backup` (`auth_routes.rs:44-87`, specifically L70-71) checks `x == c || x == &auth::hash_token(&c.to_uppercase())` without stripping dashes or uppercasing plaintext `c`, which fails if a user omits `-` or uses lowercase against Python-created plaintext backup codes.

7. **Constant-Time Token Comparison**
   - **Python:** Uses `hmac.compare_digest(rec["token_hash"], code_hash)` (`bot.py:6060`).
   - **Rust:** Uses `stored_hash != code_hash` (`auth_routes.rs:890`).

---

## 3. Verified Parity Areas (Working as Expected)

- **Atomic `used_at IS NULL` Consumption:**
  - Rust properly enforces `UPDATE password_recovery SET used_at = datetime('now','localtime') WHERE id = ?1 AND used_at IS NULL` and checks `consumed != 1` in `handle_recovery_email_verify` (`auth_routes.rs:904-917`), `handle_recovery_token_verify` (`auth_routes.rs:1003-1016`), and `handle_recovery_reset_password` (`auth_routes.rs:1191-1204`).
  - Python enforces `WHERE id=? AND used_at IS NULL` with `cur.rowcount != 1` in `_ah_recovery_token_verify` (`bot.py:6101-6106`) and `_ah_recovery_reset_password` (`bot.py:6191-6196`), and invalidates all remaining `used_at IS NULL` rows for `user_id` after password update (`bot.py:6200-6203`; Rust `auth_routes.rs:1212-1217`).
- **Attempt Lockout on Email OTP:**
  - Both Python (`bot.py:6056-6062`, `RECOVERY_MAX_ATTEMPTS = 5`) and Rust (`auth_routes.rs:881-896`, `attempts >= 5`) increment `attempts` on wrong code and lock out at 5 failed attempts.
- **Expiration Compatibility:**
  - Rust `is_expired` (`auth_routes.rs:30-42`) parses both RFC3339 timestamps (Rust-written) and Unix float timestamps (Python-written), ensuring DB interoperability.
- **Web Proxy & Cookie Isolation:**
  - `zenne-noitu-deploy-vercel` (`lib/recoveryApi.js:30-73`, `pages/api/auth/recovery/*`) isolates `reset_session` in an `HttpOnly; SameSite=Lax` cookie separate from NextAuth's session token, and Rust `RecoveryResetBody` (`auth_routes.rs:1092-1093`) aliases `password` to `new_password` to match `reset-password.js:14`.
