# Forensic Audit Report — ID 58

## 1. `rate_limited` Fixed vs Extending Window (and Sliding-Window Parity)
- **`zenne-core/src/cache.rs:121-148`:** Docstring claims sliding window, but Redis path (`L124-132`) is a **fixed window** (`INCR` + `EXPIRE` on `cnt == 1`), and in-memory fallback (`L133-147`) is an **extending window** (`expires_at` reset on every hit). Python `_rate_limited` (`bot.py:4017-4031`) is a true sliding window.

## 2. L1 300s Backfill TTL (`CacheManager::get` & `set`)
- `CacheManager::get` (`cache.rs:66-72`) backfills `self.mem` on Redis hits with a hardcoded `300s` TTL without checking Redis `TTL`, extending short-lived keys and bypassing the `50_000` capacity guard.

## 3. `wiki` Negative Caching on Transient Network Errors
- `zenne-core/src/wiki.rs:96-136`: `check_word` does not distinguish transport/timeout/429/5xx errors from 404 misses, caching `"0"` for `3600s` (1 hour) on transient failures. Python only caches positive hits.

## 4. Unbounded `rate_limited` In-Memory Map
- `CacheManager::rate_limited` (`cache.rs:133-147`) inserts into `self.mem` without the `50_000` eviction check present in `set()`. Python `_RATE_LIMIT_BUCKETS` also never deletes empty keys.

## 5. Stale `dc_user` / `word_bank` Consistency
- `dc_user:{uid}` (`zenne-api/src/lib.rs:350-390`) caches Discord profiles for 3600s with no `USER_UPDATE` invalidation.
- `ah_wordbank_del` (`lib.rs:707`) deletes across all guilds (`WHERE word = ?1` without `guild_id`), and `word_bank` mutations never invalidate `wiki:{lang}:{word}` or `wiki_cache`.
- `DbManager::word_exists` (`db.rs:332`) omits `COLLATE NOCASE` in `WHERE guild_id = ?1 AND word = ?2`.
