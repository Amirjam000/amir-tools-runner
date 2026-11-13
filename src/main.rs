//! ============================================================================
//! 🦀 AMIR TOOLS ULTIMATE BOT — 100% NATIVE RUST EDITION (v6.0)
//! ============================================================================
//! Complete high-speed Rust port of `host_bot_engine.py` with:
//! - Real YouTube / Instagram / Music downloader powered by `yt-dlp` + `ffmpeg`
//! - Autonomous Web File Hunter & Search Agent (`agent.rs`) that crawls search
//!   engines & landing pages to extract, download, and upload files to Telegram
//! - Live Currency, Gold, Coin & Crypto prices + smart converter
//! - Multi-Model AI Chat (with memory), 18-Model Flux/SDXL Image Generator,
//!   Neural TTS Voice Engine, FFmpeg Video Compressor, Visual Podcast Studio
//! - QR Generator, Open-Meteo Weather, Ping/DNS/Port/IP/Wiki, Math, Jalali Date,
//!   Abjad, Fal Hafez, Jokes, Riddles, Games, Anonymous Chat, Group Anti-Spam,
//!   and Full Owner Admin Panel (`/panel`, `/sh`, `/dl`, `/ghostban`, `/broadcast`).
//! ============================================================================

mod agent;
mod config;
mod db;
mod tg;
mod tools;
mod ytdlp;

use anyhow::Result;
use config::Config;
use db::Store;
use serde_json::json;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tg::{
    html_escape, truncate_str, CallbackQuery, InlineBtn, Keyboard, Message, TgClient, EFFECT_FIRE,
    EFFECT_HEART, EFFECT_PARTY,
};
use tokio::sync::Semaphore;

pub struct App {
    pub tg: TgClient,
    pub cfg: Config,
    pub store: Store,
    pub started_at: Instant,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cfg = Config::load()?;
    let store = Store::open(&cfg.data_dir, cfg.group_char_limit)?;
    let tg = TgClient::new(&cfg.tg_api_base, &cfg.bot_token)?;

    println!("==========================================================");
    println!("🦀 Amir Tools Ultimate Rust Bot v6.0 Starting...");
    println!(
        "OS: {} ({}) | Workers: {} | Upload Limit: {} MB",
        std::env::consts::OS,
        std::env::consts::ARCH,
        cfg.workers,
        cfg.max_upload_mb
    );

    if let Ok(me) = tg.get_me().await {
        println!(
            "✅ Connected as @{} (ID: {}, Name: {})",
            me.username.as_deref().unwrap_or(&cfg.bot_username),
            me.id,
            me.first_name
        );
    }
    println!("==========================================================");

    // Pre-check / auto-bootstrap yt-dlp in the background so the first YouTube download is instant
    {
        let cfg_bg = cfg.clone();
        let http_bg = tg.http.clone();
        tokio::spawn(async move {
            let bin = ytdlp::resolve_ytdlp_bin(&cfg_bg, &http_bg).await;
            println!("🎬 yt-dlp engine ready at: {}", bin);
        });
    }

    // Optional heartbeat to Cloudflare Worker status dashboard (does NOT trigger any GitHub Actions)
    if !cfg.cf_worker_url.is_empty() {
        let http_hb = tg.http.clone();
        let cf_url = format!("{}/api/heartbeat", cfg.cf_worker_url);
        let started = Instant::now();
        tokio::spawn(async move {
            loop {
                let payload = json!({
                    "host_id": "rust-server-v6",
                    "status": "active",
                    "uptime_sec": started.elapsed().as_secs(),
                    "cpu": 0.5,
                    "ram_mb": 6.4,
                    "details": "Amir Tools Native Rust v6.0 Online (yt-dlp + File Hunter Agent)"
                });
                let _ = http_hb
                    .post(&cf_url)
                    .json(&payload)
                    .timeout(Duration::from_secs(10))
                    .send()
                    .await;
                tokio::time::sleep(Duration::from_secs(45)).await;
            }
        });
    }

    let app = Arc::new(App {
        tg,
        cfg: cfg.clone(),
        store,
        started_at: Instant::now(),
    });

    let sem = Arc::new(Semaphore::new(cfg.workers));
    let mut offset: i64 = 0;

    loop {
        match app.tg.get_updates(offset, 25).await {
            Ok(updates) => {
                for u in updates {
                    offset = u.update_id + 1;
                    let Ok(permit) = sem.clone().acquire_owned().await else {
                        continue;
                    };
                    let app_cl = app.clone();
                    tokio::spawn(async move {
                        let _p = permit;
                        if let Some(msg) = u.message {
                            if let Err(e) = handle_message(&app_cl, msg).await {
                                eprintln!("[msg error] {:#}", e);
                            }
                        } else if let Some(cb) = u.callback_query {
                            if let Err(e) = handle_callback(&app_cl, cb).await {
                                eprintln!("[cb error] {:#}", e);
                            }
                        }
                    });
                }
            }
            Err(e) => {
                eprintln!("⚠️ getUpdates error: {} (retrying in 3s)", e);
                tokio::time::sleep(Duration::from_secs(3)).await;
            }
        }
    }
}

// ============================================================================
// KEYBOARDS & MENUS (AMIR TOOLS PERSONA)
// ============================================================================

fn home_keyboard(is_owner: bool) -> Keyboard {
    let mut kb = vec![
        vec![
            InlineBtn::cb("🎬 دانلود یوتیوب و موزیک (yt-dlp)", "menu:dl"),
            InlineBtn::cb("🕵️ ایجنت شکارچی فایل وب", "menu:agent"),
        ],
        vec![
            InlineBtn::cb("💹 نرخ زنده طلا، ارز و رمزارز", "menu:market"),
            InlineBtn::cb("🧠 هوش مصنوعی و تصویرساز", "menu:ai"),
        ],
        vec![
            InlineBtn::cb("🎙 تبدیل متن به ویس (TTS)", "menu:tts"),
            InlineBtn::cb("🗜 کم‌حجم کردن ویدیو و پادکست", "menu:media"),
        ],
        vec![
            InlineBtn::cb("🛠 جعبه ابزار کاربردی و شبکه", "menu:tools"),
            InlineBtn::cb("🎭 فال حافظ، سرگرمی و بازی", "menu:fun"),
        ],
        vec![
            InlineBtn::cb("🛡 مدیریت گروه و ضد اسپم", "menu:group"),
            InlineBtn::cb("💌 لینک چت ناشناس من", "menu:anon"),
        ],
    ];
    if is_owner {
        kb.push(vec![InlineBtn::cb(
            "👑 پنل مدیریت کل (مخصوص ادمین)",
            "menu:admin",
        )]);
    }
    kb
}

fn back_home_kb() -> Keyboard {
    vec![vec![InlineBtn::cb("🏠 بازگشت به منوی اصلی", "menu:home")]]
}

fn welcome_text(app: &App, first_name: &str) -> String {
    let up = app.started_at.elapsed().as_secs();
    let up_str = format!("{}h {}m {}s", up / 3600, (up % 3600) / 60, up % 60);
    format!(
        "👋 <b>سلام {} عزیز! به ربات همه‌فن‌حریف امیر تولز (نسخه فوق‌سریع Rust 🦀 v6.0) خوش آمدید!</b>\n\n\
         ⚡ <b>هسته اجرایی:</b> <code>100% Native Rust + Tokio Async</code>\n\
         ⏱ <b>آپ‌تایم سرور:</b> <code>{}</code>\n\n\
         🔥 <b>امکانات ویژه نسخه جدید:</b>\n\
         • 🎬 <b>دانلودر مستقیم یوتیوب و اینستاگرام با <code>yt-dlp</code>:</b> ارسال لینک یا <code>/yt نام ویدیو</code> (ویدیو 720p، 360p و موزیک MP3)\n\
         • 🕵️ <b>ایجنت هوشمند شکارچی فایل از وب:</b> <code>/findfile نام کتاب یا برنامه یا آهنگ</code> (جستجو در سایت‌ها، استخراج لینک و ارسال مستقیم فایل در تلگرام!)\n\
         • 💹 <b>نرخ لحظه‌ای طلا، سکه، دلار و رمزارز:</b> دستور <code>/price</code> یا نوشتن <code>100 دلار</code>، <code>2 بیت کوین</code>\n\
         • 🎨 <b>تصویرساز ۱۸ مدله Flux/SDXL:</b> دستور <code>/draw منظره</code> یا <code>عکس ...</code>\n\
         • 🎙 <b>صداپیشه عصبی (فرید و دل‌آرا):</b> دستور <code>/tts متن</code> یا <code>صدا ...</code>\n\
         • 🗜 <b>کم‌حجم‌کننده ویدیو و پادکست‌ساز:</b> ریپلای <code>کم حجم کن</code> روی هر ویدیو یا <code>/podcast موضوع</code>\n\
         • 🛠 <b>ابزارهای مهندسی:</b> هواشناسی زنده، بارکد QR، پینگ، DNS، اسکن پورت، ویکی‌پدیا، ابجد، ماشین‌حساب و تقویم شمسی\n\n\
         👇 <i>از دکمه‌های شیشه‌ای زیر استفاده کنید یا مستقیماً سوال، لینک یا نام فایل خود را بفرستید:</i>",
        html_escape(first_name),
        up_str
    )
}

// ============================================================================
// GROUP ANTI-SPAM & MODERATION GUARD
// ============================================================================

