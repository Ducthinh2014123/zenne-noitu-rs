use crate::state::{
    html_escape, user_display_name, user_mention, BauCuaTable, CaDaoSession, ChinhTaSession,
    FlagSession, TaiXiuTable, TelegramState, TrieuPhuQuestion, TrieuPhuSession, VtvSession,
    WordleSession,
};
use rand::seq::SliceRandom;
use rand::Rng;
use std::collections::HashMap;
use std::sync::Arc;
use teloxide::payloads::{AnswerCallbackQuerySetters, EditMessageTextSetters, SendMessageSetters};
use teloxide::prelude::*;
use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, ParseMode};
use tokio::sync::Mutex;
use zenne_games::{baucua, taixiu};

const MONEY_LADDER: [&str; 15] = [
    "200.000",
    "400.000",
    "600.000",
    "1.000.000",
    "2.000.000",
    "3.000.000",
    "6.000.000",
    "10.000.000",
    "14.000.000",
    "22.000.000",
    "30.000.000",
    "40.000.000",
    "60.000.000",
    "85.000.000",
    "150.000.000",
];

const TRIEUPHU_BANK: &[TrieuPhuQuestion] = &[
    TrieuPhuQuestion {
        q: "Thủ đô của Việt Nam là thành phố nào?",
        options: ["TP. Hồ Chí Minh", "Hà Nội", "Đà Nẵng", "Huế"],
        correct: 1,
    },
    TrieuPhuQuestion {
        q: "Đâu là tên một loại bánh truyền thống của Việt Nam trong dịp Tết Nguyên Đán?",
        options: ["Bánh Pizza", "Bánh Chưng", "Bánh Mì Pháp", "Bánh Croissant"],
        correct: 1,
    },
    TrieuPhuQuestion {
        q: "Công thức hóa học của nước tinh khiết là gì?",
        options: ["CO2", "NaCl", "H2O", "O2"],
        correct: 2,
    },
    TrieuPhuQuestion {
        q: "Đỉnh núi nào cao nhất Việt Nam và Đông Dương?",
        options: ["Fansipan", "Bà Đen", "Ngọc Linh", "Langbiang"],
        correct: 0,
    },
    TrieuPhuQuestion {
        q: "Hành tinh nào gần Mặt Trời nhất trong Hệ Mặt Trời?",
        options: ["Sao Kim", "Trái Đất", "Sao Thủy", "Sao Hỏa"],
        correct: 2,
    },
    TrieuPhuQuestion {
        q: "Tác giả của tác phẩm 'Truyện Kiều' là ai?",
        options: ["Nguyễn Trãi", "Nguyễn Du", "Hồ Xuân Hương", "Xuân Diệu"],
        correct: 1,
    },
    TrieuPhuQuestion {
        q: "Đơn vị đo cường độ dòng điện trong hệ SI là gì?",
        options: ["Vôn (V)", "Ôm (Ω)", "Ampe (A)", "Oát (W)"],
        correct: 2,
    },
    TrieuPhuQuestion {
        q: "Chiến thắng Điện Biên Phủ diễn ra vào năm nào?",
        options: ["1945", "1954", "1968", "1975"],
        correct: 1,
    },
    TrieuPhuQuestion {
        q: "Kim loại nào dẫn điện tốt nhất ở điều kiện thường?",
        options: ["Vàng", "Đồng", "Bạc", "Nhôm"],
        correct: 2,
    },
    TrieuPhuQuestion {
        q: "Đại dương nào có diện tích lớn nhất trên Trái Đất?",
        options: ["Đại Tây Dương", "Ấn Độ Dương", "Bắc Băng Dương", "Thái Bình Dương"],
        correct: 3,
    },
    TrieuPhuQuestion {
        q: "Nguyên tố hóa học nào có ký hiệu là Fe?",
        options: ["Sắt", "Đồng", "Kẽm", "Chì"],
        correct: 0,
    },
    TrieuPhuQuestion {
        q: "Con sông nào dài nhất Việt Nam chảy qua lãnh thổ Việt Nam (tính phần hạ lưu)?",
        options: ["Sông Hồng", "Sông Đà", "Sông Mê Kông (Cửu Long)", "Sông Đồng Nai"],
        correct: 3,
    },
    TrieuPhuQuestion {
        q: "Năm 1969, ai là người đầu tiên đặt chân lên Mặt Trăng?",
        options: ["Yuri Gagarin", "Neil Armstrong", "Buzz Aldrin", "Michael Collins"],
        correct: 1,
    },
    TrieuPhuQuestion {
        q: "Ngôn ngữ lập trình Rust phát hành phiên bản 1.0 ổn định vào năm nào?",
        options: ["2012", "2015", "2018", "2020"],
        correct: 1,
    },
    TrieuPhuQuestion {
        q: "Hằng số Planck ký hiệu là gì trong Vật lý lượng tử?",
        options: ["c", "G", "h", "k"],
        correct: 2,
    },
];

