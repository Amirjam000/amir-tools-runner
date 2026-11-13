//! Autonomous Web File Hunter & Search Agent (`src/agent.rs`).
//!
//! Capabilities:
//! 1. Searches DuckDuckGo HTML, Internet Archive (`archive.org`), and specialized
//!    portals (Persian music sites, GitHub releases, document repositories).
//! 2. Crawls the top result landing pages concurrently with `scraper` to extract
//!    direct downloadable links (`.pdf`, `.mp3`, `.mp4`, `.apk`, `.zip`, `.rar`,
//!    `.epub`, `.docx`, `.pptx`, `.exe`, `.srt`, etc.) from `<a href>`, `<audio>`,
//!    and `<source>` tags.
//! 3. Streams the discovered file to disk with size & HTML-trap verification and
//!    uploads it directly to the user in Telegram (`sendAudio`, `sendVideo`, or
//!    `sendDocument`), while also presenting one-click buttons for all other
//!    discovered files.

use crate::config::Config;
use crate::db::Store;
use crate::tg::{html_escape, truncate_str, InlineBtn, Keyboard, TgClient};
use anyhow::{Context, Result};
use futures_util::StreamExt;
use regex::Regex;
use reqwest::Url;
use scraper::{Html, Selector};
use serde_json::Value;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;

pub const FILE_EXTS: &[&str] = &[
    "pdf", "epub", "mobi", "djvu", "mp3", "m4a", "flac", "ogg", "wav", "opus", "mp4", "mkv",
    "webm", "mov", "apk", "xapk", "zip", "rar", "7z", "tar", "gz", "xz", "tgz", "doc", "docx",
    "xls", "xlsx", "ppt", "pptx", "csv", "txt", "srt", "exe", "msi", "deb", "dmg", "iso",
    "torrent",
];

#[derive(Debug, Clone)]
pub struct FoundFile {
    pub title: String,
    pub url: String,
    pub ext: String,
    #[allow(dead_code)]
    pub source_page: String,
}

#[derive(Debug, Clone)]
pub struct WebResult {
    pub title: String,
    pub url: String,
    #[allow(dead_code)]
    pub snippet: String,
}

fn build_agent_http() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(18))
        .connect_timeout(Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::limited(6))
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
        )
        .build()
        .unwrap_or_default()
}

/// Checks if a URL path ends with a known downloadable file extension.
pub fn file_ext_of_url(raw: &str) -> Option<String> {
    let parsed = Url::parse(raw).ok()?;
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let path = parsed.path().to_lowercase();
    let last_seg = path.rsplit('/').next()?;
    let (_, ext) = last_seg.rsplit_once('.')?;
    if FILE_EXTS.contains(&ext) {
        Some(ext.to_string())
    } else {
        None
    }
}

/// Detects if the user's query hints at a specific extension (e.g. `mp3`, `pdf`, `apk`, `zip`).
fn detect_preferred_ext(query: &str) -> Option<&'static str> {
    let ql = query.to_lowercase();
    if ql.contains("pdf") || ql.contains("پی دی اف") || ql.contains("کتاب") || ql.contains("مقاله") || ql.contains("جزوه") {
        Some("pdf")
    } else if ql.contains("mp3") || ql.contains("آهنگ") || ql.contains("اهنگ") || ql.contains("موزیک") || ql.contains("ترانه") || ql.contains("مداحی") {
        Some("mp3")
    } else if ql.contains("apk") || ql.contains("اندروید") || ql.contains("برنامه") || ql.contains("اپلیکیشن") {
        Some("apk")
    } else if ql.contains("zip") || ql.contains("rar") || ql.contains("فشرده") || ql.contains("سورس") {
        Some("zip")
    } else if ql.contains("mp4") || ql.contains("ویدیو") || ql.contains("فیلم") || ql.contains("کلیپ") {
        Some("mp4")
    } else if ql.contains("srt") || ql.contains("زیرنویس") {
        Some("srt")
    } else if ql.contains("epub") {
        Some("epub")
    } else {
        None
    }
}

