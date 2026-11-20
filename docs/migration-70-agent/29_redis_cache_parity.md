# Forensic Audit Report — ID 29

## Scope & Files Audited
- **Rust**:
  - `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-core\src\cache.rs` (`CacheManager`)
  - `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-core\src\wiki.rs` (`check_word`)
  - `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-core\src\db.rs` (`guild_settings` / `word_bank` queries)
  - `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-api\src\lib.rs` (`dc_user:` cache & `word_bank` delete)
  - `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-api\src\auth_routes.rs` (`rate_limited` call sites)
- **Python**:
  - `C:\Users\Ai Hoi\Downloads\gdrive-bot-source\bot.py` (latest 6,348-line bot with Redis + In-Memory Cache layer; note that the older `archive-2026-09-23T144146Z\bot.py` has no Redis layer and only uses `_RATE_LIMIT_BUCKETS` at L4017-4031)

---

## 1. Key Prefixes
| Domain | Rust (`zenne-noitu-rs`) | Python (`gdrive-bot-source/bot.py`) | Parity |
|---|---|---|---|
| Rate limiting | `rl:{key}` in `CacheManager::rate_limited` (`cache.rs:123`). Callers in `auth_routes.rs`: `login:{ident}` (L172), `reg:{email}` (L267), `reg_verify:{email}` (L379), `reg_resend:{email}` (L519), `rec_req:{ident}` (L758), `rec_ev:{ident}` (L820). | `rl:{key}` on Redis path in `_rate_limited` (`bot.py:5303`), **but raw `{key}` without `rl:` prefix** in `_RATE_LIMIT_BUCKETS` fallback (`bot.py:5313`). Callers: `register:{ip}` (L5584), `register_verify:{ip}` (L5656), `register_resend_ip:{ip}` (L5716), `register_resend_email:{email}` (L5718), `recovery_request:{ip}` (L5984), `recovery_email_verify:{ip}` (L6023), `recovery_token_verify:{ip}` (L6073), `recovery_reset_password:{ip}` (L6139). | **Partial** — same `rl:` Redis prefix, but subkey names differ and Python's in-mem dict omits `rl:`. |
| Wiki word cache | `wiki:{lang}:{word}` in `check_word` (`wiki.rs:79`). | `wiki:{lang}:{norm_word}` in `db_cache_exists` (`bot.py:1221`) & `db_cache_add` (`bot.py:1245`). | **Match** |
| Discord user profile | `dc_user:{uid}` in `zenne-api/src/lib.rs:350` (JSON `{"name", "avatar"}`). | **Not present** — `bot.py` calls `await bot.fetch_user(uid)` directly (`bot.py:4422, 4588`). | **Missing in Python** |

---

## 2. TTLs
- **Wiki positive hit (`"1"`)**:
  - Rust (`wiki.rs:90, 111, 128`): `86400` seconds (1 day).
  - Python (`bot.py:1233, 1245`): `86400 * 7` = `604800` seconds (7 days).
- **Wiki negative hit (`"0"`)**:
  - Rust (`wiki.rs:136`): `3600` seconds (1 hour).
  - Python: **Not cached** in Redis or `_mem_cache`.
- **`dc_user:{uid}`**:
  - Rust (`zenne-api/src/lib.rs:388`): `3600` seconds (1 hour).
  - Python: N/A.
- **L1 In-Memory Backfill TTL Mismatch (Rust)**:
  - `CacheManager::get` (`cache.rs:70`) backfills `self.mem` on a Redis hit with a **hardcoded `300` seconds** (`Duration::from_secs(300)`), ignoring the key's actual remaining Redis TTL. Python `cache_get` (`bot.py:433-447`) does not backfill `_mem_cache` on Redis hits.
- **No-expiry writes (`ttl_secs == 0` / `ex_seconds=None`)**:
  - Rust (`cache.rs:81-85, 105-109`): `expires_at: None` in `mem`, `mgr.set(key, value)` in Redis.
  - Python (`bot.py:453, 458`): `exp = None` in `_mem_cache`, `_redis_client.set(key, value, ex=ex_seconds)` in Redis.

---

## 3. `rate_limited` Sliding vs Fixed Window Behavior
- **Rust (`CacheManager::rate_limited`, `cache.rs:121-148`)**:
  - Doc comment (`cache.rs:121`) states *"Sliding-window rate limiter matching Python's `_rate_limited` (`rl:{key}`)"*, which is **inaccurate** on both paths:
  - **Redis path (`cache.rs:124-132`)**: `INCR rl:{key}` + `EXPIRE rl:{key} window_secs` when `cnt == 1`. This is a **fixed window**.
  - **In-memory fallback (`cache.rs:133-147`)**: Reads `next_cnt` if `now < expires_at`, then **resets `expires_at = Some(now + Duration::from_secs(window_secs))` on every hit** (`cache.rs:144`). This is an **extending / idle-resetting window** (not fixed and not true sliding-window: a steady stream of requests keeps pushing expiration forward).