const CADAO_CHOICES: &[(&str, &str, [&str; 4])] = &[
    (
        "Công cha như núi Thái Sơn, Nghĩa mẹ như nước trong nguồn ___",
        "chảy ra",
        ["chảy ra", "dâng cao", "tuôn trào", "mát lành"],
    ),
    (
        "Bầu ơi thương lấy bí cùng, Tuy rằng khác giống nhưng chung một ___",
        "giàn",
        ["vườn", "giàn", "nhà", "mái"],
    ),
    (
        "Gần mực thì đen, gần đèn thì ___",
        "sáng",
        ["ấm", "rõ", "sáng", "tỏ"],
    ),
    (
        "Có công mài sắt, có ngày nên ___",
        "kim",
        ["dao", "kiếm", "kim", "vàng"],
    ),
    (
        "Ăn quả nhớ kẻ ___",
        "trồng cây",
        ["tưới nước", "trồng cây", "hái quả", "bón phân"],
    ),
    (
        "Uống nước nhớ ___",
        "nguồn",
        ["suối", "sông", "nguồn", "mưa"],
    ),
    (
        "Đi một ngày đàng, học một sàng ___",
        "khôn",
        ["hay", "khôn", "giỏi", "mới"],
    ),
    (
        "Tốt gỗ hơn tốt ___",
        "nước sơn",
        ["nước sơn", "bề ngoài", "màu sắc", "hình thức"],
    ),
];

const FLAG_CHOICES: &[(&str, &str, [&str; 4])] = &[
    ("🇻🇳", "Việt Nam", ["Việt Nam", "Trung Quốc", "Thái Lan", "Lào"]),
    ("🇯🇵", "Nhật Bản", ["Hàn Quốc", "Nhật Bản", "Đài Loan", "Singapore"]),
    ("🇰🇷", "Hàn Quốc", ["Nhật Bản", "Triều Tiên", "Hàn Quốc", "Mông Cổ"]),
    ("🇺🇸", "Hoa Kỳ (Mỹ)", ["Anh", "Hoa Kỳ (Mỹ)", "Canada", "Úc"]),
    ("🇬🇧", "Vương quốc Anh", ["Úc", "New Zealand", "Vương quốc Anh", "Pháp"]),
    ("🇫🇷", "Pháp", ["Ý", "Pháp", "Hà Lan", "Bỉ"]),
    ("🇩🇪", "Đức", ["Bỉ", "Áo", "Đức", "Thụy Sĩ"]),
    ("🇹🇭", "Thái Lan", ["Thái Lan", "Indonesia", "Malaysia", "Philippines"]),
    ("🇸🇬", "Singapore", ["Malaysia", "Indonesia", "Singapore", "Brunei"]),
    ("🇧🇷", "Brazil", ["Argentina", "Brazil", "Colombia", "Mexico"]),
];

pub async fn cmd_quiz(bot: &Bot, msg: &Message, st: &TelegramState, args: &str) {
    let chat_id = msg.chat.id;
    let parts: Vec<&str> = args.split_whitespace().collect();
    let is_math = parts
        .first()
        .map(|s| matches!(s.to_lowercase().as_str(), "toan" | "math"))
        .unwrap_or(false);

    let q = if is_math {
        st.games.quiz.start_math(chat_id.0.unsigned_abs())
    } else {
        st.games.quiz.start_general(chat_id.0.unsigned_abs())
    };
    st.active_quiz.insert(chat_id.0, q.clone());

    let letters = ["A", "B", "C", "D"];
    let row1 = (0..2)
        .map(|i| {
            InlineKeyboardButton::callback(
                format!("{}. {}", letters[i], q.options[i]),
                format!("qz_ans:{}", i),
            )
        })
        .collect::<Vec<_>>();
    let row2 = (2..4)
        .map(|i| {
            InlineKeyboardButton::callback(
                format!("{}. {}", letters[i], q.options[i]),
                format!("qz_ans:{}", i),
            )
        })
        .collect::<Vec<_>>();
    let kb = InlineKeyboardMarkup::new(vec![
        row1,
        row2,
        vec![InlineKeyboardButton::callback("⏹️ Dừng Lại", "qz_stop")],
    ]);

    let text = format!(
        "🧠 <b>ĐẤU TRÍ TRI THỨC — {}</b>\n\n❓ <b>Câu hỏi:</b> {}\n\n👉 <i>Hãy chọn 1 trong 4 đáp án bên dưới:</i>",
        html_escape(&q.category),
        html_escape(&q.question)
    );
    let _ = bot
        .send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .reply_markup(kb)
        .await;
}

pub async fn cmd_vtv(bot: &Bot, msg: &Message, st: &TelegramState) {
    let chat_id = msg.chat.id;
    let sess = st.games.vuatiengviet.start(chat_id.0.unsigned_abs());
    st.active_vtv.insert(
        chat_id.0,
        VtvSession {
            target: sess.answer.clone(),
            scrambled: sess.scrambled.clone(),
            started_at: std::time::Instant::now(),
        },
    );

    let kb = InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback("💡 Gợi Ý Chữ Đầu", "vtv_hint"),
        InlineKeyboardButton::callback("🏳️ Bỏ Cuộc / Đáp Án", "vtv_giveup"),
    ]]);
    let text = format!(
        "👑 <b>MINI-GAME: VUA TIẾNG VIỆT</b> 👑\n\n🔠 <b>Các ký tự bị xáo trộn:</b>\n👉 <b><code>{}</code></b> 👈\n\n💬 <i>Hãy gõ đáp án đúng vào ô chat! Ai gõ đúng nhanh nhất sẽ giành chiến thắng!</i>",
        html_escape(&sess.scrambled)
    );
    let _ = bot
        .send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .reply_markup(kb)
        .await;
}

