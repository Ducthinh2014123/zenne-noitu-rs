# Forensic Audit Report — ID 44

## Scope & Files Audited
- **Python:** `C:\Users\Ai Hoi\Downloads\gdrive-bot-source\gdrive-bot\telegram_bot.py` (`handle_callback_query`, `handle_text_message`, `InlineKeyboardMarkup`, `create_telegram_app`)
- **Rust:**
  - `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-telegram\src\lib.rs`
  - `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-telegram\src\state.rs`
  - `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-telegram\src\noitu.rs`
  - `C:\Users\Ai Hoi\Downloads\zenne-noitu-rs\crates\zenne-telegram\src\minigames.rs`

---

## 1. Callback Data Prefixes & Button Parity

Every `callback_data` string/prefix in Python is implemented in Rust with identical naming:

| Category | `callback_data` / Prefix | Python Location (`telegram_bot.py`) | Rust Location |
|---|---|---|---|
| **Quick Menu (`/start`)** | `mp_menu_open`, `btn_quick_noitu`, `btn_quick_solo`, `btn_quick_quiz`, `btn_quick_vtv`, `btn_quick_taixiu`, `btn_quick_top`, `btn_quick_help` | L275–288 (markup), L1433–1453, L1484 (handler) | `noitu.rs` L242–256 (markup), L1198–1242, L1316 (handler) |
| **Nối Từ Lobby** | `nt_getlink`, `nt_start`, `nt_cancel` | L363–367, L719–720 (markup), L1456–1481 (handler) | `noitu.rs` L61–67, L883–884 (markup), L1245–1313 (handler) |
| **Multiplayer Menu** | `mp_create`, `mp_list`, `mp_menu` | L622–628, L723, L1514–1542 (markup), L1484–1565 (handler) | `noitu.rs` L82–90, L888, L1348–1416 (markup), L1316–1438 (handler) |
| **Quiz** | `qz_ans:<idx>`, `qz_stop` | L1568–1599 (handler) | `minigames.rs` L231–246 (markup), L731–772 (handler) |
| **Vua Tiếng Việt** | `vtv_hint`, `vtv_giveup` | L1602–1615 (handler) | `minigames.rs` L274–275 (markup), L774–803 (handler) |
| **Tài Xỉu** | `tx_bet:tai`, `tx_bet:xiu` | L1618–1628 (handler) | `minigames.rs` L308–309 (markup), L805–830 (handler) |
| **Bầu Cua** | `bc_bet:<nai\|bau\|ga\|ca\|cua\|tom>` | L1631–1644 (handler) | `minigames.rs` L408–415 (markup), L832–857 (handler) |
| **Ca Dao** | `cd_ans:<idx>` | L1647–1669 (handler) | `minigames.rs` L516–519 (markup), L859–888 (handler) |
| **Chính Tả** | `ct_ans:0`, `ct_ans:1` | L1672–1694 (handler) | `minigames.rs` L545–546 (markup), L890–919 (handler) |
| **Flag Quiz** | `fq_ans:<idx>` | L1697–1718 (handler) | `minigames.rs` L577–580 (markup), L921–950 (handler) |
| **Ai Là Triệu Phú** | `tp_ans:<idx>`, `tp_5050`, `tp_aud`, `tp_stop` | L1351–1358, L1778–1783 (markup), L1721–1811 (handler) | `minigames.rs` L615–637, L1056–1075 (markup), L952–1142 (handler) |

---

## 2. Stale Message & `InaccessibleMessage` Handling

### Rust (`zenne-telegram`)
1. **`InaccessibleMessage` silent drop on Quick Menu minigames (`noitu.rs` L1209–1230):**
   - `handle_callback_query` extracts `chat_id = msg.chat().id` and `msg_id = msg.id()` (L1192–1193), which work on `MaybeInaccessibleMessage`.
   - However, `btn_quick_quiz` (L1211–1214), `btn_quick_vtv` (L1219–1221), and `btn_quick_taixiu` (L1226–1228) gate on `msg.regular_message()`. If the `/start` message is `InaccessibleMessage` (e.g., >48h old or deleted), `regular_message()` returns `None`, so the bot answers the callback query and **silently does nothing**—even though `minigames::cmd_quiz`, `cmd_vtv`, and `cmd_taixiu` only use `msg.chat.id`! (Compare with `btn_quick_noitu`, `btn_quick_solo`, `btn_quick_top`, `btn_quick_help` at L1199–1240, which pass `chat_id` directly.)
