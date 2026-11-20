# Forensic Audit Report — ID 21

**Scope:** Auth Registration & Email Verification parity audit across:
- **Python:** `C:\Users\Ai Hoi\Downloads\gdrive-bot-source\gdrive-bot\bot.py`
- **Rust:** `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-api\src\{lib.rs, auth_routes.rs}` & `zenne-core\src\{db.rs, cache.rs, auth.rs, config.rs}`
- **Web (Next.js):** `C:\Users\Ai Hoi\Downloads\zenne-noitu-deploy-vercel\`

---

## 1. Route Path Parity
None of the three codebases implement `/pub/auth/register-verify`. All three align on the following three endpoints:
- `POST /pub/auth/register` (`bot.py:6326`, `zenne-api/src/lib.rs:93`, `pages/api/auth/register.js:53`)
- `POST /pub/auth/register/email/verify` (`bot.py:6327`, `zenne-api/src/lib.rs:95-96`, `pages/api/auth/register/email/verify.js:24`)
- `POST /pub/auth/register/email/resend` (`bot.py:6328`, `zenne-api/src/lib.rs:99-100`, `pages/api/auth/register/email/resend.js:21`)

---

## 2. `POST /pub/auth/register` — Schemas, Validation, Password Complexity & Hashing

| Aspect | Web (`login.jsx` / `register.js`) | Python (`bot.py:_ah_auth_register`) | Rust (`auth_routes.rs:handle_register`) |
|---|---|---|---|
| **Request Schema** | `{ username, email, password, turnstileToken, acceptedTerms }` (`login.jsx:325`); proxy forwards `{ username, email, password }` (`register.js:56`) | `{ username, email, password }` (`bot.py:5581-5583`) | `RegisterBody { username, email, password }` (`auth_routes.rs:255-259`) |
| **Email Normalization** | Trimmed; not lowercased in `register.js:38` | `strip().lower()` (`bot.py:5582`) | `trim()` only — **NOT lowercased** (`auth_routes.rs:263`) |
| **Input & Password Complexity** | Client (`login.jsx:298-302`) & API (`register.js:39-48`): `username >= 3`, email regex, `password >= 6`, `rPw === rPw2`, `acceptedTerms`, Turnstile. `PasswordStrengthInput.jsx:89-136` is **visual-only** (no uppercase/digit/symbol enforcement). | Non-empty check, `len(username) >= 3`, `len(password) >= 6` -> `400` (`bot.py:5584-5589`). No extra complexity rules. | **PARITY GAP:** No empty check, no `username.len() >= 3`, no `password.len() >= 6` check (`auth_routes.rs:261-277`), unlike `handle_change_password` (`579`). |
| **Password Hashing** | Delegated to backend | PBKDF2-HMAC-SHA256, 260k iters (`_hash_pw`, `bot.py:5263-5267`) | Bcrypt cost 10 (`auth::hash_password`, `zenne-core/src/auth.rs:98-100`) |
| **Response Schema (200 OK)** | Reads `r.ok` and `d.expires_in` (`login.jsx:329-335`) | `{"success": true, "message": ..., "email": email, "expires_in": 600}` (`bot.py:5645-5650`) | If `smtp_ready`: `{"ok": true, "requires_verification": true, "expires_in": 600, "email": email}` (`auth_routes.rs:341-347`). If `!smtp_ready`: auto-verifies (`email_verified = 1`) and returns `{"ok": true, "token": ..., "expires_in": 600, "user": {...}}` (`350-362`). |

---

## 3. Duplicate Email / Username Handling

- **Python (`bot.py:5600-5641`):**
  - Queries `web_users` for `existing_email` and `existing_username` (`username=? AND email<>?`).
  - Existing username -> `409 Conflict` (`"Ten dang nhap da ton tai"`, `5607-5608`).
  - Existing **verified** email -> `409 Conflict` (`"Email hoac ten da ton tai"`, `5610-5611`).
  - Existing **unverified** email (`email_verified == 0`) -> **updates** `username` and `password_hash` on the existing row, marks prior `registration_email_code` rows as used, and issues a fresh OTP (`5613-5638`).
- **Rust (`auth_routes.rs:297-309`):**
  - **PARITY GAP:** Directly executes `INSERT INTO web_users ...` (`297-302`). Any `UNIQUE` violation (`db.rs:170-171`) returns **`400 Bad Request`** (not `409`) with raw SQLite error text (`"Tài khoản hoặc email đã tồn tại: {e}"`, `304-308`).
  - **PARITY GAP:** No unverified-account re-registration recovery — if a user fails to verify on the first attempt, their email/username is permanently blocked.
  - Because Rust does not lowercase `email` on insert (`263`), case variants (`User@x.com` vs `user@x.com`) bypass `TEXT UNIQUE`.

---

## 4. `POST /pub/auth/register/email/verify` — OTP Generation, Expiry, Attempts & Lockout

| Aspect | Web (`verify.js` / `login.jsx`) | Python (`bot.py:_ah_register_email_verify`) | Rust (`auth_routes.rs:handle_register_email_verify`) |
|---|---|---|---|
| **Request Schema** | `{ email, code }`, lowercases `email`, enforces `/^\d{6}$/` (`verify.js:15-19`) | `{ email, code }`, `email.strip().lower()`, non-empty check (`5658-5661`) | `EmailVerifyBody { email, code }` (`366-369`), `trim()` only (no lowercase) (`375`) |
| **OTP Generation & Storage** | N/A | `_gen_otp_code()`: CSPRNG `secrets.randbelow(1_000_000):06d` (`000000`–`999999`, `5411-5413`); SHA-256 hex (`_hash_token`, `5289-5290`) | `gen_6_digit_otp()`: `rand::thread_rng().gen_range(100_000..=999_999)` (`14-17`, never starts with `0`); SHA-256 hex (`auth.rs:106-110`) |
| **OTP TTL** | Expects `expires_in` (default `600`s) (`login.jsx:233-240, 268`) | `REGISTRATION_OTP_TTL_SECONDS = 600` (`102`); stored as float timestamp string (`5596`), checked via `float(expires_at) < time.time()` (`5682`) | 10 min (`318`), stored as RFC3339 (`to_rfc3339()`); `is_expired()` (`30-42`) parses both RFC3339 and float timestamps |
| **Unknown User** | N/A | `400 Bad Request` (`5669-5670`) | **`404 Not Found`** via `find_user_by_username_or_email` (`389-395`, `db.rs:1103-1125`) |
| **Already Verified** | N/A | `200 OK` `{"success": true, "already_verified": true}` (`5671-5672`) | `200 OK` `{"ok": true, "token": "<web_token>"}` (`405-413`) |
| **Attempts & Lockout** | Displays `d.error` (`login.jsx:385`) | `REGISTRATION_OTP_MAX_ATTEMPTS = 5` (`103`). `attempts >= 5` -> marks `used_at` and returns `400` (`5684-5687`). Constant-time `hmac.compare_digest` (`5689`). Mismatch increments `attempts`; if `attempts_left <= 0`, sets `used_at` and returns `400`; else returns `400` with `attempts_left` (`5690-5698`). | `attempts >= 5` -> returns **`429 Too Many Requests`** without setting `used_at` (`452-458`). Uses `stored_hash != code_hash` (`461`). Mismatch increments `attempts` (`462-467`), returns `400` **without** `attempts_left` and **without** setting `used_at` on the 5th failure (`468-472`). |
| **Success Response** | Calls `signIn('credentials', ...)` (`login.jsx:392-393`) | `200 OK` `{"success": true}` (`5704`) | `200 OK` `{"ok": true, "token": "<web_token>"}` (`497-503`) |

---

## 5. Rate Limiting

- **Python (`bot.py:_rate_limited`, lines 5304-5326):**
  - `/pub/auth/register`: `8` hits / `900s` per **IP** (`register:{ip}`, `5590`) -> `429`.
  - `/pub/auth/register/email/verify`: `15` hits / `600s` per **IP** (`register_verify:{ip}`, `5662`) -> `429`.
  - `/pub/auth/register/email/resend`: `5` hits / `900s` per **IP** (`register_resend_ip:{ip}`, `5722`) **AND** `1` hit / `60s` per **email** (`register_resend_email:{email}`, `5724`) -> `429`.
- **Rust (`zenne-core/src/cache.rs:122-148`, `auth_routes.rs`):**
  - **PARITY GAP:** Handlers in `zenne-api/src/lib.rs:824-843` do not extract client IP; all rate limits are **keyed by email** instead of IP:
    - `handle_register`: `5` hits / `300s` per `reg:{email.to_lowercase()}` (`267`) -> `429`.
    - `handle_register_email_verify`: `10` hits / `300s` per `reg_verify:{email.to_lowercase()}` (`379`) -> `429`.
    - `handle_register_email_resend`: `1` hit / `60s` per `reg_resend:{email.to_lowercase()}` (`519`) -> `429`. **Missing** the IP-based limit (`5 / 900s`).
  - Note: `CacheManager::rate_limited` (`cache.rs:134-146`) resets `expires_at` on every in-memory hit (sliding TTL), whereas Redis `incr` + `expire` on `cnt == 1` (`126-129`) is a fixed window.

---

## 6. Email Sending & `/pub/auth/register/email/resend`

- **Email Transport:**
  - **Python (`bot.py:91-94`, `5415-5475`):** Uses **Resend HTTP API** (`EMAIL_PROVIDER_URL = https://api.resend.com/emails`, `EMAIL_API_KEY`, `EMAIL_FROM = no-reply@zenne-noitu.me`), dispatched non-blocking via `run_in_executor` (`5644`, `5748`).
  - **Rust (`auth_routes.rs:89-144`, `zenne-core/src/config.rs:30-34,129-133`):** Uses **SMTP** (`lettre::AsyncSmtpTransport`, port 465 relay or 587 STARTTLS) via `SMTP_HOST`/`SMTP_PORT`/`SMTP_USER`/`SMTP_PASS`/`SMTP_FROM`, awaited inline (`331-340`, `554-560`). No Resend HTTP API support.
- **`/pub/auth/register/email/resend` Parity:**
  - **Python (`bot.py:5706-5750`):** Returns generic `200 OK` (`generic_ok`, `5716-5720`) regardless of whether email exists or is already verified (prevents enumeration), and only sends OTP if `urow and not urow["email_verified"]` (`5731`).
  - **Rust (`auth_routes.rs:511-563`):**
    - **PARITY GAP (Enumeration):** Returns **`404 Not Found`** if user is not found (`529-535`).
    - **PARITY GAP (Missing Verified Guard):** Does **not** check `u.email_verified == 0` before invalidating old OTPs and emailing a new OTP (`537-560`).
    - Returns `{"ok": true, "expires_in": 600}` (`562`).
