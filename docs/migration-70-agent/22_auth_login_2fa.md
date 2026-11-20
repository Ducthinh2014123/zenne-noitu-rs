# Forensic Audit Report — ID 22

**Scope:** Auth Login & 2FA Parity Audit (`/pub/auth/login`, `/pub/auth/verify-2fa`, password hashing, TOTP verification, pending 2FA token, brute-force lockout) across Python (`bot.py`), Rust (`zenne-noitu-rs`), and Web (`zenne-noitu-deploy-vercel`).

---

## 1. `/pub/auth/login` Parity

| Aspect | Python (`bot.py`) | Rust (`zenne-noitu-rs`) | Web (`zenne-noitu-deploy-vercel`) |
| :--- | :--- | :--- | :--- |
| **Route & Handler** | `bot.py:6329` -> `_ah_auth_login` (`bot.py:5752-5768`) | `crates/zenne-api/src/lib.rs:102,135` -> `ah_auth_login` (`lib.rs:542-547`) -> `auth_routes::handle_login` (`auth_routes.rs:154-252`) | `pages/api/auth/[...nextauth].js:176-188` (`CredentialsProvider.authorize`) |
| **Identifier Lookup** | Requires `email` + `password`; queries `WHERE email=? AND provider='credentials'` (`bot.py:5763`) | Accepts `username` or `email` (`auth_routes.rs:155-160`) via `find_user_by_username_or_email` (`zenne-core/src/db.rs:1103-1125`); does **not** filter `provider='credentials'` and uses case-sensitive `WHERE username = ?1 OR email = ?1` (`db.rs:1107`) | Sends `{ email, password }` (`[...nextauth].js:179`) |
| **Email Verification** | `403` `{"error": "EmailNotVerified", ...}` (`bot.py:5766-5767`) | `403` `{"error": "EmailNotVerified", "email": ...}` (`auth_routes.rs:203-209`) | Throws `EmailNotVerified` (`[...nextauth].js:184`), triggers OTP verify flow (`pages/auth/login.jsx:262-269`) |
| **2FA Handling on Login** | Does not accept `totp_code`; returns `_user_to_dict(row)` (`bot.py:5561-5573`) with `totp_enabled: bool` and **no `token`** | Accepts optional `totp_code` (`auth_routes.rs:151,211-227`). **CRITICAL:** If `totp_enabled == 1` and `totp_code` is omitted/empty, it **still signs and returns a 7-day `token`** (`auth_routes.rs:229-242`) alongside `"requires_2fa": true`! | Checks `user.totp_enabled`; if no `credentials.totpCode`, throws `Needs2FA` (`[...nextauth].js:190-191`) |
| **Response Shape** | Flat dict: `id, username, email, image, is_admin, isAdmin, provider, totp_enabled` (`bot.py:5564-5573`) | Flat + nested: `id, username, email, is_admin, totp_enabled, requires_2fa, token, user: {...}` (`auth_routes.rs:235-250`); omits `image`, `isAdmin`, `provider` | Expects `user.id`, `user.totp_enabled`, `user.is_admin`, `user.username`/`name` (`[...nextauth].js:187-202`) |

---

## 2. `/pub/auth/verify-2fa` Parity

- **Python** (`bot.py:6330` -> `_ah_auth_verify_2fa`, `bot.py:5770-5801`):
  - Requires `user_id` (int) and `code`; returns `404` if user missing, `400` if `not row["totp_enabled"]` (`bot.py:5789-5790`).
  - Calls `_verify_2fa_or_backup` (`bot.py:5521-5559`), persists remaining `backup_codes` if backup used (`bot.py:5797-5799`), returns `200` `{"ok": True, "backup_used": is_backup}` (`bot.py:5801`).
- **Rust** (`lib.rs:103` -> `ah_auth_verify_2fa`, `lib.rs:852-910`):
  - Accepts `user_id` or `username` (`lib.rs:846-850`).
  - **Gap:** Queries `id, username, totp_secret` (`lib.rs:866`) and **does not check `totp_enabled`** before verifying.
  - Calls `auth_routes::verify_totp_or_backup` (`auth_routes.rs:44-87`); returns `{"ok": true, "token": token}` (`lib.rs:909`), omitting `backup_used`.
- **Web** (`[...nextauth].js:192-197`, `login.jsx:273-292`):
  - `do2FA` / `doBackup2FA` re-invoke `signIn('credentials', { email: pending.email, password: pending.pw, totpCode })`, causing NextAuth to call `/pub/auth/login` again and then `POST /pub/auth/verify-2fa` with `{ user_id: user.id, code: credentials.totpCode }`.

---

## 3. Password Hashing (`bcrypt` / `scrypt` / `pbkdf2`)

