# Forensic Audit Report — ID 24

**Scope:** Account Management parity audit (`/pub/account*`: profile get/update, password change, 2FA setup/enable/disable, linked OAuth providers, active sessions/ban status) across Python (`C:\Users\Ai Hoi\Downloads\gdrive-bot-source\bot.py`), Rust (`C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-api\src\lib.rs` & `auth_routes.rs`, plus `zenne-core\src\auth.rs`), and Web (`C:\Users\Ai Hoi\Downloads\zenne-noitu-deploy-vercel\`).

---

## 1. Endpoint Inventory & Parity Matrix

| Feature | Python (`bot.py`) | Rust (`zenne-api`) | Web (`zenne-noitu-deploy-vercel`) | Status |
|---|---|---|---|---|
| **Profile Get** | `GET /pub/account` (`_ah_pub_account_info`, L5860–5878, route L6327) | `GET /pub/account` (`ah_pub_account_info`, `lib.rs` L1040–1079, route L106) | `pages/api/account/info.js` (L5–22) -> `GET /pub/account?user_id=...` | **Partial Parity (Minor Schema/Type Drift)** |
| **Profile Update** | Not implemented | Not implemented | Not implemented (`pages/account/index.jsx` L200–218 is read-only) | **Absent in All 3** |
| **Password Change** | `POST /pub/account/change-password` (`_ah_pub_account_change_password`, L5880–5902, route L6328) | `POST /pub/account/change-password` (`ah_pub_account_change_password`, `lib.rs` L1081–1094 -> `auth_routes::handle_change_password`, `auth_routes.rs` L573–632, route `lib.rs` L107–110) | `pages/api/account/change-password.js` (L5–18) | **Critical Parity Gap (Hash Alg + Error Codes)** |
| **2FA Setup** | `POST /pub/account/setup-2fa` (`_ah_pub_account_setup_2fa`, L5904–5924, route L6329) | `POST /pub/account/setup-2fa` (`ah_pub_account_setup_2fa`, `lib.rs` L1101–1135, route L111) | `pages/api/account/setup-2fa.js` (L5–13), `pages/account/index.jsx` (L95–104, L282) | **BREAKING Contract Mismatch** |
| **2FA Enable** | `POST /pub/account/enable-2fa` (`_ah_pub_account_enable_2fa`, L5926–5947, route L6330) | `POST /pub/account/enable-2fa` (`ah_pub_account_enable_2fa`, `lib.rs` L1137–1152 -> `auth_routes::handle_toggle_2fa`, `auth_routes.rs` L641–703, route `lib.rs` L112) | `pages/api/account/enable-2fa.js` (L5–16) | **Partial Parity (Storage & Status Drift)** |
| **2FA Disable** | `POST /pub/account/disable-2fa` (`_ah_pub_account_disable_2fa`, L5949–5972, route L6331) | `POST /pub/account/disable-2fa` (`ah_pub_account_disable_2fa`, `lib.rs` L1154–1169 -> `auth_routes::handle_toggle_2fa`, `auth_routes.rs` L641–729, route `lib.rs` L113) | `pages/api/account/disable-2fa.js` (L5–17) | **Partial Parity (Hash Alg + Status Drift)** |
| **Linked OAuth Providers** | Single `provider` / `provider_id` column on `web_users` (`bot.py` L923–924); no `/pub/account*` link/unlink endpoints | Same single `provider` column returned in `ah_pub_account_info` (`lib.rs` L1056, L1072); no link/unlink endpoints | Single `provider` badge rendered in `pages/account/index.jsx` (L7–14, L192–193, L213–215); NextAuth uses `allowDangerousEmailAccountLinking: true` (`pages/api/auth/[...nextauth].js` L133–154) | **Parity (Single-Provider Model Only)** |
| **Active Sessions / Ban Status** | No `/pub/account*` session list/revoke or ban-status endpoints; `web_users` schema (`bot.py` L918–933) has no `banned` column | No `/pub/account*` session list/revoke or ban-status endpoints | `pages/account/index.jsx` (L354–360) only renders current NextAuth `signOut()` button | **Absent in All 3** |

---

## 2. Detailed Forensic Findings

### Finding 1 (CRITICAL / BREAKING): `/pub/account/setup-2fa` Response Schema & QR URL Mismatch
- **Web UI expectation (`pages/account/index.jsx` L282):**
  Renders `<img src={tfaData.qr_url} alt="QR Code" .../>` and displays `tfaData.secret` (L286).
- **Python (`bot.py` L5922–5924):**
  Returns `{"secret": secret, "otpauth_url": uri, "qr_url": "https://api.qrserver.com/v1/create-qr-code/?size=220x220&data=" + urllib.parse.quote(uri, safe="")}` and sets `totp_secret=?, totp_enabled=0` (L5920). Issuer in `_totp_uri` is `"NoiTuBot"` (L5499).
- **Rust (`zenne-api/src/lib.rs` L1126–1134):**
  1. Returns `Json(json!({ "secret": secret, "uri": uri, "qr_uri": uri }))` — **missing `qr_url` and `otpauth_url`**, so `tfaData.qr_url` in the Web UI is `undefined` (broken QR image).
  2. Even `qr_uri` is the raw `otpauth://totp/...` string rather than an HTTPS QR image URL.
  3. SQL at L1128 (`UPDATE web_users SET totp_secret = ?1 WHERE id = ?2`) omits `totp_enabled = 0` (unlike `bot.py` L5920).
  4. `zenne_core::auth::totp_uri` (`zenne-core/src/auth.rs` L134–139) uses issuer `"ZenneNoitu"` instead of `"NoiTuBot"` and does not URL-encode `username`.

### Finding 2 (CRITICAL): Password Hashing Algorithm Incompatibility (PBKDF2 vs bcrypt)
- **Python (`bot.py` L5257–5268):**
  `_hash_pw` / `_verify_pw` uses **PBKDF2-HMAC-SHA256** (260,000 iterations, format `"{salt}${dk.hex()}"`).
- **Rust (`zenne-core/src/auth.rs` L98–104):**
  `hash_password` / `verify_password` uses **bcrypt** (`bcrypt::hash(pw, 10)`, `bcrypt::verify(pw, hash)`).
- **Impact on `/pub/account/change-password` & `/pub/account/disable-2fa`:**
  - In `auth_routes::handle_change_password` (`auth_routes.rs` L606) and `auth_routes::handle_toggle_2fa` (`auth_routes.rs` L711), `auth::verify_password` will always return `false` for any existing Python-created `web_users.password_hash` (`salt$hex`), locking migrated users out of changing their password or disabling 2FA via password.
  - Conversely, passwords set/changed via Rust (`bcrypt`) cannot be verified by Python.

### Finding 3 (MODERATE): `/pub/account` (GET) Response Field & Type Drift
- **Python (`bot.py` L5875–5877, `_user_to_dict` L5555–5567):**
  Returns `id`, `username`, `email`, `image` (from `row["avatar"]`), `is_admin` (`bool`), `isAdmin` (`bool`), `provider`, `totp_enabled` (`bool`), `has_password` (`bool`).
- **Rust (`zenne-api/src/lib.rs` L1064–1073):**
  Returns `id`, `username`, `email`, `is_admin` (`i64`: `0`/`1`), `totp_enabled` (`i64`: `0`/`1`), `has_password` (`bool`), `avatar` (`Option<String>`), `provider`.
  - **Drift:** `is_admin` and `totp_enabled` are integers (`0`/`1`) instead of booleans; `isAdmin` and `image` are omitted (Rust returns `avatar`, which `pages/api/account/info.js` L13 actually reads via `data.avatar || session.user.image`, whereas Python didn't return `avatar` at all).
- **Auth Model:**
  - Python (`bot.py` L5862) uses `_chk(req)` (`X-API-Key` / `?api_key=`) + `?user_id=`.
  - Rust (`lib.rs` L1024–1038, `resolve_account_uid`) accepts either `Authorization: Bearer <web_token>` OR `X-API-Key` + `user_id`, which is compatible with `lib/serverApi.js` L16 (`X-API-Key`).

### Finding 4 (MODERATE): Missing User Check & HTTP Status Code Drift in Password Change & 2FA Toggle
- **Non-existent `user_id` on `change-password`:**
  - Python (`bot.py` L5897–5898) returns `404 {"error": "Khong tim thay user"}`.
  - Rust (`auth_routes.rs` L594–631) skips password verification if `rows.next()` is `None`, executes `UPDATE ... WHERE id = ?2` (affecting 0 rows), and returns `200 {"ok": true}`.
- **Invalid credentials status codes:**
  - Python returns `401` on wrong `old_password` (`bot.py` L5900), invalid `enable-2fa` OTP (`bot.py` L5943), and invalid `disable-2fa` OTP/password (`bot.py` L5969).
  - Rust returns `400 Bad Request` in all three cases (`auth_routes.rs` L608, L687, L716).

### Finding 5 (LOW / COMPATIBLE): Backup Codes Storage & Verification
- **Python (`bot.py` L5944–5945, `_gen_backup_codes` L5506–5513):**
  Stores 8 plaintext `XXXX-XXXX` codes as JSON in `web_users.backup_codes`. `_ah_pub_account_disable_2fa` (`bot.py` L5966) only checks `_totp_verify` (not backup codes).
- **Rust (`auth_routes.rs` L692–702, `verify_totp_or_backup` L44–87):**
  Stores SHA-256 hashed backup codes (`auth::hash_token(c)`) in DB while returning plaintext codes to the client. `verify_totp_or_backup` checks `codes.iter().position(|x| x == c || x == &h)` (L71), so Rust maintains backward compatibility with Python's plaintext backup codes and also allows backup codes in `disable-2fa` (L707).

### Finding 6 (INFO): Absent Features Across All Layers
- **Profile Update (`username` / `avatar` / `ui_lang`):** Not implemented in Python, Rust, or Web.
- **Linked OAuth Providers Management:** `web_users` only stores a single `(provider, provider_id)` pair; no multi-provider linking/unlinking API exists in Python, Rust, or Web.
- **Active Sessions / Ban Status:** No `/pub/account/sessions` or ban-status endpoints exist in Python, Rust, or Web (`web_users` has no ban/session columns).