/// Searches DuckDuckGo HTML endpoint and returns web results.
pub async fn ddg_search(http: &reqwest::Client, query: &str, max: usize) -> Vec<WebResult> {
    let mut out = Vec::new();
    let url = format!(
        "https://html.duckduckgo.com/html/?q={}",
        urlencoding::encode(query)
    );
    let html = match http.get(&url).send().await {
        Ok(r) => r.text().await.unwrap_or_default(),
        Err(_) => return out,
    };

    let doc = Html::parse_document(&html);
    let sel_res = Selector::parse(".result").unwrap();
    let sel_a = Selector::parse("a.result__a").unwrap();
    let sel_snip = Selector::parse(".result__snippet").unwrap();

    for node in doc.select(&sel_res) {
        let Some(a) = node.select(&sel_a).next() else {
            continue;
        };
        let Some(href) = a.value().attr("href") else {
            continue;
        };
        let real_url = unwrap_ddg_redirect(href);
        if !real_url.starts_with("http") {
            continue;
        }
        let title: String = a.text().collect::<Vec<_>>().join(" ").trim().to_string();
        let snippet: String = node
            .select(&sel_snip)
            .next()
            .map(|n| n.text().collect::<Vec<_>>().join(" ").trim().to_string())
            .unwrap_or_default();

        out.push(WebResult {
            title: if title.is_empty() {
                real_url.clone()
            } else {
                title
            },
            url: real_url,
            snippet,
        });
        if out.len() >= max {
            break;
        }
    }
    out
}

fn unwrap_ddg_redirect(href: &str) -> String {
    let full = if href.starts_with("//") {
        format!("https:{}", href)
    } else {
        href.to_string()
    };
    if let Ok(u) = Url::parse(&full) {
        for (k, v) in u.query_pairs() {
            if k == "uddg" {
                return v.into_owned();
            }
        }
    }
    full
}

/// Searches Internet Archive (`archive.org`) for direct downloadable files.
async fn archive_org_hunt(
    http: &reqwest::Client,
    query: &str,
    preferred_ext: Option<&str>,
) -> Vec<FoundFile> {
    let mut found = Vec::new();
    let search_url = format!(
        "https://archive.org/advancedsearch.php?q={}&fl[]=identifier&fl[]=title&rows=4&page=1&output=json",
        urlencoding::encode(query)
    );
    let Ok(resp) = http.get(&search_url).timeout(Duration::from_secs(10)).send().await else {
        return found;
    };
    let Ok(val) = resp.json::<Value>().await else {
        return found;
    };
    let Some(docs) = val["response"]["docs"].as_array() else {
        return found;
    };

    for d in docs.iter().take(3) {
        let Some(id) = d["identifier"].as_str() else {
            continue;
        };
        let item_title = d["title"].as_str().unwrap_or(id).to_string();
        let meta_url = format!("https://archive.org/metadata/{}", urlencoding::encode(id));
        let Ok(mresp) = http.get(&meta_url).timeout(Duration::from_secs(8)).send().await else {
            continue;
        };
        let Ok(mval) = mresp.json::<Value>().await else {
            continue;
        };
        let Some(files) = mval["files"].as_array() else {
            continue;
        };
        for f in files {
            let Some(name) = f["name"].as_str() else {
                continue;
            };
            let dl_url = format!(
                "https://archive.org/download/{}/{}",
                urlencoding::encode(id),
                urlencoding::encode(name).replace("%2F", "/")
            );
            if let Some(ext) = file_ext_of_url(&dl_url) {
                if let Some(pref) = preferred_ext {
                    if ext != pref {
                        continue;
                    }
                }
                found.push(FoundFile {
                    title: format!("{} ({})", item_title, name),
                    url: dl_url,
                    ext,
                    source_page: format!("https://archive.org/details/{}", id),
                });
                if found.len() >= 5 {
                    return found;
                }
            }
        }
    }
    found
}

