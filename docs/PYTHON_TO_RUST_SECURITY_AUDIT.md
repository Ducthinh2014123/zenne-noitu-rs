# Python → Rust Security & Token Lifecycle Audit

Audited against:
- `C:\Users\Ai Hoi\Downloads\gdrive-bot-source\gdrive-bot\{bot.py, music_system.py}`
- `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\{zenne-core, zenne-api, zenne-music}`
- `C:\Users\Ai Hoi\Downloads\zenne-noitu-deploy-vercel\{pages/api/**, lib/**}`

## Verified Security & Token Findings

1. **WebSocket Game Join Token Verification (CRITICAL — Confirmed)**
   - **Legacy Python (`bot.py:4825-4838`)**: Verifies `token` with `_verify_web_token`, binds `uid` & `username`, prevents duplicate `uid`.
   - **Current Rust (`zenne-api/src/ws_game.rs:208-256`)**: Ignores `token` completely on `"join"`, allowing unauthenticated joins and username spoofing.
   - **Fix**: Verify `token` via `zenne_core::auth::verify_web_token(&ctx.config.web_token_secret, token)` (reject with `{ "type": "error", "msg": ... }` and close if invalid/expired) and bind `user_id`.

2. **Game Score Session Token & Anti-Cheat (CRITICAL — Confirmed)**
   - **Legacy Python (`bot.py:4469-4534`)**: Requires `X-API-Key`, requires `session_token`, checks `iat` elapsed velocity in `_sanity_check_score`, allows monotonic score updates on same `nonce` (`afk`).
   - **Current Rust (`zenne-api/src/lib.rs:447-491, 550-558`, `zenne-core/src/auth.rs:41-81`, `zenne-api/src/pub_games.rs:335-346`)**: `session_token` is `Option<String>` (skippable), neither `/pub/game/score` nor `/pub/game/session/start` checks `X-API-Key`, `sign_game_session` omits `iat`, and `used_nonces` blocks valid incremental updates.
   - **Fix**: Enforce `X-API-Key` when `api_key` is set, require `session_token`, encode `iat` in `sign_game_session` + verify elapsed time, and support monotonic high-water score tracking per nonce.

3. **Recovery OTP & Reset Session `expires_at` Checks (HIGH — Confirmed)**
   - **Legacy Python (`bot.py:5682, 6058, 6098, 6135, 6169`)**: Checks `expires_at` on all registration OTP, recovery OTP, recovery token, status, and reset endpoints.
   - **Current Rust (`zenne-api/src/auth_routes.rs:573-594, 649-670`, `lib.rs:1010-1012`)**: `handle_recovery_verify` and `handle_recovery_reset_password` never select or check `expires_at`; `ah_recovery_status` is a stub returning `{"status":"ok"}`.
   - **Fix**: Store/parse `expires_at` consistently (supporting both RFC3339 and epoch float strings) and enforce expiration on every verification/status/reset step.

4. **Password Reset 2FA/TOTP Validation (HIGH — Confirmed)**
   - **Legacy Python (`bot.py:6163-6189`)**: Requires `totp_code` (or backup code) during password reset if `totp_enabled == 1`.
   - **Current Rust (`zenne-api/src/auth_routes.rs:616-693`)**: Never checks `totp_enabled` or validates `totp_code` before resetting `password_hash`.
   - **Fix**: Check `totp_enabled` on `web_users` in both `/pub/auth/recovery/status` and `/pub/auth/recovery/reset-password`, requiring valid `totp_code` (or backup code) when enabled.

5. **Atomic Token Consumption, Rate Limiting & Lockout (HIGH — Confirmed)**
   - **Legacy Python (`bot.py:5304-5326, 5684-5698, 6056-6062, 6101-6106, 6191-6196`)**: Atomic `UPDATE ... WHERE id=? AND used_at IS NULL`, `attempts < 5` lockout, IP/email rate limiting, and invalidation of unused codes.
   - **Current Rust (`zenne-api/src/auth_routes.rs:313-318, 596-601, 680-691`)**: Non-atomic `UPDATE` without `AND used_at IS NULL` or rowcount check; updates `password_hash` before marking token used; zero rate limiting or attempt lockout.
   - **Fix**: Perform atomic `UPDATE ... WHERE id = ?1 AND used_at IS NULL` checking `rows_affected == 1` *before* mutating `password_hash`, enforce `attempts < 5`, invalidate prior unused codes on issue/reset, and add rate limiting.

6. **Web and Rust Login / 2FA / OAuth / Recovery Payload Incompatibility (CRITICAL — Confirmed)**
   - See `docs/PYTHON_TO_RUST_API_COMPATIBILITY.md` for exact field mismatches.

7. **Game Token Issuance `/pub/auth/game-token` (CRITICAL — Confirmed)**
   - **Current Rust (`zenne-api/src/lib.rs:879-887`)**: Missing `X-API-Key` check and ignores JSON `{ user_id, username }`, always minting `uid=0` guest tokens for Next.js.
   - **Fix**: Enforce `check_admin_auth` (`X-API-Key`) and parse `{ user_id, username }` from JSON body (while remaining compatible with Bearer auth if provided).

8. **Constant-Time HMAC Signature Comparison (MEDIUM — Confirmed)**
   - **Current Rust (`zenne-core/src/auth.rs:29, 65`)**: Uses non-constant-time `!=` on hex strings.
   - **Fix**: Decode hex and call `mac.verify_slice(&sig_bytes).is_ok()` (constant-time via `hmac` crate).

9. **SoundCloud `client_id` Automatic Renewal (HIGH — Confirmed)**
   - **Legacy Python (`music_system.py:69-94`)**: Scrapes `soundcloud.com` / `a-v2.sndcdn.com/assets/*.js` when expired (24h cache).
   - **Current Rust (`zenne-music/src/lib.rs:50-63`)**: Static `String` from startup env; no renewal or 401/403 retry.
   - **Fix**: Store `Arc<RwLock<(String, Instant)>>` in `MusicManager` with `get_or_refresh_client_id(force: bool)` that scrapes `soundcloud.com` JS bundles on expiry or HTTP 401/403.

10. **NextAuth OAuth Provider Token Persistence & Refresh (MEDIUM — Confirmed)**
    - **Web (`pages/api/auth/[...nextauth].js:196-205`)**: `callbacks.jwt` drops `account.access_token`, `account.refresh_token`, and `account.expires_at`.
    - **Fix**: Persist `accessToken`, `refreshToken`, and `accessTokenExpires` in NextAuth JWT callback and refresh expired OAuth access tokens (Google/Discord/Top.gg) when `refreshToken` is present.