- **Python** (`bot.py:5263-5274`):
  - `_hash_pw` (`bot.py:5263-5267`): **PBKDF2-HMAC-SHA256**, 260,000 iterations, 16-byte random hex salt, stored as `"{salt}${dk.hex()}"`.
  - `_verify_pw` (`bot.py:5269-5274`): splits on `$` and compares `_hash_pw(password, salt) == stored` (non-constant-time `==`).
- **Rust** (`zenne-core/src/auth.rs:98-104`):
  - `hash_password` (`auth.rs:98-100`): **bcrypt** with cost `10` (`bcrypt::hash(pw, 10)`).
  - `verify_password` (`auth.rs:102-104`): `bcrypt::verify(pw, hash).unwrap_or(false)`.
- **Neither** uses `scrypt`.
- **CRITICAL PARITY BREAK:** Rust only verifies `bcrypt` hashes and has no PBKDF2-HMAC-SHA256 fallback. All existing `web_users.password_hash` entries created by Python (`salt$hex`) will fail login in Rust.

---

## 4. TOTP Verification & Backup Codes

- **TOTP Algorithm**:
  - **Python** (`bot.py:5478-5510`): `_gen_totp_secret` generates 10 random bytes (80-bit) Base32 without padding (`bot.py:5478-5479`). `_totp_verify` (`bot.py:5493-5503`) uses SHA1, 6 digits, 30s step, `window=1` (±30s), `hmac.compare_digest`. `_totp_uri` (`bot.py:5505-5510`) sets `issuer="NoiTuBot"`.
  - **Rust** (`zenne-core/src/auth.rs:112-139`): `gen_totp_secret` (`auth.rs:112-114`) uses `totp_rs::Secret::generate_secret()` (160-bit). `verify_totp_code` (`auth.rs:116-132`) uses `TOTP::new(Algorithm::SHA1, 6, 1, 30, ...)` (parity on SHA1/6 digits/skew=1/30s). `totp_uri` (`auth.rs:134-139`) sets `issuer="ZenneNoitu"` (differs from Python `"NoiTuBot"`).
- **Backup Codes**:
  - **Python** (`bot.py:5512-5559, 5951`): `_gen_backup_codes` generates 8 hex `XXXX-XXXX` codes stored as **plaintext** JSON in `web_users.backup_codes`. `_verify_2fa_or_backup` normalizes `clean.replace("-", "").upper()`.
  - **Rust** (`auth_routes.rs:19-28, 44-87, 692-700`): `gen_backup_codes` generates 8 numeric `XXXX-XXXX` codes (`1000..=9999`), stored as **SHA-256 hashes** (`auth::hash_token`). `verify_totp_or_backup` (`auth_routes.rs:70-71`) matches `codes.iter().position(|x| x == c || x == &h)` where `h = auth::hash_token(&c.to_uppercase())`. It supports both plaintext (legacy Python) and SHA-256 hashed codes, **except** Rust does not strip `-` before comparing (unlike Python).

---

## 5. Pending 2FA Token

- **Python**: No `pending_2fa` token is issued. Web (`pages/auth/login.jsx:163,260`) caches `{ email, pw }` in React state `pending` and re-submits them with `totpCode` to NextAuth.
- **Rust**: No `pending_2fa` token is issued either. Worse:
  1. `handle_login` (`auth_routes.rs:229-242`) returns a **full 7-day `token`** even when `totp_enabled == 1` and `totp_code` was not supplied (`"requires_2fa": true`), allowing complete 2FA bypass on direct API calls.
  2. `ah_auth_verify_2fa` (`lib.rs:852-910`) accepts `user_id` or `username` + `code` without a `pending_2fa` token, allowing unauthenticated attackers who know a `user_id`/`username` to call `/pub/auth/verify-2fa` directly and mint a 7-day `token` (`lib.rs:907-909`) without ever knowing the user's password!

---

## 6. Brute-Force Lockout & Rate Limiting

- **Python**: `_ah_auth_login` (`bot.py:5752-5768`) and `_ah_auth_verify_2fa` (`bot.py:5770-5801`) have **no rate limiting** (`_rate_limited` is not called) and **no account lockout**.
- **Rust**:
  - `handle_login` (`auth_routes.rs:169-180`): enforces `cache.rate_limited(&format!("login:{}", ident.to_lowercase()), 20, 60)` (`zenne-core/src/cache.rs:122-148`) — 20 hits/60s per identifier (`429`), with no IP limit and no progressive/account lockout. Note: `cache.rs:140-146` in-memory fallback resets `expires_at` on every hit, whereas Redis (`cache.rs:124-131`) uses a fixed window.
  - `ah_auth_verify_2fa` (`lib.rs:852-910`): has **no rate limiting** and **no brute-force lockout** (critical for 6-digit TOTP).
- **Web**: `[...nextauth].js:167-170` only verifies `turnstileToken` **if provided** (`if (credentials?.turnstileToken)`), so omitting `turnstileToken` bypasses Turnstile; `do2FA` / `doBackup2FA` (`login.jsx:277,288`) also omit `turnstileToken`.