2. **`q.message == None` (`noitu.rs` L1188–1191, `minigames.rs` L723–725):**
   - Answers callback query with empty `answer_callback_query` and returns.
3. **Stale `edit_message_text` / `edit_message_reply_markup`:**
   - `mp_menu_open`/`mp_menu` (`noitu.rs` L1318–1325) and `mp_list` (L1354–1366, L1424–1436) check `is_err()` and fall back to `cmd_multiplayer` or `bot.send_message(chat_id, ...)`.
   - All other edits (`nt_cancel`, `qz_ans:`, `qz_stop`, `vtv_giveup`, `cd_ans:`, `ct_ans:`, `fq_ans:`, `tp_ans:`, `tp_5050`, `tp_stop`) ignore errors with `let _ = ... .await;`.
4. **Stale session feedback gaps:**
   - `qz_ans:` (`minigames.rs` L734–736) silently returns `true` when `active_quiz` is already gone (whereas Python L1571–1573 shows `"Câu hỏi này đã kết thúc!"` alert).
   - `nt_getlink`, `nt_start`, `nt_cancel` (`noitu.rs` L1245–1313) fall through to empty `answer_callback_query` (L1445) if no room exists.

### Python (`telegram_bot.py`)
1. **Double `query.answer()` (`telegram_bot.py` L1427):**
   - `await query.answer()` is called unconditionally at the top of `handle_callback_query` (L1427). Subsequent `await query.answer(..., show_alert=True)` calls in `nt_getlink` (L1459), `nt_start` (L1470), `nt_cancel` (L1476), `qz_ans:` (L1572), `vtv_hint` (L1606), `tx_bet:` (L1622/1627), `bc_bet:` (L1635/1643), `tp_ans:` (L1725/1728), `tp_5050` (L1775), and `tp_aud` (L1798) are redundant/ineffective in Telegram Bot API once the query is already answered.
2. **Unprotected `query.edit_message_text` / `query.message` on stale/inaccessible messages:**
   - Only `mp_menu_open`/`mp_menu` (L1485–1492) and `mp_list` (L1517–1522, L1549–1564) wrap `edit_message_text` in `try/except` with fallback.
   - All other `query.edit_message_text` / `edit_message_reply_markup` / `query.message.reply_text` calls are unguarded and will raise into `error_handler` (L1856) if the message is `InaccessibleMessage`, deleted, or unmodified.

---

## 3. Pagination Audit

- **No pagination exists in either implementation.**
- **Python (`telegram_bot.py` L1507, L1527):** `db_get_public_rooms(15)` fetches up to 15 rooms and slices `rooms[:10]`.
- **Rust (`noitu.rs` L1344, L1373):** `st.app.db.get_public_rooms().await` + `rooms.iter().take(10)`. Same 3 buttons (`mp_list`, `mp_create`, `mp_menu`), no page offset or Next/Prev callbacks.

---

## 4. Conversation / Wizard State (`MpWizardStep` vs `mp_flow`)

- **Python (`telegram_bot.py`):**
  - Does **not** use `ConversationHandler`. Uses `context.user_data["mp_flow"]` (`L1496`, `L958–998`), scoped per-user globally across all chats.
  - Cleared in `cmd_multiplayer` (L642), `cmd_taophong` (L751), cancel keywords (`L960`), and after `"max"` step (L996).
- **Rust (`state.rs` & `noitu.rs`):**
  - Uses `MpWizardStep` enum (`state.rs` L148–153) stored in `TelegramState::mp_wizard: Arc<DashMap<(i64, i64), MpWizardStep>>` (`state.rs` L170), properly scoped by `(chat_id, user_id)`.
  - Cleared on `/taophong` (`noitu.rs` L749) and when an active Nối Từ game processes a message from that user (`noitu.rs` L1457).
  - **Gap:** `cmd_multiplayer` (`noitu.rs` L738–744) does **not** clear `mp_wizard` (unlike Python L642). Neither Python nor Rust expires abandoned wizard sessions via TTL.

---

## 5. Inline Query Support

- **Not implemented in either codebase.**