async fn check_group_guard(app: &App, msg: &Message) -> Result<bool> {
    let chat_id = msg.chat.id;
    let title = msg.chat.title.as_deref().unwrap_or("");
    let gs = app.store.group_settings(chat_id, title).await;

    // Welcome new members
    if let Some(ref members) = msg.new_chat_members {
        if gs.welcome_enabled {
            for m in members {
                if !m.is_bot {
                    let _ = app
                        .tg
                        .send_html(
                            chat_id,
                            Some(msg.message_id),
                            &format!(
                                "🌺 خوش آمدید <b>{}</b> عزیز به گروه <b>{}</b>!\nلطفاً قوانین گروه را رعایت فرمایید.",
                                html_escape(&m.first_name),
                                html_escape(title)
                            ),
                            None,
                        )
                        .await;
                }
            }
        }
        return Ok(false);
    }

    let Some(ref user) = msg.from else {
        return Ok(false);
    };
    let user_id = user.id;

    // Owner and group admins bypass all locks and character limits
    if user_id == app.cfg.owner_id || app.tg.is_admin(chat_id, user_id).await {
        return Ok(false);
    }

    let body = msg.body();

    // 1. Character limit for non-admins (default 100 chars, owner/admins exempt)
    if gs.char_limit > 0 && body.chars().count() > gs.char_limit {
        app.tg.delete(chat_id, msg.message_id).await;
        return Ok(true);
    }

    // 2. Forward lock
    if gs.lock_forwards && msg.is_forwarded() {
        app.tg.delete(chat_id, msg.message_id).await;
        return Ok(true);
    }

    // 3. Link lock
    if gs.lock_links {
        let lower = body.to_lowercase();
        if lower.contains("http://")
            || lower.contains("https://")
            || lower.contains("t.me/")
            || lower.contains("telegram.me/")
            || lower.contains("www.")
        {
            app.tg.delete(chat_id, msg.message_id).await;
            return Ok(true);
        }
    }

    // 4. Media lock
    if gs.lock_media && msg.has_media() {
        app.tg.delete(chat_id, msg.message_id).await;
        return Ok(true);
    }

    Ok(false)
}

fn group_panel_kb(gs: &db::GroupSettings) -> Keyboard {
    let b = |v: bool| if v { "🔒 فعال" } else { "🔓 غیرفعال" };
    let cl = if gs.char_limit == 0 {
        "بدون محدودیت".to_string()
    } else {
        format!("{} کاراکتر", gs.char_limit)
    };
    vec![
        vec![
            InlineBtn::cb(format!("قفل لینک: {}", b(gs.lock_links)), "grp:toggle_links"),
            InlineBtn::cb(
                format!("قفل فروارد: {}", b(gs.lock_forwards)),
                "grp:toggle_fwds",
            ),
        ],
        vec![
            InlineBtn::cb(format!("قفل مدیا: {}", b(gs.lock_media)), "grp:toggle_media"),
            InlineBtn::cb(
                format!("خوش‌آمدگو: {}", b(gs.welcome_enabled)),
                "grp:toggle_welcome",
            ),
        ],
        vec![InlineBtn::cb(
            format!("✂️ سقف کاراکتر اعضا: {}", cl),
            "grp:cycle_charlimit",
        )],
    ]
}

// ============================================================================
// MESSAGE HANDLER
// ============================================================================

