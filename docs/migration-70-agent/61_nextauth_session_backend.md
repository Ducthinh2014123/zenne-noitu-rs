# Forensic Audit Report — ID 61

## Scope Inspected
- `zenne-noitu-deploy-vercel/pages/api/auth/[...nextauth].js`, `pages/_app.jsx`, `pages/auth/login.jsx`, `pages/account/index.jsx`, `lib/serverApi.js`
- `zenne-noitu-rs/crates/zenne-api/src/{lib.rs,auth_routes.rs}`

---

## 1. JWT & Session Callbacks
- `authOptions.session` (`[...nextauth].js:218`): `{ strategy: 'jwt', maxAge: 30 * 24 * 60 * 60 }`.
- `signIn` (`L221-236`): Calls `upsertOAuthUser` -> `POST /pub/auth/oauth`. On failure (`L84`), falls back to string `providerAccountId`, breaking numeric `user_id: i64` in `resolve_account_uid` (`lib.rs:1024-1038`).
- `session` (`L256-263`): Copies `id`, `isAdmin`, `username`, `image`, `provider`, but omits `token.error` and `backendToken`.

## 2. `totp_enabled` / `Needs2FA` / `EmailNotVerified` Flows
- `EmailNotVerified`: Compatible (`403` -> `openVerifyStep`).
- `Needs2FA`: Backend `handle_login` (`auth_routes.rs:229-242`) still mints a 7-day `token` when `totp_code` is omitted. On frontend, `login.jsx:211` (`?error=Needs2FA`) opens 2FA with empty `pending` credentials.
- `setup-2fa` QR mismatch: Rust `lib.rs:1134` returns `qr_uri` instead of `qr_url` expected by `account/index.jsx:282`.

## 3. `backendToken` Propagation
- Absent. `[...nextauth].js` discards backend `token`; Next.js proxies rely on `X-API-Key: BOT_ADMIN_API_KEY` + `user_id`.

## 4. `RefreshAccessTokenError` Handling
- Silently swallowed: `callbacks.session` omits `token.error`, and `_app.jsx`/`login.jsx`/`account/index.jsx` never check `session?.error`.

## 5. `signOut` & Expiry Handling
- `_app.jsx:7-11` polls every 5m (`refetchInterval={5 * 60}`); no server-side token revocation on `signOut`.