/// Crawls a single HTML landing page and extracts all direct downloadable file links
/// from `<a href>`, `<audio src>`, `<source src>`, and raw URL regex matches.
async fn scrape_page_for_files(
    http: &reqwest::Client,
    page_url: &str,
    page_title: &str,
) -> Vec<FoundFile> {
    let mut out = Vec::new();
    let Ok(base) = Url::parse(page_url) else {
        return out;
    };

    let resp = match http
        .get(page_url)
        .timeout(Duration::from_secs(10))
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => return out,
    };

    // If the URL itself redirected to a binary file (Content-Type != html), return it directly!
    let ctype = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();

    let final_url = resp.url().to_string();
    if !ctype.contains("html") && !ctype.contains("text/plain") {
        if let Some(ext) = file_ext_of_url(&final_url) {
            out.push(FoundFile {
                title: page_title.to_string(),
                url: final_url,
                ext,
                source_page: page_url.to_string(),
            });
            return out;
        }
    }

    let Ok(html) = resp.text().await else {
        return out;
    };
    let doc = Html::parse_document(&html);

    // 1. Scan `<a href="...">`
    if let Ok(sel_a) = Selector::parse("a[href]") {
        for a in doc.select(&sel_a) {
            let Some(href) = a.value().attr("href") else {
                continue;
            };
            let Ok(resolved) = base.join(href.trim()) else {
                continue;
            };
            let abs = resolved.to_string();
            if let Some(ext) = file_ext_of_url(&abs) {
                let anchor_text: String = a.text().collect::<Vec<_>>().join(" ").trim().to_string();
                let label = if !anchor_text.is_empty() && anchor_text.len() > 2 {
                    format!("{} — {}", page_title, anchor_text)
                } else {
                    resolved
                        .path_segments()
                        .and_then(|s| s.last())
                        .map(|n| urlencoding::decode(n).unwrap_or_default().into_owned())
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| page_title.to_string())
                };
                out.push(FoundFile {
                    title: truncate_str(&label, 85),
                    url: abs,
                    ext,
                    source_page: page_url.to_string(),
                });
                if out.len() >= 10 {
                    return out;
                }
            }
        }
    }

    // 2. Scan `<audio src="...">` and `<source src="...">` (very common on music & video sites)
    if let Ok(sel_media) = Selector::parse("audio[src], source[src], video[src]") {
        for m in doc.select(&sel_media) {
            let Some(src) = m.value().attr("src") else {
                continue;
            };
            let Ok(resolved) = base.join(src.trim()) else {
                continue;
            };
            let abs = resolved.to_string();
            if let Some(ext) = file_ext_of_url(&abs) {
                out.push(FoundFile {
                    title: truncate_str(page_title, 85),
                    url: abs,
                    ext,
                    source_page: page_url.to_string(),
                });
                if out.len() >= 10 {
                    return out;
                }
            }
        }
    }

    // 3. Regex scan for direct `.mp3` / `.pdf` / `.apk` / `.zip` / `.mp4` URLs embedded in JS/HTML
    static RAW_FILE_RE: OnceLock<Regex> = OnceLock::new();
    let re = RAW_FILE_RE.get_or_init(|| {
        Regex::new(
            r#"(?i)https?://[^\s"'<>\\]+\.(?:mp3|pdf|apk|zip|rar|mp4|epub|docx|pptx|srt)(?:\?[^\s"'<>\\]*)?"#,
        )
        .unwrap()
    });
    for m in re.find_iter(&html) {
        let u = m.as_str().to_string();
        if let Some(ext) = file_ext_of_url(&u) {
            out.push(FoundFile {
                title: truncate_str(page_title, 85),
                url: u,
                ext,
                source_page: page_url.to_string(),
            });
            if out.len() >= 10 {
                break;
            }
        }
    }

    out
}