async fn handle_message(app: &App, msg: Message) -> Result<()> {
    let is_group = msg.chat.kind == "group" || msg.chat.kind == "supergroup";
    if is_group && check_group_guard(app, &msg).await? {
        return Ok(());
    }

    let Some(user) = msg.from.clone() else {
        return Ok(());
    };
    if user.is_bot {
        return Ok(());
    }

    let chat_id = msg.chat.id;
    let user_id = user.id;
    let is_owner = user_id == app.cfg.owner_id;

    let (banned, ghostbanned, user_state) = app
        .store
        .touch_user(user_id, &user.first_name, user.username.as_deref())
        .await;

    if !is_owner {
        if ghostbanned {
            // Silent ignore for ghostbanned users
            return Ok(());
        }
        if banned {
            let _ = app
                .tg
                .send_html(
                    chat_id,
                    Some(msg.message_id),
                    "⛔ حساب کاربری شما توسط مدیریت ربات مسدود شده است.",
                    None,
                )
                .await;
            return Ok(());
        }
    }

    let raw_text = msg.body().trim().to_string();

    // If user sends a video directly in private chat or with caption "کم حجم کن", offer FFmpeg compression!
    if let Some(ref vid) = msg.video {
        if !is_group || raw_text.contains("کم حجم") || raw_text.starts_with("/compress") {
            let fid_key = app.store.stash_url(&vid.file_id).await;
            let kb: Keyboard = vec![
                vec![
                    InlineBtn::cb("🔥 فشرده‌سازی شدید (CRF 34)", format!("vcomp:34:{}", fid_key)),
                    InlineBtn::cb("⚖️ متعادل (CRF 28)", format!("vcomp:28:{}", fid_key)),
                ],
                vec![InlineBtn::cb("💎 کیفیت بالا (CRF 24)", format!("vcomp:24:{}", fid_key))],
            ];
            app.tg
                .send_html(
                    chat_id,
                    Some(msg.message_id),
                    "🎬 <b>ویدیو دریافت شد!</b>\n👇 میزان فشرده‌سازی و کاهش حجم با <code>FFmpeg</code> را انتخاب کنید:",
                    Some(kb),
                )
                .await?;
            return Ok(());
        }
    }

    if raw_text.is_empty() {
        return Ok(());
    }

    // React with ⚡ on private messages or direct commands
    if !is_group || raw_text.starts_with('/') {
        app.tg.set_reaction(chat_id, msg.message_id, "⚡").await;
    }

    let text = raw_text
        .replace("@Game_iran_amir_bot", "")
        .replace("@amir_tools_bot", "")
        .trim()
        .to_string();

    // Handle active user state in private chat if text is not a slash command
    if !is_group && !text.starts_with('/') {
        if let Some(ref st) = user_state {
            app.store.set_state(user_id, None).await;
            if let Some(target_str) = st.strip_prefix("anon_send:") {
                if let Ok(target_uid) = target_str.parse::<i64>() {
                    let sender_tok = app.store.get_or_create_anon_token(user_id).await;
                    let kb: Keyboard = vec![vec![InlineBtn::cb(
                        "✍️ پاسخ ناشناس به این پیام",
                        format!("anon_reply:{}", sender_tok),
                    )]];
                    let _ = app
                        .tg
                        .send_html(
                            target_uid,
                            None,
                            &format!(
                                "💌 <b>یک پیام ناشناس جدید دریافت کردید:</b>\n\n«{}»",
                                html_escape(&text)
                            ),
                            Some(kb),
                        )
                        .await;
                    app.tg
                        .send_html(
                            chat_id,
                            Some(msg.message_id),
                            "✅ <b>پیام ناشناس شما با موفقیت ارسال شد!</b>",
                            Some(back_home_kb()),
                        )
                        .await?;
                    return Ok(());
                }
            }
            match st.as_str() {
                "wait_yt" => {
                    ytdlp::present_card(&app.tg, &app.cfg, &app.store, chat_id, msg.message_id, &text).await?;
                    return Ok(());
                }
                "wait_agent" => {
                    agent::run_file_hunter_agent(
                        &app.tg,
                        &app.cfg,
                        &app.store,
                        chat_id,
                        user_id,
                        msg.message_id,
                        &text,
                    )
                    .await?;
                    return Ok(());
                }
                "wait_draw" => {
                    tools::generate_and_send_image(
                        &app.tg,
                        &app.store,
                        chat_id,
                        user_id,
                        msg.message_id,
                        &text,
                    )
                    .await?;
                    return Ok(());
                }
                "wait_tts" => {
                    tools::synthesize_and_send_tts(
                        &app.tg,
                        &app.cfg,
                        &app.store,
                        chat_id,
                        user_id,
                        msg.message_id,
                        &text,
                    )
                    .await?;
                    return Ok(());
                }
                "wait_podcast" => {
                    tools::create_visual_podcast(&app.tg, &app.cfg, chat_id, msg.message_id, &text)
                        .await?;
                    return Ok(());
                }
                "wait_qr" => {
                    let png = tools::make_qr_png(&text)?;
                    app.tg
                        .send_photo_bytes(
                            chat_id,
                            Some(msg.message_id),
                            png,
                            "qrcode.png",
                            &format!("📱 <b>بارکد QR برای:</b>\n<code>{}</code>", html_escape(&text)),
                            Some(back_home_kb()),
                        )
                        .await?;
                    return Ok(());
                }
                "wait_weather" => {
                    let rep = tools::weather_report(&app.tg.http, &text).await;
                    app.tg
                        .send_html(chat_id, Some(msg.message_id), &rep, Some(back_home_kb()))
                        .await?;
                    return Ok(());
                }
                "wait_broadcast" if is_owner => {
                    let uids = app.store.all_user_ids().await;
                    let mut ok_count = 0usize;
                    for uid in uids {
                        if app
                            .tg
                            .send_html(
                                uid,
                                None,
                                &format!("📢 <b>پیام همگانی مدیریت:</b>\n\n{}", &text),
                                None,
                            )
                            .await
                            .is_ok()
                        {
                            ok_count += 1;
                        }
                    }
                    app.tg
                        .send_html(
                            chat_id,
                            Some(msg.message_id),
                            &format!("✅ پیام همگانی با موفقیت برای <b>{}</b> کاربر ارسال شد.", ok_count),
                            Some(back_home_kb()),
                        )
                        .await?;
                    return Ok(());
                }
                _ => {}
            }
        }
    }

    // Parse command & argument
    let (cmd, arg) = parse_cmd(&text);

    // ── 1. START / HELP / MENU (including deep-link `/start anon_xxx`) ──
    if matches!(cmd.as_str(), "/start" | "/help" | "/menu")
        || text == "منو"
        || text == "راهنما"
        || text == "استارت"
    {
        if let Some(anon_tok) = arg.strip_prefix("anon_") {
            let full_tok = format!("anon_{}", anon_tok);
            if let Some(target_uid) = app.store.resolve_anon_token(&full_tok).await {
                app.store
                    .set_state(user_id, Some(format!("anon_send:{}", target_uid)))
                    .await;
                app.tg
                    .send_html(
                        chat_id,
                        Some(msg.message_id),
                        "💌 <b>در حال ارسال پیام ناشناس...</b>\n\n✍️ لطفاً متن پیام خود را بنویسید و ارسال کنید (هویت شما کاملاً مخفی می‌ماند):",
                        Some(back_home_kb()),
                    )
                    .await?;
                return Ok(());
            }
        }
        app.store.set_state(user_id, None).await;
        let welcome = welcome_text(app, &user.first_name);
        app.tg
            .send_html_full(
                chat_id,
                Some(msg.message_id),
                Some("به ربات همه‌فن‌حریف امیر تولز خوش آمدید"),
                &welcome,
                Some(home_keyboard(is_owner)),
                if is_group { None } else { Some(EFFECT_FIRE) },
            )
            .await?;
        return Ok(());
    }

    // ── 2. YOUTUBE / INSTAGRAM / MUSIC DOWNLOADER (`yt-dlp`) ──
    if matches!(
        cmd.as_str(),
        "/yt" | "/youtube" | "/ytdl" | "/ig" | "/insta" | "/music" | "/song"
    ) {
        if arg.is_empty() {
            app.store.set_state(user_id, Some("wait_yt".into())).await;
            app.tg
                .send_html(
                    chat_id,
                    Some(msg.message_id),
                    "🎬 <b>دانلودر مستقیم یوتیوب، اینستاگرام و موزیک (<code>yt-dlp</code>):</b>\n\n\
                     🔗 لطفاً لینک ویدیو (YouTube / Instagram / TikTok / SoundCloud) یا <b>نام ویدیو/آهنگ</b> را بفرستید:",
                    Some(back_home_kb()),
                )
                .await?;
        } else if cmd == "/music" || cmd == "/song" {
            // Direct MP3 download or card
            ytdlp::present_card(&app.tg, &app.cfg, &app.store, chat_id, msg.message_id, &arg)
                .await?;
        } else {
            ytdlp::present_card(&app.tg, &app.cfg, &app.store, chat_id, msg.message_id, &arg)
                .await?;
        }
        return Ok(());
    }

    // Auto-detect YouTube / Instagram / TikTok / SoundCloud links in message!
    if let Some(media_url) = ytdlp::detect_media_url(&text) {
        ytdlp::present_card(
            &app.tg,
            &app.cfg,
            &app.store,
            chat_id,
            msg.message_id,
            &media_url,
        )
        .await?;
        return Ok(());
    }

    // ── 3. AUTONOMOUS WEB FILE HUNTER & SEARCH AGENT ──
    if matches!(
        cmd.as_str(),
        "/findfile" | "/agent" | "/hunt" | "/file" | "/searchfile"
    ) {
        if arg.is_empty() {
            app.store
                .set_state(user_id, Some("wait_agent".into()))
                .await;
            app.tg
                .send_html(
                    chat_id,
                    Some(msg.message_id),
                    "🕵️ <b>ایجنت هوشمند شکارچی فایل از وب:</b>\n\n\
                     🔎 نام فایل، کتاب (PDF)، آهنگ (MP3)، برنامه اندروید (APK)، زیرنویس (SRT) یا فایل فشرده (ZIP) مورد نظر خود را بفرستید تا ایجنت در سایت‌ها بگردد، دانلود کند و برایتان بفرستد:",
                    Some(back_home_kb()),
                )
                .await?;
        } else {
            agent::run_file_hunter_agent(
                &app.tg,
                &app.cfg,
                &app.store,
                chat_id,
                user_id,
                msg.message_id,
                &arg,
            )
            .await?;
        }
        return Ok(());
    }

    // Direct URL to Telegram uploader (`/dl <url>`)
    if cmd == "/dl" || cmd == "/download" {
        if arg.is_empty() {
            app.tg
                .send_html(
                    chat_id,
                    Some(msg.message_id),
                    "⬇️ <b>آپلودر مستقیم لینک به تلگرام:</b>\nمثال: <code>/dl https://example.com/file.pdf</code>",
                    None,
                )
                .await?;
        } else {
            agent::handle_direct_download(
                &app.tg,
                &app.cfg,
                &app.store,
                chat_id,
                user_id,
                Some(msg.message_id),
                &arg,
            )
            .await?;
        }
        return Ok(());
    }

    // Natural Persian trigger for the File Hunter Agent (e.g. `پیدا کن کتاب ...`, `دانلود فایل ...`, `بگرد دنبال ...`, `آهنگ ... دانلود کن`)
    if let Some(hunt_q) = extract_natural_file_hunt_query(&text) {
        agent::run_file_hunter_agent(
            &app.tg,
            &app.cfg,
            &app.store,
            chat_id,
            user_id,
            msg.message_id,
            &hunt_q,
        )
        .await?;
        return Ok(());
    }

    // ── 4. LIVE CURRENCY, GOLD, COIN & CRYPTO ──
    if matches!(
        cmd.as_str(),
        "/currency" | "/price" | "/arz" | "/crypto" | "/gold" | "/tala"
    ) || matches!(
        text.as_str(),
        "قیمت" | "قیمت ارز" | "قیمت طلا" | "قیمت دلار" | "ارز" | "دلار" | "طلا" | "سکه" | "کریپتو" | "رمزارز"
    ) {
        app.tg.send_action(chat_id, "typing").await;
        let board = tools::format_full_market_board(&app.tg.http).await;
        let kb: Keyboard = vec![
            vec![InlineBtn::cb("🔄 بروزرسانی لحظه‌ای قیمت‌ها", "menu:market")],
            vec![InlineBtn::cb("🏠 منوی اصلی", "menu:home")],
        ];
        app.tg
            .send_html_full(
                chat_id,
                Some(msg.message_id),
                Some("تابلو زنده نرخ ارز، طلا و رمزارز"),
                &board,
                Some(kb),
                if is_group { None } else { Some(EFFECT_FIRE) },
            )
            .await?;
        return Ok(());
    }

    if let Some(conv_reply) = tools::try_currency_conversion(&app.tg.http, &text).await {
        app.tg
            .send_html(chat_id, Some(msg.message_id), &conv_reply, None)
            .await?;
        return Ok(());
    }

    // ── 5. AI IMAGE GENERATION (`/draw`, `عکس ...`) ──
    if matches!(cmd.as_str(), "/draw" | "/image" | "/img")
        || text.starts_with("عکس ")
        || text.starts_with("تصویر ")
        || text.starts_with("بکش ")
    {
        let prompt = if !arg.is_empty() {
            arg.clone()
        } else {
            text.trim_start_matches("عکس ")
                .trim_start_matches("تصویر ")
                .trim_start_matches("بکش ")
                .trim()
                .to_string()
        };
        if prompt.is_empty() {
            app.store.set_state(user_id, Some("wait_draw".into())).await;
            app.tg
                .send_html(
                    chat_id,
                    Some(msg.message_id),
                    "🎨 لطفاً موضوع تصویر مورد نظر خود را بنویسید (مثال: <code>یک شهر سایبرپانک در شب بارانی</code>):",
                    Some(back_home_kb()),
                )
                .await?;
        } else {
            tools::generate_and_send_image(
                &app.tg,
                &app.store,
                chat_id,
                user_id,
                msg.message_id,
                &prompt,
            )
            .await?;
        }
        return Ok(());
    }

    // ── 6. VOICE / TTS ENGINE (`/tts`, `/voice`, `صدا ...`) ──
    if matches!(cmd.as_str(), "/tts" | "/voice")
        || text.starts_with("صدا ")
        || text.starts_with("وویس ")
        || text.starts_with("بگو ")
    {
        let tts_text = if !arg.is_empty() {
            arg.clone()
        } else {
            text.trim_start_matches("صدا ")
                .trim_start_matches("وویس ")
                .trim_start_matches("بگو ")
                .trim()
                .to_string()
        };
        if tts_text.is_empty() {
            app.store.set_state(user_id, Some("wait_tts".into())).await;
            app.tg
                .send_html(
                    chat_id,
                    Some(msg.message_id),
                    "🎙 لطفاً متنی که می‌خواهید به ویس تبدیل شود را ارسال کنید:",
                    Some(back_home_kb()),
                )
                .await?;
        } else {
            tools::synthesize_and_send_tts(
                &app.tg,
                &app.cfg,
                &app.store,
                chat_id,
                user_id,
                msg.message_id,
                &tts_text,
            )
            .await?;
        }
        return Ok(());
    }

    // ── 7. VIDEO COMPRESSOR & VISUAL PODCAST ──
    if cmd == "/compress" || text == "کم حجم کن" || text == "فشرده کن" {
        if let Some(ref rep) = msg.reply_to_message {
            if let Some(ref vid) = rep.video {
                let fid_key = app.store.stash_url(&vid.file_id).await;
                let kb: Keyboard = vec![
                    vec![
                        InlineBtn::cb("🔥 فشرده‌سازی شدید (CRF 34)", format!("vcomp:34:{}", fid_key)),
                        InlineBtn::cb("⚖️ متعادل (CRF 28)", format!("vcomp:28:{}", fid_key)),
                    ],
                    vec![InlineBtn::cb("💎 کیفیت بالا (CRF 24)", format!("vcomp:24:{}", fid_key))],
                ];
                app.tg
                    .send_html(
                        chat_id,
                        Some(msg.message_id),
                        "🗜 <b>میزان فشرده‌سازی ویدیو را انتخاب کنید:</b>",
                        Some(kb),
                    )
                    .await?;
                return Ok(());
            }
        }
        app.tg
            .send_html(
                chat_id,
                Some(msg.message_id),
                "🗜 لطفاً یک ویدیو برای ربات بفرستید یا روی ویدیوی مورد نظر ریپلای بزنید و بنویسید <code>کم حجم کن</code>.",
                None,
            )
            .await?;
        return Ok(());
    }

    if cmd == "/podcast" {
        if arg.is_empty() {
            app.store
                .set_state(user_id, Some("wait_podcast".into()))
                .await;
            app.tg
                .send_html(
                    chat_id,
                    Some(msg.message_id),
                    "🎬 لطفاً متن یا موضوع پادکست تصویری را بفرستید:",
                    Some(back_home_kb()),
                )
                .await?;
        } else {
            tools::create_visual_podcast(&app.tg, &app.cfg, chat_id, msg.message_id, &arg).await?;
        }
        return Ok(());
    }

    // ── 8. TRANSLATE, SUMMARIZE, REWRITE, AI MODELS & MEMORY ──
    if cmd == "/tr" || cmd == "/translate" || text.starts_with("ترجمه ") {
        let q = if !arg.is_empty() {
            arg.clone()
        } else {
            text.trim_start_matches("ترجمه ").trim().to_string()
        };
        if !q.is_empty() {
            app.tg.send_action(chat_id, "typing").await;
            let ans = tools::ai_chat(
                &app.tg.http,
                &app.cfg,
                &app.store,
                user_id,
                &q,
                Some("متن کاربر را با دقت بالا به فارسی روان (و اگر فارسی است به انگلیسی) ترجمه کن."),
            )
            .await;
            app.tg
                .send_html(
                    chat_id,
                    Some(msg.message_id),
                    &format!("🌐 <b>ترجمه هوشمند:</b>\n\n{}", html_escape(&ans)),
                    None,
                )
                .await?;
            return Ok(());
        }
    }

    if cmd == "/summary" || text.starts_with("خلاصه ") {
        let q = if !arg.is_empty() {
            arg.clone()
        } else {
            text.trim_start_matches("خلاصه ").trim().to_string()
        };
        if !q.is_empty() {
            app.tg.send_action(chat_id, "typing").await;
            let ans = tools::ai_chat(
                &app.tg.http,
                &app.cfg,
                &app.store,
                user_id,
                &q,
                Some("متن زیر را به صورت نکات کلیدی و خلاصه به زبان فارسی استخراج کن."),
            )
            .await;
            app.tg
                .send_html(
                    chat_id,
                    Some(msg.message_id),
                    &format!("📝 <b>خلاصه هوشمند:</b>\n\n{}", html_escape(&ans)),
                    None,
                )
                .await?;
            return Ok(());
        }
    }

    if cmd == "/clear" || text == "پاکسازی حافظه" {
        app.store.clear_history(user_id).await;
        app.tg
            .send_html(
                chat_id,
                Some(msg.message_id),
                "🧹 <b>حافظه گفتگوی هوش مصنوعی شما پاکسازی شد.</b>",
                None,
            )
            .await?;
        return Ok(());
    }

    // ── 9. QR, WEATHER, NETWORK, WIKI, MATH, DATE, ABJAD, PASS ──
    if cmd == "/qr" || text.starts_with("بارکد ") {
        let q = if !arg.is_empty() {
            arg.clone()
        } else {
            text.trim_start_matches("بارکد ").trim().to_string()
        };
        if q.is_empty() {
            app.store.set_state(user_id, Some("wait_qr".into())).await;
            app.tg
                .send_html(
                    chat_id,
                    Some(msg.message_id),
                    "📱 متن یا لینک مورد نظر برای ساخت بارکد QR را ارسال کنید:",
                    Some(back_home_kb()),
                )
                .await?;
        } else {
            let png = tools::make_qr_png(&q)?;
            app.tg
                .send_photo_bytes(
                    chat_id,
                    Some(msg.message_id),
                    png,
                    "qrcode.png",
                    &format!("📱 <b>بارکد QR اختصاصی:</b>\n<code>{}</code>", html_escape(&q)),
                    None,
                )
                .await?;
        }
        return Ok(());
    }

    if matches!(cmd.as_str(), "/weather" | "/hava")
        || text.starts_with("هوا ")
        || text.starts_with("آب و هوا ")
        || text == "هواشناسی"
    {
        let city = if !arg.is_empty() {
            arg.clone()
        } else {
            text.trim_start_matches("آب و هوا ")
                .trim_start_matches("هواشناسی")
                .trim_start_matches("هوا ")
                .trim()
                .to_string()
        };
        let rep = tools::weather_report(&app.tg.http, &city).await;
        app.tg
            .send_html(chat_id, Some(msg.message_id), &rep, None)
            .await?;
        return Ok(());
    }

    if cmd == "/ping" || text == "پینگ" || text.starts_with("پینگ ") {
        let target = if !arg.is_empty() {
            arg.as_str()
        } else if let Some(r) = text.strip_prefix("پینگ ") {
            r.trim()
        } else {
            "google.com"
        };
        let rep = tools::ping_host(target).await;
        app.tg
            .send_html(chat_id, Some(msg.message_id), &rep, None)
            .await?;
        return Ok(());
    }

    if cmd == "/dns" || text.starts_with("دی ان اس ") {
        let dom = if !arg.is_empty() {
            arg.as_str()
        } else {
            text.trim_start_matches("دی ان اس ").trim()
        };
        let rep = tools::dns_lookup(dom).await;
        app.tg
            .send_html(chat_id, Some(msg.message_id), &rep, None)
            .await?;
        return Ok(());
    }

    if cmd == "/port" || text.starts_with("پورت ") {
        let rest = if !arg.is_empty() {
            arg.as_str()
        } else {
            text.trim_start_matches("پورت ").trim()
        };
        let parts: Vec<&str> = rest.split_whitespace().collect();
        let host = parts.first().copied().unwrap_or("google.com");
        let port: u16 = parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(443);
        let rep = tools::port_check(host, port).await;
        app.tg
            .send_html(chat_id, Some(msg.message_id), &rep, None)
            .await?;
        return Ok(());
    }

    if cmd == "/ip" || cmd == "/whois" {
        let target = if arg.is_empty() { "1.1.1.1" } else { &arg };
        let rep = tools::ip_lookup(&app.tg.http, target).await;
        app.tg
            .send_html(chat_id, Some(msg.message_id), &rep, None)
            .await?;
        return Ok(());
    }

    if cmd == "/wiki" || text.starts_with("ویکی ") {
        let q = if !arg.is_empty() {
            arg.as_str()
        } else {
            text.trim_start_matches("ویکی ").trim()
        };
        let rep = tools::wikipedia_summary(&app.tg.http, q).await;
        app.tg
            .send_html(chat_id, Some(msg.message_id), &rep, None)
            .await?;
        return Ok(());
    }

    if cmd == "/calc" || text.starts_with("حساب ") || text.starts_with("محاسبه ") {
        let expr = if !arg.is_empty() {
            arg.as_str()
        } else {
            text.trim_start_matches("حساب ")
                .trim_start_matches("محاسبه ")
                .trim()
        };
        let rep = match tools::eval_math(expr) {
            Ok(v) => format!("🧮 <b>نتیجه محاسبه:</b>\n\n<code>{} = {}</code>", html_escape(expr), v),
            Err(e) => format!("⚠️ خطا در فرمول: {}", html_escape(&e.to_string())),
        };
        app.tg
            .send_html(chat_id, Some(msg.message_id), &rep, None)
            .await?;
        return Ok(());
    }

    if matches!(cmd.as_str(), "/time" | "/date" | "/cal")
        || matches!(text.as_str(), "ساعت" | "تاریخ" | "تقویم")
    {
        app.tg
            .send_html(
                chat_id,
                Some(msg.message_id),
                &format!("📅 <b>تقویم شمسی و ساعت رسمی ایران:</b>\n\n<code>{}</code>", tools::now_jalali_str()),
                None,
            )
            .await?;
        return Ok(());
    }

    if cmd == "/abjad" || text.starts_with("ابجد ") {
        let q = if !arg.is_empty() {
            arg.as_str()
        } else {
            text.trim_start_matches("ابجد ").trim()
        };
        app.tg
            .send_html(chat_id, Some(msg.message_id), &tools::abjad_calc(q), None)
            .await?;
        return Ok(());
    }

    if cmd == "/pass" || cmd == "/password" || text == "پسورد" || text == "رمز" {
        let n: usize = arg.parse().unwrap_or(16);
        app.tg
            .send_html(
                chat_id,
                Some(msg.message_id),
                &tools::generate_password(n),
                None,
            )
            .await?;
        return Ok(());
    }

    // ── 10. ENTERTAINMENT: FAL, JOKE, FACT, QUOTE, RIDDLE, COIN, RPS, ANON ──
    if cmd == "/fal" || text == "فال" || text == "فال حافظ" {
        app.tg
            .send_html_full(
                chat_id,
                Some(msg.message_id),
                None,
                &tools::hafez_fal(),
                None,
                if is_group { None } else { Some(EFFECT_HEART) },
            )
            .await?;
        return Ok(());
    }

    if cmd == "/joke" || text == "جوک" || text == "لطیفه" {
        app.tg
            .send_html_full(
                chat_id,
                Some(msg.message_id),
                None,
                &tools::persian_joke(),
                None,
                if is_group { None } else { Some(EFFECT_PARTY) },
            )
            .await?;
        return Ok(());
    }

    if cmd == "/fact" || text == "دانستنی" {
        app.tg
            .send_html(chat_id, Some(msg.message_id), &tools::persian_fact(), None)
            .await?;
        return Ok(());
    }

    if cmd == "/quote" || text == "سخن بزرگان" {
        app.tg
            .send_html(chat_id, Some(msg.message_id), &tools::persian_quote(), None)
            .await?;
        return Ok(());
    }

    if cmd == "/riddle" || text == "معما" {
        app.tg
            .send_html(chat_id, Some(msg.message_id), &tools::persian_riddle(), None)
            .await?;
        return Ok(());
    }

    if matches!(cmd.as_str(), "/coin" | "/flip") || text == "سکه" || text == "شیر یا خط" {
        let heads = rand::random::<bool>();
        let side = if heads { "🦁 <b>شیر</b>" } else { "📜 <b>خط</b>" };
        app.tg
            .send_html(
                chat_id,
                Some(msg.message_id),
                &format!("🪙 سکه چرخید و آمد: {}", side),
                None,
            )
            .await?;
        return Ok(());
    }

    if cmd == "/rps" {
        let kb: Keyboard = vec![vec![
            InlineBtn::cb("🪨 سنگ", "rps:سنگ"),
            InlineBtn::cb("📄 کاغذ", "rps:کاغذ"),
            InlineBtn::cb("✂️ قیچی", "rps:قیچی"),
        ]];
        app.tg
            .send_html(
                chat_id,
                Some(msg.message_id),
                "🎮 <b>بازی سنگ، کاغذ، قیچی!</b>\nانتخاب خود را بزنید:",
                Some(kb),
            )
            .await?;
        return Ok(());
    }

    if matches!(cmd.as_str(), "/anon" | "/harf") || text == "چت ناشناس" {
        let tok = app.store.get_or_create_anon_token(user_id).await;
        let link = format!("https://t.me/{}?start={}", app.cfg.bot_username, tok);
        app.tg
            .send_html(
                chat_id,
                Some(msg.message_id),
                &format!(
                    "💌 <b>لینک اختصاصی پیام ناشناس شما:</b>\n\n\
                     🔗 <code>{}</code>\n\n\
                     <i>این لینک را در بیو یا کانال/گروه خود قرار دهید؛ هرکس روی آن بزند می‌تواند بدون مشخص شدن نامش به شما پیام بفرستد!</i>",
                    link
                ),
                Some(back_home_kb()),
            )
            .await?;
        return Ok(());
    }

    if cmd == "/score" || cmd == "/profile" || text == "امتیاز من" || text == "پروفایل" {
        if let Some(u) = app.store.get_user(user_id).await {
            app.tg
                .send_html(
                    chat_id,
                    Some(msg.message_id),
                    &format!(
                        "👤 <b>پروفایل کاربری شما در امیر تولز:</b>\n\n\
                         • نام: <b>{}</b>\n\
                         • شناسه عددی: <code>{}</code>\n\
                         • امتیاز کل: <b>{} ⭐</b>\n\
                         • تعداد پیام‌ها: <code>{}</code>\n\
                         • فایل‌های دانلود شده: <code>{}</code>\n\
                         • مدل هوش مصنوعی: <code>{}</code>",
                        html_escape(&u.first_name),
                        u.id,
                        u.score,
                        u.messages,
                        u.downloads,
                        html_escape(u.ai_model.as_deref().unwrap_or(&app.cfg.ai_model))
                    ),
                    None,
                )
                .await?;
        }
        return Ok(());
    }

    if cmd == "/top" || text == "برترین ها" {
        let top = app.store.top_users(10).await;
        let mut lines = vec!["🏆 <b>۱۰ کاربر برتر ربات امیر تولز:</b>\n".to_string()];
        for (i, u) in top.iter().enumerate() {
            lines.push(format!(
                "{}. <b>{}</b> — <code>{} ⭐</code> ({} دانلود)",
                i + 1,
                html_escape(&u.first_name),
                u.score,
                u.downloads
            ));
        }
        app.tg
            .send_html(chat_id, Some(msg.message_id), &lines.join("\n"), None)
            .await?;
        return Ok(());
    }

    // ── 11. GROUP MODERATION COMMANDS (RUBIKA & TELEGRAM STYLE) ──
    if is_group {
        let is_grp_admin = is_owner || app.tg.is_admin(chat_id, user_id).await;
        if is_grp_admin {
            if cmd == "/settings" || text == "تنظیمات گروه" || text == "پنل گروه" {
                let gs = app
                    .store
                    .group_settings(chat_id, msg.chat.title.as_deref().unwrap_or(""))
                    .await;
                app.tg
                    .send_html(
                        chat_id,
                        Some(msg.message_id),
                        "🛡 <b>پنل مدیریت هوشمند گروه امیر تولز:</b>",
                        Some(group_panel_kb(&gs)),
                    )
                    .await?;
                return Ok(());
            }
            if text == "قفل لینک" {
                app.store.update_group(chat_id, |g| g.lock_links = true).await;
                app.tg
                    .send_html(chat_id, Some(msg.message_id), "🔒 <b>قفل ارسال لینک فعال شد.</b>", None)
                    .await?;
                return Ok(());
            }
            if text == "بازکردن لینک" || text == "بازه لینک" {
                app.store.update_group(chat_id, |g| g.lock_links = false).await;
                app.tg
                    .send_html(chat_id, Some(msg.message_id), "🔓 <b>ارسال لینک آزاد شد.</b>", None)
                    .await?;
                return Ok(());
            }
            if text == "قفل فروارد" {
                app.store.update_group(chat_id, |g| g.lock_forwards = true).await;
                app.tg
                    .send_html(chat_id, Some(msg.message_id), "🔒 <b>قفل فروارد فعال شد.</b>", None)
                    .await?;
                return Ok(());
            }
            if text == "بازکردن فروارد" {
                app.store.update_group(chat_id, |g| g.lock_forwards = false).await;
                app.tg
                    .send_html(chat_id, Some(msg.message_id), "🔓 <b>ارسال فروارد آزاد شد.</b>", None)
                    .await?;
                return Ok(());
            }
            if let Some(ref rep) = msg.reply_to_message {
                if let Some(ref target_u) = rep.from {
                    if cmd == "/ban" || text == "بن" || text == "اخراج" {
                        let _ = app.tg.ban_member(chat_id, target_u.id).await;
                        app.tg
                            .send_html(
                                chat_id,
                                Some(msg.message_id),
                                &format!("🔨 کاربر <b>{}</b> از گروه اخراج شد.", html_escape(&target_u.first_name)),
                                None,
                            )
                            .await?;
                        return Ok(());
                    }
                    if cmd == "/unban" || text == "حذف بن" {
                        let _ = app.tg.unban_member(chat_id, target_u.id).await;
                        app.tg
                            .send_html(
                                chat_id,
                                Some(msg.message_id),
                                &format!("✅ مسدودیت <b>{}</b> برداشته شد.", html_escape(&target_u.first_name)),
                                None,
                            )
                            .await?;
                        return Ok(());
                    }
                    if cmd == "/mute" || text == "سکوت" {
                        let _ = app.tg.mute_member(chat_id, target_u.id, false).await;
                        app.tg
                            .send_html(
                                chat_id,
                                Some(msg.message_id),
                                &format!("🔇 کاربر <b>{}</b> در حالت سکوت قرار گرفت.", html_escape(&target_u.first_name)),
                                None,
                            )
                            .await?;
                        return Ok(());
                    }
                    if cmd == "/unmute" || text == "حذف سکوت" {
                        let _ = app.tg.mute_member(chat_id, target_u.id, true).await;
                        app.tg
                            .send_html(
                                chat_id,
                                Some(msg.message_id),
                                &format!("🔊 سکوت <b>{}</b> برداشته شد.", html_escape(&target_u.first_name)),
                                None,
                            )
                            .await?;
                        return Ok(());
                    }
                    if cmd == "/pin" || text == "سنجاق" {
                        let _ = app.tg.pin_message(chat_id, rep.message_id).await;
                        app.tg
                            .send_html(chat_id, Some(msg.message_id), "📌 پیام سنجاق شد.", None)
                            .await?;
                        return Ok(());
                    }
                }
            }
        }
    }

    // ── 12. MASTER ADMIN COMMANDS (`/panel`, `/status`, `/sysinfo`, `/sh`, `/ghostban`) ──
    if matches!(cmd.as_str(), "/status" | "/sysinfo") || text == "وضعیت" || text == "سرور" {
        let up = app.started_at.elapsed().as_secs();
        let (users, groups, msgs, dls) = app.store.stats().await;
        let status_txt = format!(
            "📊 <b>وضعیت لحظه‌ای سرور و هسته Rust امیر تولز:</b>\n\n\
             • <b>موتور اجرایی:</b> <code>100% Pure Rust v6.0 (Tokio Async)</code> 🦀\n\
             • <b>سیستم‌عامل:</b> <code>{} ({})</code>\n\
             • <b>آپ‌تایم بدون وقفه:</b> <code>{}h {}m {}s</code>\n\
             • <b>کاربران ثبت‌شده:</b> <code>{}</code>  |  <b>گروه‌ها:</b> <code>{}</code>\n\
             • <b>کل پیام‌های پردازش‌شده:</b> <code>{}</code>\n\
             • <b>کل فایل‌های دانلود شده:</b> <code>{}</code>\n\
             • <b>موتور دانلود یوتیوب:</b> <code>yt-dlp + ffmpeg (فعال)</code>\n\
             • <b>ایجنت شکارچی فایل وب:</b> <code>فعال (Multi-Stage Scraper)</code>",
            std::env::consts::OS,
            std::env::consts::ARCH,
            up / 3600,
            (up % 3600) / 60,
            up % 60,
            users,
            groups,
            msgs,
            dls
        );
        app.tg
            .send_html(chat_id, Some(msg.message_id), &status_txt, None)
            .await?;
        return Ok(());
    }

    if is_owner {
        if matches!(cmd.as_str(), "/panel" | "/admin") || text == "پنل" || text == "پنل ادمین" {
            let (users, groups, msgs, dls) = app.store.stats().await;
            let kb: Keyboard = vec![
                vec![
                    InlineBtn::cb("📢 ارسال پیام همگانی", "adm:broadcast"),
                    InlineBtn::cb("📊 آمار کامل ربات", "adm:stats"),
                ],
                vec![InlineBtn::cb("🏠 بازگشت به منوی اصلی", "menu:home")],
            ];
            app.tg
                .send_html(
                    chat_id,
                    Some(msg.message_id),
                    &format!(
                        "👑 <b>پنل فرماندهی کل امیر تولز (Owner ID: <code>{}</code>):</b>\n\n\
                         👥 کاربران: <b>{}</b> | 🛡 گروه‌ها: <b>{}</b>\n\
                         💬 پیام‌ها: <b>{}</b> | 📥 دانلودها: <b>{}</b>\n\n\
                         📌 <b>دستورات سریع مدیریت:</b>\n\
                         • <code>/sh دستور لینوکس</code> (ترمینال مستقیم سرور)\n\
                         • <code>/dl لینک مستقیم</code> (دانلود فایل به سرور و ارسال به تلگرام)\n\
                         • <code>/ghostban آیدی</code> / <code>/unghostban آیدی</code> (روح‌بن بی‌صدا)\n\
                         • <code>/banuser آیدی</code> / <code>/unbanuser آیدی</code>\n\
                         • <code>/addrule قانون جدید</code> / <code>/clearrules</code>",
                        app.cfg.owner_id, users, groups, msgs, dls
                    ),
                    Some(kb),
                )
                .await?;
            return Ok(());
        }

        if cmd == "/ghostban" {
            let target_id = arg
                .parse::<i64>()
                .ok()
                .or_else(|| msg.reply_to_message.as_ref().and_then(|r| r.from.as_ref().map(|u| u.id)));
            if let Some(tid) = target_id {
                app.store.set_ghostban(tid, true).await;
                app.tg
                    .send_html(
                        chat_id,
                        Some(msg.message_id),
                        &format!("👻 کاربر <code>{}</code> روح‌بن (Ghostban) شد.", tid),
                        None,
                    )
                    .await?;
            }
            return Ok(());
        }

        if cmd == "/unghostban" {
            let target_id = arg
                .parse::<i64>()
                .ok()
                .or_else(|| msg.reply_to_message.as_ref().and_then(|r| r.from.as_ref().map(|u| u.id)));
            if let Some(tid) = target_id {
                app.store.set_ghostban(tid, false).await;
                app.tg
                    .send_html(
                        chat_id,
                        Some(msg.message_id),
                        &format!("✅ روح‌بن کاربر <code>{}</code> برداشته شد.", tid),
                        None,
                    )
                    .await?;
            }
            return Ok(());
        }

        if cmd == "/banuser" {
            if let Ok(tid) = arg.parse::<i64>() {
                app.store.set_ban(tid, true).await;
                app.tg
                    .send_html(
                        chat_id,
                        Some(msg.message_id),
                        &format!("⛔ کاربر <code>{}</code> مسدود شد.", tid),
                        None,
                    )
                    .await?;
            }
            return Ok(());
        }

        if cmd == "/unbanuser" {
            if let Ok(tid) = arg.parse::<i64>() {
                app.store.set_ban(tid, false).await;
                app.tg
                    .send_html(
                        chat_id,
                        Some(msg.message_id),
                        &format!("✅ کاربر <code>{}</code> رفع مسدودیت شد.", tid),
                        None,
                    )
                    .await?;
            }
            return Ok(());
        }

        if cmd == "/addrule" && !arg.is_empty() {
            app.store.add_rule(&arg).await;
            app.tg
                .send_html(
                    chat_id,
                    Some(msg.message_id),
                    "✅ قانون جدید به حافظه دائمی هوش مصنوعی اضافه شد.",
                    None,
                )
                .await?;
            return Ok(());
        }

        if cmd == "/clearrules" {
            app.store.clear_rules().await;
            app.tg
                .send_html(chat_id, Some(msg.message_id), "🧹 تمام قوانین سفارشی پاک شدند.", None)
                .await?;
            return Ok(());
        }

        if matches!(cmd.as_str(), "/sh" | "/bash" | "/cmd") {
            if arg.is_empty() {
                app.tg
                    .send_html(
                        chat_id,
                        Some(msg.message_id),
                        "💻 مثال: <code>/sh uname -a && df -h</code>",
                        None,
                    )
                    .await?;
                return Ok(());
            }
            app.tg.send_action(chat_id, "typing").await;
            let shell = if cfg!(windows) {
                tokio::process::Command::new("powershell")
                    .args(["-NoProfile", "-Command", &arg])
                    .output()
            } else {
                tokio::process::Command::new("sh")
                    .args(["-c", &arg])
                    .output()
            };
            match tokio::time::timeout(Duration::from_secs(30), shell).await {
                Ok(Ok(out)) => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    let stderr = String::from_utf8_lossy(&out.stderr);
                    let combined = format!("{}{}", stdout, stderr);
                    let clean = if combined.trim().is_empty() {
                        "(خروجی خالی)".to_string()
                    } else {
                        truncate_str(combined.trim(), 3500)
                    };
                    app.tg
                        .send_html(
                            chat_id,
                            Some(msg.message_id),
                            &format!(
                                "💻 <b>Exit {}:</b>\n<pre>{}</pre>",
                                out.status.code().unwrap_or(0),
                                html_escape(&clean)
                            ),
                            None,
                        )
                        .await?;
                }
                _ => {
                    app.tg
                        .send_html(
                            chat_id,
                            Some(msg.message_id),
                            "❌ خطا یا اتمام مهلت ۳۰ ثانیه‌ای دستور.",
                            None,
                        )
                        .await?;
                }
            }
            return Ok(());
        }
    }

    // ── 13. DIRECT FILE URL DETECTION (NON-YOUTUBE) ──
    if text.starts_with("http://") || text.starts_with("https://") {
        if agent::file_ext_of_url(&text).is_some() {
            agent::handle_direct_download(
                &app.tg,
                &app.cfg,
                &app.store,
                chat_id,
                user_id,
                Some(msg.message_id),
                &text,
            )
            .await?;
            return Ok(());
        }
    }

    // ── 14. DEFAULT: SMART AI CONVERSATION (IN PRIVATE CHAT OR WHEN `/ai` / MENTIONED IN GROUP) ──
    if !is_group || cmd == "/ai" || cmd == "/ask" || raw_text.contains("@Game_iran_amir_bot") {
        let prompt = if cmd == "/ai" || cmd == "/ask" {
            arg.as_str()
        } else {
            text.as_str()
        };
        if !prompt.is_empty() {
            app.tg.send_action(chat_id, "typing").await;
            let ans =
                tools::ai_chat(&app.tg.http, &app.cfg, &app.store, user_id, prompt, None).await;
            app.tg
                .send_html_full(
                    chat_id,
                    Some(msg.message_id),
                    Some(&truncate_str(prompt, 40)),
                    &ans,
                    None,
                    None,
                )
                .await?;
        }
    }

    Ok(())
}

