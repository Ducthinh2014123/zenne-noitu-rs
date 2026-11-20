# Forensic Audit Report — ID 23

**Scope:** OAuth (`/pub/auth/oauth`), Game Token (`/pub/auth/game-token`), Steam OpenID, NextAuth providers (Discord/Google/GitHub/Facebook/Top.gg), and Token Refresh across Python (`bot.py`), Rust (`zenne-api` / `zenne-core`), and Web (`zenne-noitu-deploy-vercel`).

---

## 1. `/pub/auth/oauth` Parity Audit

| Aspect | Python (`archive-2026-09-23T144146Z/bot.py`) | Rust (`zenne-noitu-rs/crates/zenne-api/src/lib.rs`) | Web (`zenne-noitu-deploy-vercel`) |
|---|---|---|---|
| **Handler & Route** | `_ah_auth_oauth` (L4423–4464), `POST /pub/auth/oauth` (L4832) | `ah_auth_oauth` (L922–978), `POST /pub/auth/oauth` (L104) | `upsertOAuthUser` (`pages/api/auth/[...nextauth].js` L70–85), `upsertSteamUser` (`pages/api/auth/steam-callback.js` L51–73) |
| **Auth Guard** | None (no `_chk(req)`) | None (no `check_admin_auth`) | `[...nextauth].js` sends no `X-API-Key`; `steam-callback.js` sends `X-API-Key` |
| **Missing `email`** | Returns `400 {"error": "Thieu email"}` (L4435–4436) | Synthesizes `${uname}@oauth.local` (L933–939) | `steam-callback.js` synthesizes `steam_${steamId}@noitu.local` (L66); NextAuth passes `user.email` as-is |
| **Username Collision** | Deduplicates `username` (strips spaces, max 25 chars, retries 10x with random `100..9999` suffix, L4448–4460) | **NO deduplication** — `INSERT` with `UNIQUE` `username` (`zenne-core/src/db.rs` L170) silently fails (`let _ = conn.execute(...)`, L941–957), returning bare `{"ok": true}` without `id`/`user`/`token` (L977) | N/A |
| **Existing Email Conflict** | Updates `avatar` and `provider_id` only; preserves `provider` (L4441–4444) | `ON CONFLICT(email) DO UPDATE` overwrites `provider`, `provider_id`, `avatar` (L945–948) | N/A |
| **`email_verified` & `provider` default** | Does not set `email_verified` on insert (L4455); defaults `provider` to `"oauth"` (L4430) | Sets `email_verified = 1` (L944); defaults `provider` to `"discord"` (L952) | N/A |
| **Response Payload** | `_user_to_dict(row)` (no `token`) (L4464) | `{ ok, id, username, email, is_admin, token, user }` with 7-day `sign_web_token` (L960–975) | `upsertOAuthUser` falls back to `{ id: providerAccountId, ... }` on failure (L84), which breaks integer `user_id` expectations downstream |

---

## 2. `/pub/auth/game-token` Parity Audit

| Aspect | Python (`bot.py`) | Rust (`zenne-api/src/lib.rs`, `zenne-core/src/auth.rs`, `config.rs`) | Web (`pages/api/pub/game-token.js`) |
|---|---|---|---|
| **Handler & Route** | `_ah_pub_game_token` (L4466–4484), `POST /pub/auth/game-token` (L4833) | `ah_pub_game_token` (L986–1022), `POST /pub/auth/game-token` (L105) | `handler` (L8–27), `POST /api/pub/game-token` |
| **Auth Guard** | `_chk(req)` (`X-API-Key` required, L4469) | `check_admin_auth` (`X-API-Key` / `Bearer api_key`) **OR** user `Bearer <web_token>` via `extract_bearer_uid` (L991–1018) | Requires NextAuth `getServerSession` with `session?.user?.id` (L11–14), calls `botApi('POST', '/pub/auth/game-token', ...)` with `X-API-Key` (L17–20) |
| **Secret Derivation** | `sha256(API_KEY + "::web-game-token")` (`_web_token_secret`, L67–68) | `WEB_TOKEN_SECRET` env var, defaulting to raw `api_key` (`zenne-core/src/config.rs` L70) | N/A (server-side proxy) |
| **Token Claims & TTL** | `_sign_web_token(uid, username, ttl_seconds=900)` -> `{"uid", "un", "exp"}` (**15 min**, L70–75) | `sign_web_token(..., uid, &uname, 3600)` -> `{"uid", "username", "exp"}` (**60 min**, `auth.rs` L9–22) | N/A |
| **Username Handling** | Truncated to 20 chars (`[:20]`, L4476) | Not truncated (L999) | `session.user.username \|\| session.user.name \|\| 'Nguoi choi'` (L16) |
| **WS Consumer** | Reads `payload.get("un")` (`bot.py` L3809) | Reads `claims.get("username")` (`ws_game.rs` L225) | N/A |
| **Response Payload** | `{"token": token, "username": username}` (L4484) | `{"token": tok, "user_id": uid}` (L1021) | Forwards backend JSON directly (L26) |