/// Runs the full multi-stage file hunter agent and returns `(discovered_files, trace_steps)`.
pub async fn hunt_files(query: &str) -> (Vec<FoundFile>, Vec<String>) {
    let http = build_agent_http();
    let mut trace = Vec::new();
    let pref_ext = detect_preferred_ext(query);

    trace.push(format!(
        "🔍 تحلیل درخواست: «{}» (فرمت هدف: {})",
        query,
        pref_ext.unwrap_or("همه فرمت‌ها")
    ));

    // Build smart search queries
    let mut queries = vec![query.to_string()];
    if let Some(ext) = pref_ext {
        if ext == "mp3" {
            queries.push(format!("دانلود آهنگ {} mp3 320", query));
        } else if ext == "pdf" {
            queries.push(format!("{} filetype:pdf OR دانلود pdf", query));
        } else if ext == "apk" {
            queries.push(format!("دانلود {} apk آخرین نسخه", query));
        } else {
            queries.push(format!("{} download {}", query, ext));
        }
    } else {
        queries.push(format!("دانلود مستقیم {}", query));
    }

    let mut pages: Vec<WebResult> = Vec::new();
    let mut seen_pages = HashSet::new();
    for q in &queries {
        let res = ddg_search(&http, q, 6).await;
        for r in res {
            if seen_pages.insert(r.url.clone()) {
                pages.push(r);
            }
        }
        if pages.len() >= 8 {
            break;
        }
    }
    trace.push(format!(
        "🌐 {} صفحه وب مرتبط از موتورهای جستجو استخراج شد.",
        pages.len()
    ));

    let mut files: Vec<FoundFile> = Vec::new();
    let mut seen_files = HashSet::new();

    // 1. Check if any search result URL is already a direct file link
    for p in &pages {
        if let Some(ext) = file_ext_of_url(&p.url) {
            if seen_files.insert(p.url.clone()) {
                files.push(FoundFile {
                    title: p.title.clone(),
                    url: p.url.clone(),
                    ext,
                    source_page: p.url.clone(),
                });
            }
        }
    }

    // 2. Crawl top 6 landing pages concurrently to extract inner download links
    let crawl_targets: Vec<WebResult> = pages.into_iter().take(6).collect();
    if !crawl_targets.is_empty() {
        trace.push(format!(
            "🕷 در حال خزش موازی در {} سایت برای استخراج لینک مستقیم فایل...",
            crawl_targets.len()
        ));
        let futs = crawl_targets
            .iter()
            .map(|p| scrape_page_for_files(&http, &p.url, &p.title));
        let batch = futures_util::future::join_all(futs).await;
        for list in batch {
            for f in list {
                if seen_files.insert(f.url.clone()) {
                    files.push(f);
                }
            }
        }
    }

    // 3. Also query Archive.org in parallel if we still have few results
    if files.len() < 4 {
        trace.push("🏛 جستجو در مخزن جهانی Archive.org...".to_string());
        for f in archive_org_hunt(&http, query, pref_ext).await {
            if seen_files.insert(f.url.clone()) {
                files.push(f);
            }
        }
    }

    // Rank preferred extension first (and prefer 320.mp3 over 128.mp3 if mp3)
    if let Some(pref) = pref_ext {
        files.sort_by_key(|f| {
            let ext_match = if f.ext == pref { 0 } else { 1 };
            let quality_bonus = if f.url.contains("320") { 0 } else { 1 };
            (ext_match, quality_bonus)
        });
    }

    trace.push(format!(
        "✅ مجموعاً {} لینک دانلود مستقیم فایل کشف شد.",
        files.len()
    ));

    (files, trace)
}