// ============================================================================
// CALLBACK QUERY ROUTER
// ============================================================================

async fn handle_callback(app: &App, cb: CallbackQuery) -> Result<()> {
    let data = cb.data.as_deref().unwrap_or("");
    let Some(msg) = cb.message.clone() else {
        app.tg.answer_cb(&cb.id, None, false).await;
        return Ok(());
    };
    let chat_id = msg.chat.id;
    let msg_id = msg.message_id;
    let user_id = cb.from.id;
    let is_owner = user_id == app.cfg.owner_id;

    // 1. YouTube / Media quality selection (`ytdl:<mode>:<tok>`)
    if let Some(rest) = data.strip_prefix("ytdl:") {
        let parts: Vec<&str> = rest.splitn(2, ':').collect();
        if parts.len() == 2 {
            let mode = match parts[0] {
                "mp3" => ytdlp::MediaMode::AudioMp3,
                "v360" => ytdlp::MediaMode::Video360,
                "vbest" => ytdlp::MediaMode::VideoBest,
                _ => ytdlp::MediaMode::Video720,
            };
            if let Some(url) = app.store.recall_url(parts[1]).await {
                app.tg
                    .answer_cb(&cb.id, Some("⏳ دانلود با yt-dlp آغاز شد..."), false)
                    .await;
                ytdlp::run_download(
                    &app.tg, &app.cfg, &app.store, chat_id, user_id, msg_id, &url, mode,
                )
                .await?;
            } else {
                app.tg
                    .answer_cb(&cb.id, Some("⚠️ لینک منقضی شده؛ مجدداً ارسال کنید."), true)
                    .await;
            }
        }
        return Ok(());
    }

    // 2. Web File Hunter direct download button (`fhdl:<tok>`)
    if let Some(tok) = data.strip_prefix("fhdl:") {
        if let Some(url) = app.store.recall_url(tok).await {
            app.tg
                .answer_cb(&cb.id, Some("⬇️ در حال دانلود فایل به سرور..."), false)
                .await;
            agent::handle_direct_download(
                &app.tg,
                &app.cfg,
                &app.store,
                chat_id,
                user_id,
                Some(msg_id),
                &url,
            )
            .await?;
        } else {
            app.tg
                .answer_cb(&cb.id, Some("⚠️ لینک منقضی شده است."), true)
                .await;
        }
        return Ok(());
    }

    // 3. FFmpeg Video Compression (`vcomp:<crf>:<tok>`)
    if let Some(rest) = data.strip_prefix("vcomp:") {
        let parts: Vec<&str> = rest.splitn(2, ':').collect();
        if parts.len() == 2 {
            let crf: u8 = parts[0].parse().unwrap_or(28);
            if let Some(file_id) = app.store.recall_url(parts[1]).await {
                app.tg
                    .answer_cb(&cb.id, Some("🗜 فشرده‌سازی ویدیو آغاز شد..."), false)
                    .await;
                tools::compress_video_and_send(&app.tg, &app.cfg, chat_id, msg_id, &file_id, crf)
                    .await?;
            }
        }
        return Ok(());
    }

    // 4. Anonymous reply button (`anon_reply:<tok>`)
    if let Some(tok) = data.strip_prefix("anon_reply:") {
        if let Some(target_uid) = app.store.resolve_anon_token(tok).await {
            app.store
                .set_state(user_id, Some(format!("anon_send:{}", target_uid)))
                .await;
            app.tg.answer_cb(&cb.id, None, false).await;
            app.tg
                .send_html(
                    chat_id,
                    Some(msg_id),
                    "✍️ <b>پاسخ ناشناس خود را بنویسید و ارسال کنید:</b>",
                    Some(back_home_kb()),
                )
                .await?;
        }
        return Ok(());
    }

    // 5. AI Model Selection (`set_aimodel:<model>`)
    if let Some(m) = data.strip_prefix("set_aimodel:") {
        app.store.set_user_ai_model(user_id, m).await;
        app.tg
            .answer_cb(
                &cb.id,
                Some(&format!("✅ مدل هوش مصنوعی روی {} تنظیم شد", m)),
                false,
            )
            .await;
        app.tg
            .edit_html(
                chat_id,
                msg_id,
                &format!("✅ <b>مدل هوش مصنوعی شما روی <code>{}</code> تنظیم شد!</b>\nاکنون هر سوالی دارید بپرسید.", html_escape(m)),
                Some(back_home_kb()),
            )
            .await?;
        return Ok(());
    }

    // 6. Image Model Selection (`set_imgmodel:<model>`)
    if let Some(m) = data.strip_prefix("set_imgmodel:") {
        app.store.set_user_image_model(user_id, m).await;
        app.store.set_state(user_id, Some("wait_draw".into())).await;
        app.tg
            .answer_cb(&cb.id, Some(&format!("🎨 مدل تصویرساز: {}", m)), false)
            .await;
        app.tg
            .edit_html(
                chat_id,
                msg_id,
                &format!(
                    "🎨 <b>مدل تصویرساز روی <code>{}</code> تنظیم شد!</b>\n✏️ حالا موضوع تصویر مورد نظر خود را بنویسید:",
                    html_escape(m)
                ),
                Some(back_home_kb()),
            )
            .await?;
        return Ok(());
    }

    // 7. TTS Voice Selection (`set_voice:<voice>`)
    if let Some(v) = data.strip_prefix("set_voice:") {
        app.store.set_user_tts_voice(user_id, v).await;
        app.store.set_state(user_id, Some("wait_tts".into())).await;
        app.tg
            .answer_cb(&cb.id, Some("🎙 گوینده تغییر کرد"), false)
            .await;
        app.tg
            .edit_html(
                chat_id,
                msg_id,
                &format!(
                    "🎙 <b>گوینده صوتی روی <code>{}</code> تنظیم شد!</b>\n✍️ حالا متن مورد نظر برای تبدیل به ویس را بفرستید:",
                    html_escape(v)
                ),
                Some(back_home_kb()),
            )
            .await?;
        return Ok(());
    }

    // 8. Rock-Paper-Scissors (`rps:<choice>`)
    if let Some(choice) = data.strip_prefix("rps:") {
        let rep = tools::play_rps(choice);
        if rep.contains("برنده شدید") {
            app.store.add_score(user_id, 5).await;
        }
        app.tg.answer_cb(&cb.id, None, false).await;
        let kb: Keyboard = vec![
            vec![
                InlineBtn::cb("🪨 سنگ", "rps:سنگ"),
                InlineBtn::cb("📄 کاغذ", "rps:کاغذ"),
                InlineBtn::cb("✂️ قیچی", "rps:قیچی"),
            ],
            vec![InlineBtn::cb("🏠 منوی اصلی", "menu:home")],
        ];
        app.tg.edit_html(chat_id, msg_id, &rep, Some(kb)).await?;
        return Ok(());
    }

    // 9. Group Settings Toggles (`grp:...`)
    if let Some(act) = data.strip_prefix("grp:") {
        if !is_owner && !app.tg.is_admin(chat_id, user_id).await {
            app.tg
                .answer_cb(&cb.id, Some("⛔ فقط ادمین‌های گروه دسترسی دارند."), true)
                .await;
            return Ok(());
        }
        let gs = app
            .store
            .update_group(chat_id, |g| match act {
                "toggle_links" => g.lock_links = !g.lock_links,
                "toggle_fwds" => g.lock_forwards = !g.lock_forwards,
                "toggle_media" => g.lock_media = !g.lock_media,
                "toggle_welcome" => g.welcome_enabled = !g.welcome_enabled,
                "cycle_charlimit" => {
                    g.char_limit = match g.char_limit {
                        100 => 250,
                        250 => 500,
                        500 => 0,
                        _ => 100,
                    };
                }
                _ => {}
            })
            .await;
        app.tg
            .answer_cb(&cb.id, Some("✅ تنظیمات گروه بروزرسانی شد"), false)
            .await;
        app.tg
            .edit_html(
                chat_id,
                msg_id,
                "🛡 <b>پنل مدیریت هوشمند گروه امیر تولز:</b>",
                Some(group_panel_kb(&gs)),
            )
            .await?;
        return Ok(());
    }

    // 10. Menu Navigation (`menu:...` & `adm:...`)
    app.tg.answer_cb(&cb.id, None, false).await;
    match data {
        "menu:home" => {
            app.store.set_state(user_id, None).await;
            let txt = welcome_text(app, &cb.from.first_name);
            app.tg
                .edit_html(chat_id, msg_id, &txt, Some(home_keyboard(is_owner)))
                .await?;
        }
        "menu:dl" => {
            app.store.set_state(user_id, Some("wait_yt".into())).await;
            app.tg
                .edit_html(
                    chat_id,
                    msg_id,
                    "🎬 <b>دانلودر مستقیم یوتیوب، اینستاگرام، تیک‌تاک و موزیک (<code>yt-dlp</code>):</b>\n\n\
                     • کافیست <b>لینک ویدیو</b> یا <b>نام ویدیو / آهنگ</b> را همینجا بفرستید!\n\
                     • پشتیبانی از کیفیت‌های <code>720p MP4</code>، <code>360p MP4</code>، <code>Best</code> و استخراج مستقیم <code>MP3 192kbps</code>.",
                    Some(back_home_kb()),
                )
                .await?;
        }
        "menu:agent" => {
            app.store
                .set_state(user_id, Some("wait_agent".into()))
                .await;
            app.tg
                .edit_html(
                    chat_id,
                    msg_id,
                    "🕵️ <b>ایجنت هوشمند شکارچی فایل از وب (Web File Hunter):</b>\n\n\
                     ✍️ نام هر فایلی که می‌خواهید را بنویسید تا ایجنت به صورت خودکار:\n\
                     ۱. در موتورهای جستجو و مخازن فایل بگردد\n\
                     ۲. وارد سایت‌ها شود و لینک دانلود مستقیم (<code>PDF</code>, <code>MP3</code>, <code>APK</code>, <code>ZIP</code>, <code>MP4</code>, <code>EPUB</code>, <code>SRT</code>) را استخراج کند\n\
                     ۳. فایل را روی سرور دانلود کرده و مستقیماً در همین چت برایتان بفرستد!\n\n\
                     👇 <i>همین الان نام فایل، کتاب، برنامه یا آهنگ مورد نظر را تایپ کنید:</i>",
                    Some(back_home_kb()),
                )
                .await?;
        }
        "menu:market" => {
            let board = tools::format_full_market_board(&app.tg.http).await;
            let kb: Keyboard = vec![
                vec![InlineBtn::cb("🔄 بروزرسانی مجدد", "menu:market")],
                vec![InlineBtn::cb("🏠 منوی اصلی", "menu:home")],
            ];
            app.tg.edit_html(chat_id, msg_id, &board, Some(kb)).await?;
        }
        "menu:ai" => {
            let kb: Keyboard = vec![
                vec![
                    InlineBtn::cb("🧠 انتخاب مدل هوش مصنوعی", "menu:ai_models"),
                    InlineBtn::cb("🎨 ساخت تصویر (۱۸ مدل Flux)", "menu:img_models"),
                ],
                vec![InlineBtn::cb("🧹 پاکسازی حافظه چت", "ai:clear")],
                vec![InlineBtn::cb("🏠 منوی اصلی", "menu:home")],
            ];
            app.tg
                .edit_html(
                    chat_id,
                    msg_id,
                    "🧠 <b>مرکز هوش مصنوعی و تصویرساز امیر تولز:</b>\n\n\
                     • برای گفتگو با هوش مصنوعی کافیست هر سوالی دارید مستقیماً بنویسید.\n\
                     • برای ترجمه: <code>/tr متن</code>  |  برای خلاصه‌سازی: <code>/summary متن</code>\n\
                     • برای ساخت تصویر: <code>عکس یک قلعه باستانی در مه</code>",
                    Some(kb),
                )
                .await?;
        }
        "menu:ai_models" => {
            let mut kb: Keyboard = Vec::new();
            for chunk in tools::AI_MODELS.chunks(2) {
                let row: Vec<InlineBtn> = chunk
                    .iter()
                    .map(|(id, label)| InlineBtn::cb(*label, format!("set_aimodel:{}", id)))
                    .collect();
                kb.push(row);
            }
            kb.push(vec![InlineBtn::cb("🏠 منوی اصلی", "menu:home")]);
            app.tg
                .edit_html(
                    chat_id,
                    msg_id,
                    "🧠 <b>مدل هوش مصنوعی مورد نظر خود را انتخاب کنید:</b>",
                    Some(kb),
                )
                .await?;
        }
        "menu:img_models" => {
            let mut kb: Keyboard = Vec::new();
            for chunk in tools::IMAGE_MODELS.chunks(2) {
                let row: Vec<InlineBtn> = chunk
                    .iter()
                    .map(|(id, label)| InlineBtn::cb(*label, format!("set_imgmodel:{}", id)))
                    .collect();
                kb.push(row);
            }
            kb.push(vec![InlineBtn::cb("🏠 منوی اصلی", "menu:home")]);
            app.tg
                .edit_html(
                    chat_id,
                    msg_id,
                    "🎨 <b>مدل تصویرساز هوش مصنوعی را انتخاب کنید:</b>",
                    Some(kb),
                )
                .await?;
        }
        "ai:clear" => {
            app.store.clear_history(user_id).await;
            app.tg
                .edit_html(
                    chat_id,
                    msg_id,
                    "🧹 <b>حافظه گفتگوی هوش مصنوعی شما پاکسازی شد.</b>",
                    Some(back_home_kb()),
                )
                .await?;
        }
        "menu:tts" => {
            let mut kb: Keyboard = Vec::new();
            for (id, label) in tools::TTS_VOICES {
                kb.push(vec![InlineBtn::cb(*label, format!("set_voice:{}", id))]);
            }
            kb.push(vec![InlineBtn::cb("🏠 منوی اصلی", "menu:home")]);
            app.tg
                .edit_html(
                    chat_id,
                    msg_id,
                    "🎙 <b>استودیو تبدیل متن به گفتار عصبی (TTS):</b>\nگوینده مورد نظر خود را انتخاب کنید:",
                    Some(kb),
                )
                .await?;
        }
        "menu:media" => {
            let kb: Keyboard = vec![
                vec![InlineBtn::cb("🎬 ساخت پادکست تصویری هوشمند", "media:podcast")],
                vec![InlineBtn::cb("🏠 منوی اصلی", "menu:home")],
            ];
            app.tg
                .edit_html(
                    chat_id,
                    msg_id,
                    "🗜 <b>استودیو ویدیو و پادکست امیر تولز:</b>\n\n\
                     • <b>کم‌حجم کردن ویدیو:</b> کافیست هر ویدیویی را برای ربات بفرستید تا دکمه‌های فشرده‌سازی <code>FFmpeg</code> نمایش داده شود!\n\
                     • <b>پادکست تصویری:</b> ترکیب خودکار کاور هوش مصنوعی + نریشن صوتی در قالب ویدیوی MP4.",
                    Some(kb),
                )
                .await?;
        }
        "media:podcast" => {
            app.store
                .set_state(user_id, Some("wait_podcast".into()))
                .await;
            app.tg
                .edit_html(
                    chat_id,
                    msg_id,
                    "🎬 لطفاً متن یا موضوع پادکست تصویری را بنویسید:",
                    Some(back_home_kb()),
                )
                .await?;
        }
        "menu:tools" => {
            let kb: Keyboard = vec![
                vec![
                    InlineBtn::cb("🌤 هواشناسی زنده", "tool:weather"),
                    InlineBtn::cb("📱 ساخت بارکد QR", "tool:qr"),
                ],
                vec![
                    InlineBtn::cb("📅 تقویم شمسی و ساعت", "tool:time"),
                    InlineBtn::cb("🔐 تولید رمز عبور امن", "tool:pass"),
                ],
                vec![InlineBtn::cb("🏠 منوی اصلی", "menu:home")],
            ];
            app.tg
                .edit_html(
                    chat_id,
                    msg_id,
                    "🛠 <b>جعبه ابزار مهندسی و شبکه امیر تولز:</b>\n\n\
                     • <code>/weather شهر</code> — هواشناسی ماهواره‌ای\n\
                     • <code>/qr متن</code> — تولید بارکد QR\n\
                     • <code>/ping دامنه</code> — تست پینگ TCP\n\
                     • <code>/dns دامنه</code> — استخراج رکوردهای DNS\n\
                     • <code>/port دامنه 443</code> — اسکن پورت\n\
                     • <code>/ip آی‌پی یا دامنه</code> — اطلاعات دیتاسنتر و کشور\n\
                     • <code>/wiki موضوع</code> — خلاصه ویکی‌پدیا\n\
                     • <code>/calc (25*4)+10</code> — ماشین‌حساب مهندسی\n\
                     • <code>/abjad متن</code> — محاسبه ابجد کبیر و صغیر",
                    Some(kb),
                )
                .await?;
        }
        "tool:weather" => {
            app.store
                .set_state(user_id, Some("wait_weather".into()))
                .await;
            app.tg
                .edit_html(
                    chat_id,
                    msg_id,
                    "🌤 نام شهر مورد نظر را بنویسید (مثال: <code>تهران</code>، <code>مشهد</code>، <code>لندن</code>):",
                    Some(back_home_kb()),
                )
                .await?;
        }
        "tool:qr" => {
            app.store.set_state(user_id, Some("wait_qr".into())).await;
            app.tg
                .edit_html(
                    chat_id,
                    msg_id,
                    "📱 متن یا لینک مورد نظر برای ساخت بارکد QR را بفرستید:",
                    Some(back_home_kb()),
                )
                .await?;
        }
        "tool:time" => {
            app.tg
                .edit_html(
                    chat_id,
                    msg_id,
                    &format!("📅 <b>تقویم شمسی و ساعت ایران:</b>\n\n<code>{}</code>", tools::now_jalali_str()),
                    Some(back_home_kb()),
                )
                .await?;
        }
        "tool:pass" => {
            app.tg
                .edit_html(
                    chat_id,
                    msg_id,
                    &tools::generate_password(16),
                    Some(back_home_kb()),
                )
                .await?;
        }
        "menu:fun" => {
            let kb: Keyboard = vec![
                vec![
                    InlineBtn::cb("📖 فال حافظ شیرازی", "fun:fal"),
                    InlineBtn::cb("😄 جوک و لطیفه", "fun:joke"),
                ],
                vec![
                    InlineBtn::cb("💡 آیا می‌دانستید؟", "fun:fact"),
                    InlineBtn::cb("🧩 معمای روز", "fun:riddle"),
                ],
                vec![
                    InlineBtn::cb("✨ سخن بزرگان", "fun:quote"),
                    InlineBtn::cb("🎮 سنگ کاغذ قیچی", "fun:rps"),
                ],
                vec![InlineBtn::cb("🏠 منوی اصلی", "menu:home")],
            ];
            app.tg
                .edit_html(
                    chat_id,
                    msg_id,
                    "🎭 <b>سرگرمی، فال حافظ و بازی‌های امیر تولز:</b>",
                    Some(kb),
                )
                .await?;
        }
        "fun:fal" => {
            let kb: Keyboard = vec![
                vec![InlineBtn::cb("📖 نیت مجدد و فال جدید", "fun:fal")],
                vec![InlineBtn::cb("🏠 منوی اصلی", "menu:home")],
            ];
            app.tg
                .edit_html(chat_id, msg_id, &tools::hafez_fal(), Some(kb))
                .await?;
        }
        "fun:joke" => {
            let kb: Keyboard = vec![
                vec![InlineBtn::cb("😄 جوک بعدی", "fun:joke")],
                vec![InlineBtn::cb("🏠 منوی اصلی", "menu:home")],
            ];
            app.tg
                .edit_html(chat_id, msg_id, &tools::persian_joke(), Some(kb))
                .await?;
        }
        "fun:fact" => {
            let kb: Keyboard = vec![
                vec![InlineBtn::cb("💡 دانستنی بعدی", "fun:fact")],
                vec![InlineBtn::cb("🏠 منوی اصلی", "menu:home")],
            ];
            app.tg
                .edit_html(chat_id, msg_id, &tools::persian_fact(), Some(kb))
                .await?;
        }
        "fun:riddle" => {
            let kb: Keyboard = vec![
                vec![InlineBtn::cb("🧩 معمای بعدی", "fun:riddle")],
                vec![InlineBtn::cb("🏠 منوی اصلی", "menu:home")],
            ];
            app.tg
                .edit_html(chat_id, msg_id, &tools::persian_riddle(), Some(kb))
                .await?;
        }
        "fun:quote" => {
            let kb: Keyboard = vec![
                vec![InlineBtn::cb("✨ جمله بعدی", "fun:quote")],
                vec![InlineBtn::cb("🏠 منوی اصلی", "menu:home")],
            ];
            app.tg
                .edit_html(chat_id, msg_id, &tools::persian_quote(), Some(kb))
                .await?;
        }
        "fun:rps" => {
            let kb: Keyboard = vec![
                vec![
                    InlineBtn::cb("🪨 سنگ", "rps:سنگ"),
                    InlineBtn::cb("📄 کاغذ", "rps:کاغذ"),
                    InlineBtn::cb("✂️ قیچی", "rps:قیچی"),
                ],
                vec![InlineBtn::cb("🏠 منوی اصلی", "menu:home")],
            ];
            app.tg
                .edit_html(
                    chat_id,
                    msg_id,
                    "🎮 <b>بازی سنگ، کاغذ، قیچی:</b>\nیکی را انتخاب کنید:",
                    Some(kb),
                )
                .await?;
        }
        "menu:group" => {
            app.tg
                .edit_html(
                    chat_id,
                    msg_id,
                    "🛡 <b>سیستم ضد اسپم و مدیریت گروه امیر تولز:</b>\n\n\
                     ۱. ربات را در گروه خود ادمین کنید.\n\
                     ۲. دستور <code>/settings</code> یا <code>تنظیمات گروه</code> را در گروه بفرستید تا پنل شیشه‌ای قفل‌ها باز شود.\n\
                     ۳. دستورات سریع ادمین در گروه:\n\
                     • <code>قفل لینک</code> / <code>بازکردن لینک</code>\n\
                     • <code>قفل فروارد</code> / <code>بازکردن فروارد</code>\n\
                     • ریپلای روی پیام کاربر: <code>بن</code>، <code>حذف بن</code>، <code>سکوت</code>، <code>حذف سکوت</code>، <code>سنجاق</code>\n\
                     • محدودیت خودکار ۱۰۰ کاراکتر برای اعضای عادی (ادمین‌ها و مالک ربات معاف هستند).",
                    Some(back_home_kb()),
                )
                .await?;
        }
        "menu:anon" => {
            let tok = app.store.get_or_create_anon_token(user_id).await;
            let link = format!("https://t.me/{}?start={}", app.cfg.bot_username, tok);
            app.tg
                .edit_html(
                    chat_id,
                    msg_id,
                    &format!(
                        "💌 <b>لینک اختصاصی چت ناشناس شما:</b>\n\n\
                         🔗 <code>{}</code>\n\n\
                         <i>هرکس روی این لینک کلیک کند می‌تواند به صورت کاملاً ناشناس به شما پیام بفرستد و شما هم می‌توانید با دکمه شیشه‌ای به او پاسخ دهید!</i>",
                        link
                    ),
                    Some(back_home_kb()),
                )
                .await?;
        }
        "menu:admin" | "adm:stats" if is_owner => {
            let (users, groups, msgs, dls) = app.store.stats().await;
            let kb: Keyboard = vec![
                vec![InlineBtn::cb("📢 ارسال پیام همگانی", "adm:broadcast")],
                vec![InlineBtn::cb("🏠 منوی اصلی", "menu:home")],
            ];
            app.tg
                .edit_html(
                    chat_id,
                    msg_id,
                    &format!(
                        "👑 <b>پنل مدیریت کل امیر تولز:</b>\n\n\
                         • کاربران کل: <b>{}</b>\n\
                         • گروه‌های فعال: <b>{}</b>\n\
                         • کل پیام‌ها: <b>{}</b>\n\
                         • کل دانلودها: <b>{}</b>\n\n\
                         💻 دستورات ادمین: <code>/sh</code>، <code>/dl</code>، <code>/ghostban</code>، <code>/unghostban</code>، <code>/banuser</code>، <code>/addrule</code>",
                        users, groups, msgs, dls
                    ),
                    Some(kb),
                )
                .await?;
        }
        "adm:broadcast" if is_owner => {
            app.store
                .set_state(user_id, Some("wait_broadcast".into()))
                .await;
            app.tg
                .edit_html(
                    chat_id,
                    msg_id,
                    "📢 <b>متن پیام همگانی را ارسال کنید تا برای تمامی کاربران ربات ارسال شود:</b>",
                    Some(back_home_kb()),
                )
                .await?;
        }
        _ => {}
    }

    Ok(())
}