pub async fn cmd_taixiu(bot: &Bot, msg: &Message, st: &TelegramState) {
    let chat_id = msg.chat.id;
    if st.active_taixiu.contains_key(&chat_id.0) {
        let _ = bot
            .send_message(
                chat_id,
                "⚠️ Bàn cược Tài Xỉu đang mở! Hãy nhấn nút đặt cược bên dưới.",
            )
            .await;
        return;
    }

    let table = Arc::new(Mutex::new(TaiXiuTable {
        accepting: true,
        bets: HashMap::new(),
        names: HashMap::new(),
    }));
    st.active_taixiu.insert(chat_id.0, table.clone());

    let kb = InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback("🔴 TÀI (11 - 18)", "tx_bet:tai"),
        InlineKeyboardButton::callback("🔵 XỈU (3 - 10)", "tx_bet:xiu"),
    ]]);
    let text = "🎲 <b>BÀN CƯỢC TÀI XỈU DÂN GIAN ĐÃ MỞ!</b> 🎲\n\n\
        • <b>TÀI 🔴:</b> Tổng 3 xúc xắc từ 11 đến 18\n\
        • <b>XỈU 🔵:</b> Tổng 3 xúc xắc từ 3 đến 10\n\
        ⏳ <b>Thời gian đặt cược:</b> 20 giây!\n\n\
        👉 <i>Nhấn nút bên dưới để chọn cửa cược (+50 điểm):</i>";
    let _ = bot
        .send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .reply_markup(kb)
        .await;

    let bot_c = bot.clone();
    let st_c = st.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(20)).await;
        let Some((_, tbl_arc)) = st_c.active_taixiu.remove(&chat_id.0) else {
            return;
        };
        let tbl = tbl_arc.lock().await;
        let r = taixiu::roll_taixiu();
        let result = if r.is_tai { "tai" } else { "xiu" };
        let result_label = if r.is_tai { "TÀI 🔴" } else { "XỈU 🔵" };
        let emojis = ["⚀", "⚁", "⚂", "⚃", "⚄", "⚅"];

        let mut winners = Vec::new();
        for (uid, choice) in &tbl.bets {
            if choice == result {
                let name = tbl
                    .names
                    .get(uid)
                    .cloned()
                    .unwrap_or_else(|| format!("User {}", uid));
                winners.push(html_escape(&name));
                st_c.app.db.game_submit_score("taixiu", *uid, 50).await;
            }
        }
        let win_str = if winners.is_empty() {
            "<i>Không có ai thắng</i>".to_string()
        } else {
            winners.join(", ")
        };
        let d = r.dice;
        let msg_text = format!(
            "🎲 <b>KẾT QUẢ MỞ BÁT TÀI XỈU:</b>\n\n\
            🎲 Xúc xắc: {} ({}) + {} ({}) + {} ({})\n\
            🎯 <b>Tổng điểm:</b> <b>{}</b> ➔ <b>{}</b>\n\n\
            🎉 <b>Người thắng cược (+50đ):</b> {}\n\
            Gõ <code>/taixiu</code> để bắt đầu ván mới!",
            emojis[(d[0] - 1) as usize],
            d[0],
            emojis[(d[1] - 1) as usize],
            d[1],
            emojis[(d[2] - 1) as usize],
            d[2],
            r.total,
            result_label,
            win_str
        );
        let _ = bot_c
            .send_message(chat_id, msg_text)
            .parse_mode(ParseMode::Html)
            .await;
    });
}

fn baucua_info(key: &str) -> (&'static str, &'static str) {
    match key {
        "nai" => ("Nai", "🦌"),
        "bau" => ("Bầu", "🎃"),
        "ga" => ("Gà", "🐓"),
        "ca" => ("Cá", "🐟"),
        "cua" => ("Cua", "🦀"),
        "tom" => ("Tôm", "🦐"),
        _ => ("Bầu", "🎃"),
    }
}

pub async fn cmd_baucua(bot: &Bot, msg: &Message, st: &TelegramState) {
    let chat_id = msg.chat.id;
    if st.active_baucua.contains_key(&chat_id.0) {
        let _ = bot
            .send_message(
                chat_id,
                "⚠️ Bàn cược Bầu Cua đang mở! Hãy nhấn nút đặt con vật bạn thích.",
            )
            .await;
        return;
    }

    let table = Arc::new(Mutex::new(BauCuaTable {
        bets: HashMap::new(),
        names: HashMap::new(),
    }));
    st.active_baucua.insert(chat_id.0, table);

    let kb = InlineKeyboardMarkup::new(vec![
        vec![
            InlineKeyboardButton::callback("🦌 Nai", "bc_bet:nai"),
            InlineKeyboardButton::callback("🎃 Bầu", "bc_bet:bau"),
            InlineKeyboardButton::callback("🐓 Gà", "bc_bet:ga"),
        ],
        vec![
            InlineKeyboardButton::callback("🐟 Cá", "bc_bet:ca"),
            InlineKeyboardButton::callback("🦀 Cua", "bc_bet:cua"),
            InlineKeyboardButton::callback("🦐 Tôm", "bc_bet:tom"),
        ],
    ]);
    let text = "🎋 <b>BÀN CƯỢC BẦU CUA TÔM CÁ DÂN GIAN!</b> 🎋\n\n\
        Cược 20 điểm mỗi lần nhấn vào linh vật may mắn của bạn!\n\
        ⏳ <b>Thời gian đặt cược:</b> 25 giây!\n\n\
        👉 <i>Chọn linh vật bên dưới:</i>";
    let _ = bot
        .send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .reply_markup(kb)
        .await;

    let bot_c = bot.clone();
    let st_c = st.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(25)).await;
        let Some((_, tbl_arc)) = st_c.active_baucua.remove(&chat_id.0) else {
            return;
        };
        let tbl = tbl_arc.lock().await;
        let rolled_faces = baucua::roll_baucua();
        let rolled_keys: Vec<&str> = rolled_faces
            .iter()
            .map(|f| match f {
                baucua::BauCuaFace::Nai => "nai",
                baucua::BauCuaFace::Bau => "bau",
                baucua::BauCuaFace::Ga => "ga",
                baucua::BauCuaFace::Ca => "ca",
                baucua::BauCuaFace::Cua => "cua",
                baucua::BauCuaFace::Tom => "tom",
            })
            .collect();
        let roll_emojis = rolled_keys
            .iter()
            .map(|k| {
                let (name, em) = baucua_info(k);
                format!("{} <b>{}</b>", em, name)
            })
            .collect::<Vec<_>>()
            .join("  •  ");

        let mut summary = Vec::new();
        for (uid, user_bets) in &tbl.bets {
            let p_name = tbl
                .names
                .get(uid)
                .cloned()
                .unwrap_or_else(|| format!("User {}", uid));
            let mut total_won = 0i64;
            for (item_key, amt) in user_bets {
                let hits = rolled_keys.iter().filter(|k| *k == item_key).count() as i64;
                total_won += amt * hits;
            }
            if total_won > 0 {
                st_c.app
                    .db
                    .game_submit_score("baucua", *uid, total_won)
                    .await;
                summary.push(format!("• {}: +{} điểm", html_escape(&p_name), total_won));
            }
        }
        let res_str = if summary.is_empty() {
            "<i>Không ai trúng linh vật nào!</i>".to_string()
        } else {
            summary.join("\n")
        };

        let msg_text = format!(
            "🎋 <b>KẾT QUẢ MỞ BÁT BẦU CUA:</b>\n\n\
            🎲 <b>3 Viên xúc xắc:</b>\n{}\n\n\
            🏆 <b>Thưởng người thắng:</b>\n{}\n\n\
            Gõ <code>/baucua</code> để chơi tiếp ván mới!",
            roll_emojis, res_str
        );
        let _ = bot_c
            .send_message(chat_id, msg_text)
            .parse_mode(ParseMode::Html)
            .await;
    });
}