- **Python (`_rate_limited`, `bot.py:5298-5320`)**:
  - **Redis path (`bot.py:5301-5309`)**: `INCR rl:{key}` + `EXPIRE` when `cnt == 1` — **fixed window** (matches Rust's Redis path).
  - **In-memory fallback (`bot.py:5311-5320`)**: `_RATE_LIMIT_BUCKETS` (`dict[key, list[timestamps]]`) trims timestamps `< now - window_seconds` (`bot.py:5314-5316`) and checks `len(bucket) >= limit`. This is a **true sliding window**.
- **Parity verdict**: Redis paths match (both fixed-window), but both languages diverge internally between Redis and fallback, and Rust's fallback (extending window) diverges from Python's fallback (true sliding window).

---

## 4. Memory Eviction (`prune` vs Unbounded)
- **Rust `CacheManager::set` (`cache.rs:87-94`)**:
  - Cap at `50_000` keys. When `self.mem.len() > 50_000`, retains only unexpired entries (`now < e`), and if still `> 50_000`, calls `self.mem.clear()` (full wipe).
- **Rust `CacheManager::rate_limited` (`cache.rs:140-146`)**:
  - Inserts into `self.mem` **without** checking the `50_000` cap or pruning — unbounded growth if `set()` is not called.
- **Python `cache_set` (`bot.py:460-464`)**:
  - Cap at `20_000` keys. When `len(_mem_cache) > 20000`, prunes expired entries (`exp_at and exp_at <= now`). **No `clear()` or LRU eviction** if `> 20,000` active or `ex_seconds=None` keys exist — unbounded growth above 20k.
- **Python `_rate_limited` (`bot.py:5296, 5313`)**:
  - `_RATE_LIMIT_BUCKETS` has **no size cap and no key-level pruning**; expired timestamps are only popped when that exact key is hit again, leaking memory for one-off IPs/emails.

---

## 5. Cache Invalidation on Writes (`word_bank`, `guild_settings`, `dc_user`)
- Both `CacheManager::delete` (`cache.rs:113-119`) and `cache_delete` (`bot.py:467-475`) are implemented, **but neither is called anywhere in either codebase** (0 call sites).
- **`word_bank`**: Not cached in Redis or in-memory cache in Rust (`db.rs:328-357`, `zenne-api/src/lib.rs:707`) or Python (`bot.py:1121, 1199, 4255, 4281, 4322`). In Rust `wiki::check_word` (`wiki.rs:71` vs `L80`), `db.word_exists` is checked *before* `cache.get`, so newly added `word_bank` entries bypass any negative `wiki:{lang}:{word} = "0"` cache.
- **`guild_settings`**: Not cached in Redis or in-memory cache in Rust (`db.rs:242-326`) or Python (`bot.py:1074-1102`); all reads/writes go straight to DB.
- **`dc_user`**: Cached in Rust (`zenne-api/src/lib.rs:350-390`, 3600s TTL) with **no write invalidation** (TTL-only expiry). Not cached in Python.

---

## 6. Fallback Behavior When Redis is Unavailable
- **Startup failure**:
  - Rust (`CacheManager::new`, `cache.rs:19-48`): Empty/invalid URL or `ConnectionManager::new` failure logs `warn!` and sets `redis: None` (permanent `DashMap` fallback for process lifetime; no reconnect).
  - Python (`_init_redis`, `bot.py:386-431`): Tries `REDIS_PUBLIC_URL`, `REDIS_URL`, and hardcoded `public_fallback` (`bot.py:398`); if `HAS_REDIS` is False or `ping()` fails, sets `_redis_is_connected = False, _redis_client = None` (permanent `_mem_cache` fallback).
- **Runtime Redis behavior & cache warmth**:
  - **Rust (`cache.rs`)**:
    - `get` (`L50-78`): Checks `mem` first; on miss, queries Redis (silently returns `None` on Redis error) and backfills `mem` (300s TTL).
    - `set` (`L80-111`): **Dual-writes** to `mem` and Redis (ignoring Redis errors), keeping `mem` warm if Redis drops.
    - `delete` (`L113-119`): Removes from `mem` and Redis (ignoring Redis errors).
    - `rate_limited` (`L124-147`): Tries Redis first; on error, falls back to `mem`.
  - **Python (`bot.py`)**:
    - `cache_get` (`L433-447`): Queries Redis first; on exception, falls back to `_mem_cache`.
    - `cache_set` (`L449-465`): **Returns `True` immediately after Redis `set` (`L454`), skipping `_mem_cache`**. `_mem_cache` is only written when Redis is disconnected or throws. Thus, if Redis fails mid-run, `_mem_cache` starts completely cold.
    - `cache_delete` (`L467-475`): Deletes from Redis (ignoring exceptions) and pops from `_mem_cache`.
    - `_rate_limited` (`L5298-5320`): Queries Redis first; on exception, falls back to `_RATE_LIMIT_BUCKETS` (also cold if Redis was previously healthy).