// ============================================================================
// HELPER PARSERS
// ============================================================================

fn parse_cmd(text: &str) -> (String, String) {
    if !text.starts_with('/') {
        return (String::new(), String::new());
    }
    let mut parts = text.splitn(2, char::is_whitespace);
    let first = parts.next().unwrap_or("");
    let cmd = first
        .split('@')
        .next()
        .unwrap_or(first)
        .to_lowercase();
    let arg = parts.next().unwrap_or("").trim().to_string();
    (cmd, arg)
}

/// Detects natural Persian requests asking the bot to search the web and download a file.
fn extract_natural_file_hunt_query(text: &str) -> Option<String> {
    let prefixes = [
        "پیدا کن ",
        "دانلود فایل ",
        "فایل ",
        "بگرد دنبال ",
        "شکار فایل ",
        "دانلود کتاب ",
        "دانلود برنامه ",
        "دانلود آهنگ ",
        "دانلود اهنگ ",
    ];
    for p in prefixes {
        if let Some(rest) = text.strip_prefix(p) {
            let q = rest
                .trim_end_matches("رو بفرست")
                .trim_end_matches("را بفرست")
                .trim_end_matches("رو دانلود کن")
                .trim();
            if q.chars().count() >= 2 {
                return Some(q.to_string());
            }
        }
    }
    None
}