pub async fn cmd_cadao(bot: &Bot, msg: &Message, st: &TelegramState) {
    let chat_id = msg.chat.id;
    let (prompt, answer, raw_opts) = {
        let mut rng = rand::thread_rng();
        *CADAO_CHOICES.choose(&mut rng).unwrap_or(&CADAO_CHOICES[0])
    };
    let mut opts: Vec<String> = raw_opts.iter().map(|s| s.to_string()).collect();
    opts.shuffle(&mut rand::thread_rng());

    st.active_cadao.insert(
        chat_id.0,
        CaDaoSession {
            prompt: prompt.to_string(),
            answer: answer.to_string(),
            options: opts.clone(),
        },
    );

    let row1 = (0..2)
        .map(|i| InlineKeyboardButton::callback(opts[i].clone(), format!("cd_ans:{}", i)))
        .collect::<Vec<_>>();
    let row2 = (2..4)
        .map(|i| InlineKeyboardButton::callback(opts[i].clone(), format!("cd_ans:{}", i)))
        .collect::<Vec<_>>();
    let text = format!(
        "🌾 <b>CÂU ĐỐ CA DAO - TỤC NGỮ VIỆT NAM</b> 🌾\n\n📜 <i>\"{}\"</i>\n\n👉 <i>Hãy chọn từ thích hợp để điền vào chỗ trống:</i>",
        html_escape(prompt)
    );
    let _ = bot
        .send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .reply_markup(InlineKeyboardMarkup::new(vec![row1, row2]))
        .await;
}

pub async fn cmd_chinhta(bot: &Bot, msg: &Message, st: &TelegramState) {
    let chat_id = msg.chat.id;
    let sess = st.games.chinhta.start(chat_id.0.unsigned_abs());
    st.active_chinhta.insert(
        chat_id.0,
        ChinhTaSession {
            opt_a: sess.opt_a.clone(),
            opt_b: sess.opt_b.clone(),
            correct: sess.correct.clone(),
        },
    );

    let kb = InlineKeyboardMarkup::new(vec![vec![
        InlineKeyboardButton::callback(sess.opt_a.clone(), "ct_ans:0"),
        InlineKeyboardButton::callback(sess.opt_b.clone(), "ct_ans:1"),
    ]]);
    let text = "✍️ <b>THỬ THÁCH CHUẨN HÓA CHÍNH TẢ TIẾNG VIỆT</b>\n\n\
        ❓ <b>Đâu là từ viết ĐÚNG chính tả?</b>\n\n\
        👉 <i>Bấm chọn đáp án đúng:</i>";
    let _ = bot
        .send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .reply_markup(kb)
        .await;
}

pub async fn cmd_flag(bot: &Bot, msg: &Message, st: &TelegramState) {
    let chat_id = msg.chat.id;
    let (flag, correct, raw_opts) = {
        let mut rng = rand::thread_rng();
        *FLAG_CHOICES.choose(&mut rng).unwrap_or(&FLAG_CHOICES[0])
    };
    let mut opts: Vec<String> = raw_opts.iter().map(|s| s.to_string()).collect();
    opts.shuffle(&mut rand::thread_rng());

    st.active_flag.insert(
        chat_id.0,
        FlagSession {
            flag: flag.to_string(),
            correct: correct.to_string(),
            options: opts.clone(),
        },
    );

    let row1 = (0..2)
        .map(|i| InlineKeyboardButton::callback(opts[i].clone(), format!("fq_ans:{}", i)))
        .collect::<Vec<_>>();
    let row2 = (2..4)
        .map(|i| InlineKeyboardButton::callback(opts[i].clone(), format!("fq_ans:{}", i)))
        .collect::<Vec<_>>();
    let text = format!(
        "🌍 <b>ĐỐ VUI QUỐC KỲ THẾ GIỚI</b> 🌍\n\n🏳️ <b>Quốc kỳ:</b>  {}  {}  {}\n\n👉 <i>Đây là quốc kỳ của quốc gia nào?</i>",
        flag, flag, flag
    );
    let _ = bot
        .send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .reply_markup(InlineKeyboardMarkup::new(vec![row1, row2]))
        .await;
}