**Key Parity Drift:** Secret derivation, TTL (`900s` vs `3600s`), JWT-like claim key (`un` vs `username`), and HTTP response keys (`username` vs `user_id`) differ between Python and Rust.

---

## 3. Steam OpenID Audit

- **Step 1 (`pages/api/auth/steam-login.js` L5–19):** Redirects to `https://steamcommunity.com/openid/login` with `openid.mode=checkid_setup`, `openid.return_to=${base}/api/auth/steam-callback`, `openid.realm=${base}`.
- **Step 2 (`pages/api/auth/steam-callback.js` L9–117):**
  - Validates 17-digit `steamId` from `openid.claimed_id` (L86–91).
  - Verifies assertion via `check_authentication` POST to Steam (`verifySteamOpenId`, L9–24).
  - Fetches profile via Steam Web API `GetPlayerSummaries/v2` if `STEAM_API_KEY` is set (`fetchSteamProfile`, L26–49).
  - Upserts user via `POST /pub/auth/oauth` with `provider: 'steam'` (`upsertSteamUser`, L51–73).
  - Signs a 60-second HMAC-SHA256 `steam_token` using `NEXTAUTH_SECRET` (`signToken`, L75–80) and redirects to `${base}/auth/login?steam_token=...` (L116).
- **Critical Broken Flow:**
  - `pages/auth/login.jsx` contains **no** Steam login button and **no** handler to read `steam_token` from URL query parameters.
  - `pages/api/auth/[...nextauth].js` has **no** Steam provider or `CredentialsProvider` branch to verify `steam_token` and create a NextAuth session.
  - Neither Python `bot.py` nor Rust `zenne-api` implements native Steam OpenID endpoints.
  - Result: Steam OpenID authentication dead-ends on `/auth/login?steam_token=...` without logging the user in.

---

## 4. Discord / Google / GitHub (and Facebook / Top.gg) OAuth in NextAuth

- **File:** `C:\Users\Ai Hoi\Downloads\zenne-noitu-deploy-vercel\pages\api\auth\[...nextauth].js`
- **Configured Providers (`authOptions.providers`, L129–155):**
  - `GoogleProvider` (L130–134)
  - `GitHubProvider` (L135–139)
  - `DiscordProvider` (L140–145, explicit `callbackUrl: process.env.DISCORD_REDIRECT_URI || 'https://zenne-noitu.me/api/auth/callback/discord'`)
  - `FacebookProvider` (L146–150, **hardcoded fallback `clientId` `'3476491435872335'` and `clientSecret` `'9d52cec3bc722483a00b436651915c5f'`**)
  - Custom `TopGGProvider` (L45–68, L151–155)
  - All OAuth providers enable `allowDangerousEmailAccountLinking: true`.
  - Also note `PRIMARY_TURNSTILE_SECRET` is hardcoded at L18.
- **`signIn` Callback (L221–236):** Calls `upsertOAuthUser` for `account?.type === 'oauth'` and maps `id`, `isAdmin`, `username`, `provider` onto `user`. If an OAuth provider (e.g. GitHub/Discord/Top.gg) omits `email`, Python `/pub/auth/oauth` returns `400`, causing `upsertOAuthUser` to fall back to `providerAccountId` (L84).

---

## 5. Token Refresh Audit

- **NextAuth OAuth Refresh (`pages/api/auth/[...nextauth].js`):**
  - `jwt` callback (L237–255) persists `accessToken`, `refreshToken`, and `accessTokenExpires` (defaulting to `Date.now() + 3600 * 1000` if `account.expires_at` is absent, L248).
  - On expiry (`Date.now() >= token.accessTokenExpires`, L251), invokes `refreshOAuthAccessToken(token)` (L87–126):
    - Supports `google` (`https://oauth2.googleapis.com/token`), `discord` (`https://discord.com/api/oauth2/token`), and `topgg` (`https://top.gg/oauth2/token`).
    - `github` and `facebook` fall through and return `token` unchanged (L106–108).
    - On failure, returns `{ ...token, error: 'RefreshAccessTokenError' }` (L124).
  - **Gap:** `session` callback (L256–263) does **not** forward `token.error` or `token.accessToken` to `session`, so the frontend cannot detect `RefreshAccessTokenError` to prompt re-authentication.
- **Backend / Game Tokens:**
  - NextAuth uses JWT sessions with `maxAge: 30 * 24 * 60 * 60` (30 days, L218).
  - Neither Python `bot.py` nor Rust `zenne-api` provides a `/pub/auth/refresh` endpoint for backend `web_token` (7-day TTL in Rust `auth_routes.rs` L233 / `lib.rs` L964) or `game-token` (15m Python / 1h Rust); clients must re-fetch `/api/pub/game-token` using an active NextAuth session.
