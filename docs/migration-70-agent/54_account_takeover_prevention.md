# Forensic Audit Report — ID 54

**Scope:** Password recovery, OTP, 2FA, and account takeover prevention audit across `zenne-api/src/{auth_routes.rs,lib.rs}`, `zenne-core/src/auth.rs`, `zenne-discord/src/extra_cmds.rs`.

---

## Critical Findings

1. **`handle_login` 2FA Bypass (`auth_routes.rs:211-242`):** When `totp_enabled == 1` and `totp_code` is omitted/empty, `handle_login` still calls `auth::sign_web_token` and returns a valid 7-day `"token"` alongside `"requires_2fa": true`.
2. **Unauthenticated `/pub/auth/verify-2fa` Token Minting (`lib.rs:852-910`):** Accepts `user_id`/`username` + `code` without password or pre-auth challenge token, has no rate limit, and does not check `totp_enabled == 1` or `email_verified == 1`.
3. **`handle_register_email_verify` Account Takeover (`auth_routes.rs:405-413`):** When `u.email_verified == 1`, immediately mints and returns a 7-day `"token"` without checking `body.code`.
4. **Unverified `/pub/auth/oauth` Account Takeover (`lib.rs:922-978`):** Upserts `web_users` on `email` conflict and mints a 7-day `"token"` without `check_admin_auth` or OAuth provider token verification.

---

## High / Medium Findings

5. **Missing `attempts < 5` Atomic Check (`auth_routes.rs:417, 452-481, 847, 881-910`):** `attempts >= 5` checked in Rust before `UPDATE ... SET attempts = attempts + 1 WHERE id = ?1` without `AND attempts < 5`, and `used_at` consumption lacks `AND attempts < 5`.
6. **Email / Account Enumeration (`auth_routes.rs:304, 389, 529, 830`, `lib.rs:891`):** Different error messages / HTTP statuses (`404`/`400`) leak account existence.
7. **Backup Code Normalization & Race (`auth_routes.rs:44-87`):** Does not strip `-` or spaces; `x == c` matches raw SHA-256 hex digest; non-atomic read-modify-write allows concurrent reuse.
8. **`setup-2fa` State Corruption (`lib.rs:1101-1135`, `auth.rs:134-139`):** Overwrites `totp_secret` even when `totp_enabled == 1`, doesn't reset `totp_enabled = 0`, and `totp_uri` doesn't URL-encode `username`.
9. **`revoke_2fa` leaves `backup_codes` intact (`extra_cmds.rs:329`)** and password change/reset does not invalidate existing 7-day tokens.