/// Full interactive execution of the Autonomous Web File Hunter Agent:
/// 1. Shows live progress
/// 2. Crawls the web for direct files matching `query`
/// 3. Automatically downloads the top file and sends it to the user in Telegram
/// 4. Displays buttons to download any of the other discovered files with 1 click!
pub async fn run_file_hunter_agent(
    tg: &TgClient,
    cfg: &Config,
    store: &Store,
    chat_id: i64,
    user_id: i64,
    reply_to: i64,
    query: &str,
) -> Result<()> {
    let t0 = Instant::now();
    let status = tg
        .send_html(
            chat_id,
            Some(reply_to),
            &format!(
                "🕵️ <b>ایجنت هوشمند شکارچی فایل فعال شد...</b>\n\
                 🔎 هدف: <code>{}</code>\n\
                 <i>در حال جستجوی وب، باز کردن سایت‌ها و استخراج لینک مستقیم فایل...</i>",
                html_escape(query)
            ),
            None,
        )
        .await?;

    let (files, mut trace) = hunt_files(query).await;

    if files.is_empty() {
        // Fallback: if it looks like a song or video, offer one-click yt-dlp download!
        let key = store.stash_url(&format!("ytsearch1:{}", query)).await;
        let kb: Keyboard = vec![
            vec![
                InlineBtn::cb("🎵 دانلود به عنوان موزیک (MP3)", format!("ytdl:mp3:{}", key)),
                InlineBtn::cb("🎬 دانلود به عنوان ویدیو (720p)", format!("ytdl:v720:{}", key)),
            ],
            vec![InlineBtn::cb("🏠 منوی اصلی", "menu:home")],
        ];
        tg.edit_html(
            chat_id,
            status.message_id,
            &format!(
                "⚠️ <b>فایل مستقیمی با پسوند استاندارد در صفحات وب اول برای «{}» یافت نشد.</b>\n\n\
                 💡 اما می‌توانید همین عبارت را مستقیماً با موتور <code>yt-dlp</code> به صورت موزیک (MP3) یا ویدیو دانلود کنید:",
                html_escape(query)
            ),
            Some(kb),
        )
        .await?;
        return Ok(());
    }

    // Build buttons for up to 8 discovered files
    let mut kb: Keyboard = Vec::new();
    let mut lines: Vec<String> = Vec::new();
    for (idx, f) in files.iter().take(8).enumerate() {
        let tok = store.stash_url(&f.url).await;
        lines.push(format!(
            "{}. <b>[{}]</b> {} \n   🔗 <a href=\"{}\">لینک مستقیم</a>",
            idx + 1,
            f.ext.to_uppercase(),
            html_escape(&truncate_str(&f.title, 65)),
            html_escape(&f.url)
        ));
        let btn_title = format!(
            "📥 دانلود #{} ({}) — {}",
            idx + 1,
            f.ext.to_uppercase(),
            truncate_str(&f.title, 26)
        );
        kb.push(vec![InlineBtn::cb(btn_title, format!("fhdl:{}", tok))]);
    }
    kb.push(vec![InlineBtn::cb("🏠 منوی اصلی", "menu:home")]);

    let _ = tg
        .edit_html(
            chat_id,
            status.message_id,
            &format!(
                "🕵️ <b>ایجنت {} فایل قابل دانلود پیدا کرد!</b>\n\
                 ⏳ <i>در حال دانلود خودکار بهترین گزینه و ارسال به تلگرام شما...</i>\n\n{}",
                files.len(),
                lines.join("\n\n")
            ),
            Some(kb.clone()),
        )
        .await;

    // Automatically try downloading the top 3 candidates until one succeeds and uploads to Telegram!
    let mut auto_sent = false;
    for (idx, candidate) in files.iter().take(3).enumerate() {
        trace.push(format!(
            "⬇️ تلاش برای دانلود خودکار گزینه #{}: {}",
            idx + 1,
            candidate.url
        ));
        match download_and_upload_direct_url(
            tg,
            cfg,
            store,
            chat_id,
            user_id,
            Some(reply_to),
            &candidate.url,
            Some(&candidate.title),
        )
        .await
        {
            Ok(()) => {
                trace.push(format!(
                    "✅ گزینه #{} با موفقیت دانلود و به تلگرام ارسال شد!",
                    idx + 1
                ));
                auto_sent = true;
                break;
            }
            Err(e) => {
                trace.push(format!("⚠️ گزینه #{} رد شد: {}", idx + 1, e));
            }
        }
    }

    let elapsed = t0.elapsed().as_secs_f32();
    let trace_block = format!(
        "<blockquote expandable>⏱ {:.1}s • 🕵️ مراحل کار ایجنت شکارچی فایل\n{}</blockquote>",
        elapsed,
        html_escape(&trace.join("\n"))
    );

    let header = if auto_sent {
        format!(
            "✅ <b>ایجنت فایل «{}» را پیدا کرد و برایتان فرستاد!</b>\n\
             👇 <i>سایر فایل‌های پیدا شده در سایت‌ها (با کلیک روی هر دکمه، ربات آن را هم دانلود و ارسال می‌کند):</i>",
            html_escape(query)
        )
    } else {
        format!(
            "📂 <b>فایل‌های کشف شده برای «{}»:</b>\n\
             👇 <i>روی هر کدام کلیک کنید تا ربات آن را روی سرور دانلود کرده و برایتان بفرستد:</i>",
            html_escape(query)
        )
    };

    let final_report = format!("{}\n\n{}\n\n{}", header, lines.join("\n\n"), trace_block);
    let _ = tg
        .edit_html(chat_id, status.message_id, &final_report, Some(kb))
        .await;

    Ok(())
}