pub async fn cmd_trieuphu(bot: &Bot, msg: &Message, st: &TelegramState) {
    let chat_id = msg.chat.id;
    let (uid, _) = user_display_name(msg.from.as_ref());
    let q = TRIEUPHU_BANK[0].clone();
    let sess = TrieuPhuSession {
        player_id: uid,
        level: 1,
        lifeline_5050: true,
        lifeline_audience: true,
        current_q: q,
    };
    st.active_trieuphu.insert(chat_id.0, sess.clone());
    send_trieuphu_question(bot, chat_id, &sess).await;
}

async fn send_trieuphu_question(bot: &Bot, chat_id: ChatId, sess: &TrieuPhuSession) {
    let letters = ["A", "B", "C", "D"];
    let q = &sess.current_q;
    let row1 = (0..2)
        .map(|i| {
            InlineKeyboardButton::callback(
                format!("{}. {}", letters[i], q.options[i]),
                format!("tp_ans:{}", i),
            )
        })
        .collect::<Vec<_>>();
    let row2 = (2..4)
        .map(|i| {
            InlineKeyboardButton::callback(
                format!("{}. {}", letters[i], q.options[i]),
                format!("tp_ans:{}", i),
            )
        })
        .collect::<Vec<_>>();

    let mut help_row = Vec::new();
    if sess.lifeline_5050 {
        help_row.push(InlineKeyboardButton::callback("✂️ 50:50", "tp_5050"));
    }
    if sess.lifeline_audience {
        help_row.push(InlineKeyboardButton::callback("👥 Khán Giả", "tp_aud"));
    }
    help_row.push(InlineKeyboardButton::callback(
        "⏹️ Dừng Cuộc Chơi",
        "tp_stop",
    ));

    let ladder_val = MONEY_LADDER[sess.level.saturating_sub(1).min(14)];
    let text = format!(
        "👑 <b>AI LÀ TRIỆU PHÚ — CÂU SỐ {}/15</b>\n💰 <b>Mức tiền thưởng:</b> <code>{} VNĐ</code>\n\n❓ <b>{}</b>\n\n👉 <i>Chọn câu trả lời hoặc sử dụng quyền trợ giúp:</i>",
        sess.level,
        ladder_val,
        html_escape(q.q)
    );
    let _ = bot
        .send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .reply_markup(InlineKeyboardMarkup::new(vec![row1, row2, help_row]))
        .await;
}

pub async fn cmd_wordle(bot: &Bot, msg: &Message, st: &TelegramState) {
    let chat_id = msg.chat.id;
    let target = st.games.wordle.start(chat_id.0.unsigned_abs());
    st.active_wordle.insert(
        chat_id.0,
        WordleSession {
            target,
            attempts: 0,
            max_attempts: 6,
            history: Vec::new(),
        },
    );

    let text = "🟩 <b>CHÀO MỪNG ĐẾN VỚI WORDLE 5 CHỮ CÁI!</b> 🟩\n\n\
        Bot đã chọn 1 từ bí mật gồm <b>5 chữ cái</b> tiếng Anh.\n\
        Bạn có <b>6 lượt đoán</b>.\n\n\
        • 🟩 : Ký tự đúng và đúng vị trí\n\
        • 🟨 : Ký tự có trong từ nhưng sai vị trí\n\
        • ⬛ : Ký tự không có trong từ\n\n\
        👉 <i>Nhập 1 từ 5 chữ cái vào ô chat để đoán! (Ví dụ: APPLE, HOUSE, LIGHT)</i>";
    let _ = bot
        .send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .await;
}

pub async fn cmd_mygame(bot: &Bot, msg: &Message, st: &TelegramState) {
    let chat_id = msg.chat.id;
    let (uid, uname) = user_display_name(msg.from.as_ref());
    let games = [
        "noitu",
        "quiz",
        "vuatiengviet",
        "taixiu",
        "baucua",
        "cadao",
        "chinhta",
        "trieuphu",
    ];
    let mut lines = Vec::new();
    let mut total_pts = 0i64;
    for g in games {
        let (best, plays) = st.app.db.game_user_score(g, uid).await;
        total_pts += best;
        lines.push(format!(
            "• <b>{}:</b> {} điểm ({} lượt chơi)",
            g.to_uppercase(),
            best,
            plays
        ));
    }
    let text = format!(
        "📊 <b>KỶ LỤC CÁ NHÂN: {}</b>\n\n{}\n\n🌟 <b>Tổng điểm tích lũy:</b> <b>{}</b> điểm!",
        user_mention(uid, &uname),
        lines.join("\n"),
        total_pts
    );
    let _ = bot
        .send_message(chat_id, text)
        .parse_mode(ParseMode::Html)
        .await;
}

