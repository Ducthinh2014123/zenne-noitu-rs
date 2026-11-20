# Forensic Audit Report — ID 52

## Scope & Inspected Files
- `zenne-noitu-deploy-vercel/pages/api/auth/{[...nextauth].js, steam-callback.js, steam-login.js}`, `pages/_app.jsx`
- `zenne-noitu-rs/crates/zenne-api/src/lib.rs` (`ah_auth_oauth`), `gdrive-bot-source/bot.py`

---

## 1. Refresh Token Persistence
- **No backend database persistence:** OAuth `access_token`, `refresh_token`, and `expires_at` are never sent to `/pub/auth/oauth` (`ah_auth_oauth` / `_ah_auth_oauth`) and live solely in the encrypted NextAuth JWT cookie (`[...nextauth].js:245-250`).
- **Google `refresh_token` gap:** `GoogleProvider` (`L130-134`) lacks `access_type: 'offline', prompt: 'consent'`.
- **Unused backend JWT:** `ah_auth_oauth` (`lib.rs:960-974`) signs a 7-day `web_token`, but `signIn` (`[...nextauth].js:230-233`) discards it.

---

## 2. Token Rotation & Concurrency Edge Cases
- **Concurrent refresh race:** `_app.jsx:7-11` (`refetchInterval={5 * 60}`, `refetchOnWindowFocus={true}`) has no mutex around `refreshOAuthAccessToken` (`[...nextauth].js:251-252`). Concurrent refreshes on single-use rotating providers (Discord/Top.gg) cause `invalid_grant`.
- **Infinite retry loop after failure:** `callbacks.jwt` (`L251`) does not short-circuit when `token.error === 'RefreshAccessTokenError'`.

---

## 3. Expiry Edge Cases (`expires_at` seconds vs ms)
- `L248`: `account.expires_at * 1000` assumes seconds; `L119`: `(Number(d.expires_in) || 3600) * 1000`; `L251` has zero clock-skew margin.
- Session `maxAge` is 30 days (`L218`) vs backend `sign_web_token` 7 days (`lib.rs:964`).

---

## 4. `RefreshAccessTokenError` Propagation to Client Session
- **Swallowed error (CRITICAL):** `callbacks.session` (`[...nextauth].js:256-263`) never forwards `token.error` to `session`, so the client UI never detects refresh failures.

---

## 5. Provider-Specific Gaps (GitHub / Facebook / Steam)
- **GitHub & Facebook:** `refreshOAuthAccessToken` (`L94-108`) falls through to `return token` unchanged.
- **Steam:** `steam-callback.js:116` redirects to `/auth/login?steam_token=...`, which neither `[...nextauth].js` nor `login.jsx` consumes (broken Steam login).