/// Streams a remote file URL to disk (verifying it's not an HTML page and obeys
/// the upload size limit) and uploads it directly to Telegram as Audio, Video, or Document.
pub async fn download_and_upload_direct_url(
    tg: &TgClient,
    cfg: &Config,
    store: &Store,
    chat_id: i64,
    user_id: i64,
    reply_to: Option<i64>,
    url: &str,
    custom_title: Option<&str>,
) -> Result<()> {
    let http = build_agent_http();
    let resp = http
        .get(url)
        .timeout(Duration::from_secs(180))
        .send()
        .await
        .context("عدم پاسخ از سرور مقصد")?;

    if !resp.status().is_success() {
        anyhow::bail!("کد وضعیت HTTP {}", resp.status().as_u16());
    }

    let ctype = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();

    // Reject HTML pages masquerading as download links
    if ctype.contains("text/html") {
        anyhow::bail!("لینک به یک صفحه وب HTML اشاره دارد، نه فایل باینری");
    }

    if let Some(clen) = resp.content_length() {
        if clen > cfg.max_upload_bytes() {
            anyhow::bail!(
                "حجم فایل ({:.1} MB) بیش از سقف {} MB است",
                clen as f64 / 1_048_576.0,
                cfg.max_upload_mb
            );
        }
        if clen < 512 {
            anyhow::bail!("فایل بیش از حد کوچک است ({} بایت)", clen);
        }
    }

    let filename = derive_filename(resp.url(), &ctype);
    let tmp_path: PathBuf = cfg.tmp_dir().join(format!(
        "dl_{}_{}_{}",
        user_id,
        chrono::Utc::now().timestamp_millis().unsigned_abs(),
        filename
    ));

    let mut file = tokio::fs::File::create(&tmp_path).await?;
    let mut stream = resp.bytes_stream();
    let mut total: u64 = 0;
    let max_bytes = cfg.max_upload_bytes();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        total += chunk.len() as u64;
        if total > max_bytes {
            drop(file);
            let _ = tokio::fs::remove_file(&tmp_path).await;
            anyhow::bail!("حجم فایل از سقف {} MB عبور کرد", cfg.max_upload_mb);
        }
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    drop(file);

    if total < 512 {
        let _ = tokio::fs::remove_file(&tmp_path).await;
        anyhow::bail!("فایل دانلود شده خالی یا ناقص بود");
    }

    let ext = tmp_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    let (action, method, field) = match ext.as_str() {
        "mp3" | "m4a" | "ogg" | "flac" | "wav" | "opus" => ("upload_voice", "sendAudio", "audio"),
        "mp4" | "mkv" | "webm" | "mov" => ("upload_video", "sendVideo", "video"),
        "jpg" | "jpeg" | "png" | "webp" => ("upload_photo", "sendPhoto", "photo"),
        _ => ("upload_document", "sendDocument", "document"),
    };

    tg.send_action(chat_id, action).await;

    let title_str = custom_title.unwrap_or(&filename);
    let caption = format!(
        "📦 <b>{}</b>\n\
         📁 نام فایل: <code>{}</code>\n\
         📐 حجم: <code>{:.2} MB</code>\n\
         🕵️ <i>ارسال شده توسط ایجنت شکارچی فایل امیر تولز 🦀</i>",
        html_escape(title_str),
        html_escape(&filename),
        total as f64 / 1_048_576.0
    );

    let send_res = tg
        .send_file(chat_id, reply_to, &tmp_path, &caption, method, field, None)
        .await;
    let _ = tokio::fs::remove_file(&tmp_path).await;
    send_res?;
    store.inc_download(user_id).await;
    Ok(())
}