pub async fn handle_minigame_callback(
    bot: &Bot,
    q: &CallbackQuery,
    st: &TelegramState,
    data: &str,
) -> bool {
    let Some(ref msg) = q.message else {
        return false;
    };
    let chat_id = msg.chat().id;
    let msg_id = msg.id();
    let uid = q.from.id.0 as i64;
    let uname = q.from.first_name.clone();

    if let Some(idx_str) = data.strip_prefix("qz_ans:") {
        let _ = bot.answer_callback_query(q.id.clone()).await;
        let idx: usize = idx_str.parse().unwrap_or(0);
        let Some((_, q_data)) = st.active_quiz.remove(&chat_id.0) else {
            return true;
        };
        if idx == q_data.correct_index {
            st.app.db.game_submit_score("quiz", uid, 10).await;
            let text = format!(
                "🎉 <b>CHÍNH XÁC!</b> {} đã chọn đáp án đúng (+10đ)!\n\nGõ <code>/quiz</code> để chơi tiếp câu hỏi mới!",
                user_mention(uid, &uname)
            );
            let _ = bot
                .edit_message_text(chat_id, msg_id, text)
                .parse_mode(ParseMode::Html)
                .await;
        } else {
            let correct_opt = q_data
                .options
                .get(q_data.correct_index)
                .map(|s| s.as_str())
                .unwrap_or("");
            let text = format!(
                "❌ <b>RẤT TIẾC, ĐÁP ÁN CHƯA ĐÚNG!</b>\n\n✅ Đáp án chính xác là: <b>{}</b>\n\nGõ <code>/quiz</code> để thử lại câu khác nhé!",
                html_escape(correct_opt)
            );
            let _ = bot
                .edit_message_text(chat_id, msg_id, text)
                .parse_mode(ParseMode::Html)
                .await;
        }
        return true;
    }
    if data == "qz_stop" {
        let _ = bot.answer_callback_query(q.id.clone()).await;
        st.active_quiz.remove(&chat_id.0);
        let _ = bot
            .edit_message_text(
                chat_id,
                msg_id,
                "⏹️ <b>Đã dừng câu hỏi trắc nghiệm!</b>",
            )
            .parse_mode(ParseMode::Html)
            .await;
        return true;
    }

    if data == "vtv_hint" {
        if let Some(vtv) = st.active_vtv.get(&chat_id.0) {
            let first_c = vtv.target.chars().next().unwrap_or('?');
            let _ = bot
                .answer_callback_query(q.id.clone())
                .text(format!(
                    "💡 Gợi ý: Từ bắt đầu bằng chữ '{}'",
                    first_c.to_uppercase()
                ))
                .show_alert(true)
                .await;
        } else {
            let _ = bot.answer_callback_query(q.id.clone()).await;
        }
        return true;
    }
    if data == "vtv_giveup" {
        let _ = bot.answer_callback_query(q.id.clone()).await;
        if let Some((_, vtv)) = st.active_vtv.remove(&chat_id.0) {
            let text = format!(
                "🏳️ <b>BỎ CUỘC!</b>\n\nĐáp án chính xác là: <b><code>{}</code></b>!\nGõ <code>/vtv</code> để thử từ khác!",
                html_escape(&vtv.target)
            );
            let _ = bot
                .edit_message_text(chat_id, msg_id, text)
                .parse_mode(ParseMode::Html)
                .await;
        }
        return true;
    }

    if let Some(choice) = data.strip_prefix("tx_bet:") {
        if let Some(tbl_arc) = st.active_taixiu.get(&chat_id.0) {
            let mut tbl = tbl_arc.lock().await;
            if tbl.accepting {
                tbl.bets.insert(uid, choice.to_string());
                tbl.names.insert(uid, uname);
                let label = if choice == "tai" {
                    "TÀI 🔴"
                } else {
                    "XỈU 🔵"
                };
                let _ = bot
                    .answer_callback_query(q.id.clone())
                    .text(format!("✅ Đã đặt cược cửa {}!", label))
                    .show_alert(true)
                    .await;
                return true;
            }
        }
        let _ = bot
            .answer_callback_query(q.id.clone())
            .text("⚠️ Bàn cược đã đóng!")
            .show_alert(true)
            .await;
        return true;
    }

    if let Some(item_key) = data.strip_prefix("bc_bet:") {
        if let Some(tbl_arc) = st.active_baucua.get(&chat_id.0) {
            let mut tbl = tbl_arc.lock().await;
            let user_bets = tbl.bets.entry(uid).or_default();
            let curr = user_bets.entry(item_key.to_string()).or_insert(0);
            *curr += 20;
            let new_amt = *curr;
            tbl.names.insert(uid, uname);
            let (name, em) = baucua_info(item_key);
            let _ = bot
                .answer_callback_query(q.id.clone())
                .text(format!(
                    "✅ Đã đặt thêm 20đ vào {} {} (Tổng: {}đ)",
                    em, name, new_amt
                ))
                .show_alert(true)
                .await;
            return true;
        }
        let _ = bot
            .answer_callback_query(q.id.clone())
            .text("⚠️ Bàn cược đã đóng!")
            .show_alert(true)
            .await;
        return true;
    }

    if let Some(idx_str) = data.strip_prefix("cd_ans:") {
        let _ = bot.answer_callback_query(q.id.clone()).await;
        let idx: usize = idx_str.parse().unwrap_or(0);
        if let Some((_, cd)) = st.active_cadao.remove(&chat_id.0) {
            let selected = cd.options.get(idx).map(|s| s.as_str()).unwrap_or("");
            let full = cd.prompt.replace("___", &cd.answer);
            if selected == cd.answer {
                st.app.db.game_submit_score("cadao", uid, 15).await;
                let text = format!(
                    "🎉 <b>CHÍNH XÁC!</b> {} (+15 điểm)\n\n📜 <b>{}</b>\n\nGõ <code>/cadao</code> để tiếp tục thử sức!",
                    user_mention(uid, &uname),
                    html_escape(&full)
                );
                let _ = bot
                    .edit_message_text(chat_id, msg_id, text)
                    .parse_mode(ParseMode::Html)
                    .await;
            } else {
                let text = format!(
                    "❌ <b>CHƯA ĐÚNG!</b>\n\n📜 Câu đầy đủ: <b>{}</b>\n\nGõ <code>/cadao</code> để thử lại câu khác!",
                    html_escape(&full)
                );
                let _ = bot
                    .edit_message_text(chat_id, msg_id, text)
                    .parse_mode(ParseMode::Html)
                    .await;
            }
        }
        return true;
    }

    if let Some(idx_str) = data.strip_prefix("ct_ans:") {
        let _ = bot.answer_callback_query(q.id.clone()).await;
        let idx: usize = idx_str.parse().unwrap_or(0);
        if let Some((_, ct)) = st.active_chinhta.remove(&chat_id.0) {
            let selected = if idx == 0 { &ct.opt_a } else { &ct.opt_b };
            if selected.eq_ignore_ascii_case(&ct.correct) || selected.to_lowercase() == ct.correct.to_lowercase() {
                st.app.db.game_submit_score("chinhta", uid, 15).await;
                let text = format!(
                    "🎉 <b>HOÀN TOÀN CHÍNH XÁC!</b> (+15đ)\n\n✅ <b>{}</b> là cách viết chuẩn xác.\n\nGõ <code>/chinhta</code> để làm câu tiếp theo!",
                    html_escape(&ct.correct)
                );
                let _ = bot
                    .edit_message_text(chat_id, msg_id, text)
                    .parse_mode(ParseMode::Html)
                    .await;
            } else {
                let text = format!(
                    "❌ <b>TIẾC QUÁ, BẠN CHỌN SAI RỒI!</b>\n\n✅ Cách viết ĐÚNG là: <b>{}</b>\n\nGõ <code>/chinhta</code> để thử lại!",
                    html_escape(&ct.correct)
                );
                let _ = bot
                    .edit_message_text(chat_id, msg_id, text)
                    .parse_mode(ParseMode::Html)
                    .await;
            }
        }
        return true;
    }

    if let Some(idx_str) = data.strip_prefix("fq_ans:") {
        let _ = bot.answer_callback_query(q.id.clone()).await;
        let idx: usize = idx_str.parse().unwrap_or(0);
        if let Some((_, fq)) = st.active_flag.remove(&chat_id.0) {
            let selected = fq.options.get(idx).map(|s| s.as_str()).unwrap_or("");
            if selected == fq.correct {
                st.app.db.game_submit_score("flagquiz", uid, 15).await;
                let text = format!(
                    "🎉 <b>CHÚC MỪNG BẠN ĐÃ ĐOÁN ĐÚNG!</b> (+15đ)\n\n🏳️ <b>Quốc gia:</b> {} {}\n\nGõ <code>/flag</code> để tiếp tục khám phá thế giới!",
                    html_escape(&fq.correct),
                    fq.flag
                );
                let _ = bot
                    .edit_message_text(chat_id, msg_id, text)
                    .parse_mode(ParseMode::Html)
                    .await;
            } else {
                let text = format!(
                    "❌ <b>CHƯA CHÍNH XÁC!</b>\n\n🏳️ Quốc kỳ này thuộc về: <b>{} {}</b>\nGõ <code>/flag</code> để thử lại!",
                    html_escape(&fq.correct),
                    fq.flag
                );
                let _ = bot
                    .edit_message_text(chat_id, msg_id, text)
                    .parse_mode(ParseMode::Html)
                    .await;
            }
        }
        return true;
    }

    if let Some(idx_str) = data.strip_prefix("tp_ans:") {
        let idx: usize = idx_str.parse().unwrap_or(0);
        let Some(mut tp_ref) = st.active_trieuphu.get_mut(&chat_id.0) else {
            let _ = bot
                .answer_callback_query(q.id.clone())
                .text("Ván chơi đã kết thúc!")
                .show_alert(true)
                .await;
            return true;
        };
        if uid != tp_ref.player_id {
            let _ = bot
                .answer_callback_query(q.id.clone())
                .text("⛔ Chỉ người tham gia thi đấu mới được chọn đáp án!")
                .show_alert(true)
                .await;
            return true;
        }
        let _ = bot.answer_callback_query(q.id.clone()).await;

        if idx == tp_ref.current_q.correct {
            let level = tp_ref.level;
            let prize = MONEY_LADDER[level.saturating_sub(1).min(14)];
            if level >= 15 {
                drop(tp_ref);
                st.active_trieuphu.remove(&chat_id.0);
                st.app.db.game_submit_score("trieuphu", uid, 150_000_000).await;
                let _ = bot
                    .edit_message_text(
                        chat_id,
                        msg_id,
                        "👑 <b>XUẤT SẮC! BẠN LÀ QUÁN QUÂN TRIỆU PHÚ!</b> 👑\n\nBạn đã vượt qua câu 15 và ẵm trọn <b>150.000.000 VNĐ</b> tiền thưởng!",
                    )
                    .parse_mode(ParseMode::Html)
                    .await;
            } else {
                tp_ref.level = level + 1;
                tp_ref.current_q = TRIEUPHU_BANK[level.min(TRIEUPHU_BANK.len() - 1)].clone();
                let next_sess = tp_ref.clone();
                drop(tp_ref);

                let text = format!(
                    "🎉 <b>CHÍNH XÁC!</b> Bạn đã vượt qua câu {}!\n💰 Mức tiền thưởng hiện tại: <b>{} VNĐ</b>!\n\n⏳ <i>Đang chuẩn bị câu hỏi số {}...</i>",
                    level, prize, level + 1
                );
                let _ = bot
                    .edit_message_text(chat_id, msg_id, text)
                    .parse_mode(ParseMode::Html)
                    .await;
                let bot_c = bot.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    send_trieuphu_question(&bot_c, chat_id, &next_sess).await;
                });
            }
        } else {
            let level = tp_ref.level;
            let correct_opt = tp_ref.current_q.options[tp_ref.current_q.correct];
            drop(tp_ref);
            st.active_trieuphu.remove(&chat_id.0);
            let safe_prize: i64 = if level > 10 {
                22_000_000
            } else if level > 5 {
                2_000_000
            } else {
                0
            };
            st.app.db.game_submit_score("trieuphu", uid, safe_prize).await;
            let text = format!(
                "❌ <b>RẤT TIẾC, CÂU TRẢ LỜI KHÔNG CHÍNH XÁC!</b>\n\n✅ Đáp án đúng là: <b>{}</b>\n💰 Bạn ra về với mức thưởng an toàn: <b>{} VNĐ</b>!\nGõ <code>/trieuphu</code> để bắt đầu lại nhé!",
                html_escape(correct_opt),
                safe_prize
            );
            let _ = bot
                .edit_message_text(chat_id, msg_id, text)
                .parse_mode(ParseMode::Html)
                .await;
        }
        return true;
    }

    if data == "tp_5050" {
        if let Some(mut tp) = st.active_trieuphu.get_mut(&chat_id.0) {
            if tp.lifeline_5050 {
                tp.lifeline_5050 = false;
                let correct = tp.current_q.correct;
                let mut wrong: Vec<usize> = (0..4).filter(|i| *i != correct).collect();
                wrong.shuffle(&mut rand::thread_rng());
                let removed = [wrong[0], wrong[1]];
                let letters = ["A", "B", "C", "D"];
                let opts = tp.current_q.options;
                let row1 = (0..2)
                    .map(|i| {
                        let lbl = if removed.contains(&i) {
                            "❌".to_string()
                        } else {
                            format!("{}. {}", letters[i], opts[i])
                        };
                        InlineKeyboardButton::callback(lbl, format!("tp_ans:{}", i))
                    })
                    .collect::<Vec<_>>();
                let row2 = (2..4)
                    .map(|i| {
                        let lbl = if removed.contains(&i) {
                            "❌".to_string()
                        } else {
                            format!("{}. {}", letters[i], opts[i])
                        };
                        InlineKeyboardButton::callback(lbl, format!("tp_ans:{}", i))
                    })
                    .collect::<Vec<_>>();
                let mut help_row = Vec::new();
                if tp.lifeline_audience {
                    help_row.push(InlineKeyboardButton::callback("👥 Khán Giả", "tp_aud"));
                }
                help_row.push(InlineKeyboardButton::callback(
                    "⏹️ Dừng Cuộc Chơi",
                    "tp_stop",
                ));
                let _ = bot
                    .answer_callback_query(q.id.clone())
                    .text("✂️ Đã loại bỏ 2 phương án sai!")
                    .show_alert(true)
                    .await;
                let _ = bot
                    .edit_message_reply_markup(chat_id, msg_id)
                    .reply_markup(InlineKeyboardMarkup::new(vec![row1, row2, help_row]))
                    .await;
                return true;
            }
        }
        let _ = bot.answer_callback_query(q.id.clone()).await;
        return true;
    }

    if data == "tp_aud" {
        if let Some(mut tp) = st.active_trieuphu.get_mut(&chat_id.0) {
            if tp.lifeline_audience {
                tp.lifeline_audience = false;
                let correct = tp.current_q.correct;
                let poll_str = {
                    let mut rng = rand::thread_rng();
                    let mut poll = [0u32; 4];
                    for p in &mut poll {
                        *p = rng.gen_range(5..=15);
                    }
                    poll[correct] += rng.gen_range(35..=55);
                    let total: u32 = poll.iter().sum();
                    let letters = ["A", "B", "C", "D"];
                    (0..4)
                        .map(|i| format!("{}: {}%", letters[i], (poll[i] * 100) / total))
                        .collect::<Vec<_>>()
                        .join(" | ")
                };
                let _ = bot
                    .answer_callback_query(q.id.clone())
                    .text(format!("👥 Ý kiến khán giả:\n{}", poll_str))
                    .show_alert(true)
                    .await;
                return true;
            }
        }
        let _ = bot.answer_callback_query(q.id.clone()).await;
        return true;
    }

    if data == "tp_stop" {
        let _ = bot.answer_callback_query(q.id.clone()).await;
        if let Some((_, tp)) = st.active_trieuphu.remove(&chat_id.0) {
            let prize = if tp.level > 1 {
                MONEY_LADDER[(tp.level - 2).min(14)]
            } else {
                "0"
            };
            let text = format!(
                "⏹️ <b>BẠN ĐÃ DỪNG CUỘC CHƠI AN TOÀN!</b>\n\n💰 Bạn bảo toàn số tiền thưởng: <b>{} VNĐ</b>!\nGõ <code>/trieuphu</code> để quay lại thi đấu!",
                prize
            );
            let _ = bot
                .edit_message_text(chat_id, msg_id, text)
                .parse_mode(ParseMode::Html)
                .await;
        }
        return true;
    }

    false
}