/// Interactive callback handler when the user clicks `fhdl:<token>` or runs `/dl <url>`.
pub async fn handle_direct_download(
    tg: &TgClient,
    cfg: &Config,
    store: &Store,
    chat_id: i64,
    user_id: i64,
    reply_to: Option<i64>,
    url: &str,
) -> Result<()> {
    let wait = tg
        .send_html(
            chat_id,
            reply_to,
            &format!(
                "⬇️ <b>در حال دانلود مستقیم فایل به سرور و آپلود در تلگرام...</b>\n🔗 <code>{}</code>",
                html_escape(url)
            ),
            None,
        )
        .await?;

    match download_and_upload_direct_url(tg, cfg, store, chat_id, user_id, reply_to, url, None).await
    {
        Ok(()) => {
            tg.delete(chat_id, wait.message_id).await;
        }
        Err(e) => {
            // Maybe the page itself has embedded files; try scraping it once!
            let http = build_agent_http();
            let inner = scrape_page_for_files(&http, url, "فایل استخراج شده").await;
            if let Some(first) = inner.first() {
                if download_and_upload_direct_url(
                    tg,
                    cfg,
                    store,
                    chat_id,
                    user_id,
                    reply_to,
                    &first.url,
                    Some(&first.title),
                )
                .await
                .is_ok()
                {
                    tg.delete(chat_id, wait.message_id).await;
                    return Ok(());
                }
            }
            tg.edit_html(
                chat_id,
                wait.message_id,
                &format!(
                    "❌ <b>خطا در دانلود مستقیم فایل:</b> {}\n🔗 <a href=\"{}\">باز کردن لینک در مرورگر</a>",
                    html_escape(&e.to_string()),
                    html_escape(url)
                ),
                None,
            )
            .await?;
        }
    }
    Ok(())
}

fn derive_filename(url: &Url, content_type: &str) -> String {
    let raw = url
        .path_segments()
        .and_then(|s| s.last())
        .filter(|s| !s.is_empty())
        .unwrap_or("download");
    let decoded = urlencoding::decode(raw)
        .map(|c| c.into_owned())
        .unwrap_or_else(|_| raw.to_string());
    let mut clean: String = decoded
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if clean.len() > 80 {
        clean = clean[clean.len() - 80..].to_string();
    }
    if !clean.contains('.') {
        let ext = if content_type.contains("pdf") {
            "pdf"
        } else if content_type.contains("mpeg") || content_type.contains("mp3") {
            "mp3"
        } else if content_type.contains("mp4") {
            "mp4"
        } else if content_type.contains("android") {
            "apk"
        } else if content_type.contains("zip") {
            "zip"
        } else {
            "bin"
        };
        clean = format!("{}.{}", clean, ext);
    }
    clean
}
