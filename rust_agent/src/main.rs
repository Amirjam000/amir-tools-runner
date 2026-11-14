use futures_util::future::join_all;
use futures_util::StreamExt;
use regex::Regex;
use reqwest::Client;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::io::Write;
use std::net::ToSocketAddrs;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::net::TcpStream;

const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36";

const SUPPORTED_EXTS: &[&str] = &[
    "pdf", "mp3", "m4a", "flac", "ogg", "wav", "mp4", "mkv", "webm", "mov",
    "apk", "zip", "rar", "7z", "tar", "gz", "epub", "mobi", "djvu", "docx",
    "doc", "pptx", "ppt", "xlsx", "xls", "csv", "srt", "exe", "msi", "dmg", "iso",
];

// ──────────────────── PURE RUST SHA-256 IMPLEMENTATION ────────────────────

struct Sha256 {
    state: [u32; 8],
    data: Vec<u8>,
    bit_len: u64,
}

impl Sha256 {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];

    fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
            ],
            data: Vec::with_capacity(64),
            bit_len: 0,
        }
    }

    fn update(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.data.push(b);
            if self.data.len() == 64 {
                let block: [u8; 64] = self.data.as_slice().try_into().unwrap();
                self.transform(&block);
                self.bit_len = self.bit_len.wrapping_add(512);
                self.data.clear();
            }
        }
    }

    fn transform(&mut self, block: &[u8; 64]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                block[i * 4],
                block[i * 4 + 1],
                block[i * 4 + 2],
                block[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(Self::K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }

    fn finalize_hex(mut self) -> String {
        let rem_bits = (self.data.len() as u64) * 8;
        self.bit_len = self.bit_len.wrapping_add(rem_bits);
        self.data.push(0x80);

        if self.data.len() > 56 {
            while self.data.len() < 64 {
                self.data.push(0);
            }
            let block: [u8; 64] = self.data.as_slice().try_into().unwrap();
            self.transform(&block);
            self.data.clear();
        }
        while self.data.len() < 56 {
            self.data.push(0);
        }
        self.data.extend_from_slice(&self.bit_len.to_be_bytes());
        let block: [u8; 64] = self.data.as_slice().try_into().unwrap();
        self.transform(&block);

        let mut out = String::with_capacity(64);
        for s in &self.state {
            out.push_str(&format!("{:08x}", s));
        }
        out
    }
}

// ──────────────────── DATA STRUCTURES ────────────────────

#[derive(Serialize, Deserialize, Clone, Debug)]
struct DownloadResult {
    success: bool,
    filename: String,
    filepath: String,
    size_bytes: u64,
    size_mb: f64,
    sha256: String,
    time_taken: f64,
    url: String,
    content_type: String,
    error: String,
}

fn build_client(timeout_secs: u64) -> Client {
    Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(timeout_secs))
        .redirect(reqwest::redirect::Policy::limited(6))
        .build()
        .unwrap_or_else(|_| Client::new())
}

fn detect_file_extension(query: &str, explicit_ext: &str) -> String {
    let exp = explicit_ext.trim().trim_start_matches('.').to_lowercase();
    if !exp.is_empty() && exp != "bin" {
        return exp;
    }
    let low = query.to_lowercase();
    for &e in SUPPORTED_EXTS {
        if low.contains(&format!(".{}", e)) || low.split_whitespace().any(|w| w == e) {
            return e.to_string();
        }
    }
    if ["pdf", "پی دی اف", "کتاب", "مقاله", "جزوه"].iter().any(|k| low.contains(k)) {
        return "pdf".to_string();
    }
    if ["mp3", "آهنگ", "اهنگ", "موزیک", "ترانه", "صوتی", "ریمیکس"].iter().any(|k| low.contains(k)) {
        return "mp3".to_string();
    }
    if ["apk", "برنامه", "اپلیکیشن", "اندروید"].iter().any(|k| low.contains(k)) {
        return "apk".to_string();
    }
    if ["zip", "rar", "فشرده", "سورس"].iter().any(|k| low.contains(k)) {
        return "zip".to_string();
    }
    if ["mp4", "ویدیو", "فیلم", "کلیپ"].iter().any(|k| low.contains(k)) {
        return "mp4".to_string();
    }
    if ["epub", "ایپاب"].iter().any(|k| low.contains(k)) {
        return "epub".to_string();
    }
    if ["srt", "زیرنویس"].iter().any(|k| low.contains(k)) {
        return "srt".to_string();
    }
    "pdf".to_string()
}

fn sanitize_filename(raw: &str) -> String {
    let re = Regex::new(r"[^\w\.\-\u0600-\u06FF]").unwrap();
    let cleaned = re.replace_all(raw.trim(), "_").to_string();
    if cleaned.is_empty() {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        format!("file_{}.bin", ts)
    } else {
        cleaned
    }
}

fn url_matches_downloadable_ext(url: &str, preferred_ext: &str) -> Option<String> {
    let base = url.split('?').next().unwrap_or(url).split('#').next().unwrap_or(url).to_lowercase();
    if !preferred_ext.is_empty() && base.ends_with(&format!(".{}", preferred_ext)) {
        return Some(preferred_ext.to_string());
    }
    for &ext in SUPPORTED_EXTS {
        if base.ends_with(&format!(".{}", ext)) {
            return Some(ext.to_string());
        }
    }
    None
}

fn resolve_relative_url(base_url: &str, href: &str) -> Option<String> {
    let h = href.trim();
    if h.starts_with("http://") || h.starts_with("https://") {
        return Some(h.to_string());
    }
    if h.starts_with("//") {
        return Some(format!("https:{}", h));
    }
    if let Ok(parsed) = reqwest::Url::parse(base_url) {
        if let Ok(joined) = parsed.join(h) {
            return Some(joined.to_string());
        }
    }
    None
}

// ──────────────────── STREAMING FILE DOWNLOADER ────────────────────

async fn stream_download_file(url: &str, out_dir: &str, max_size_mb: u64, preferred_ext: &str) -> DownloadResult {
    let t0 = Instant::now();
    let client = build_client(30);
    let _ = std::fs::create_dir_all(out_dir);

    let clean_path = url.split('?').next().unwrap_or(url);
    let raw_base = Path::new(clean_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("downloaded_file");
    let decoded_base = urlencoding::decode(raw_base).map(|s| s.into_owned()).unwrap_or_else(|_| raw_base.to_string());
    let mut filename = sanitize_filename(&decoded_base);

    let resp = match client.get(url).header("Accept", "*/*").send().await {
        Ok(r) => r,
        Err(e) => {
            return DownloadResult {
                success: false,
                filename,
                filepath: String::new(),
                size_bytes: 0,
                size_mb: 0.0,
                sha256: String::new(),
                time_taken: t0.elapsed().as_secs_f64(),
                url: url.to_string(),
                content_type: String::new(),
                error: format!("Request error: {}", e),
            };
        }
    };

    if !resp.status().is_success() {
        return DownloadResult {
            success: false,
            filename,
            filepath: String::new(),
            size_bytes: 0,
            size_mb: 0.0,
            sha256: String::new(),
            time_taken: t0.elapsed().as_secs_f64(),
            url: url.to_string(),
            content_type: String::new(),
            error: format!("HTTP {}", resp.status()),
        };
    }

    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();

    // Reject HTML landing pages masquerading as files
    if content_type.contains("text/html") || content_type.contains("application/xhtml") {
        return DownloadResult {
            success: false,
            filename,
            filepath: String::new(),
            size_bytes: 0,
            size_mb: 0.0,
            sha256: String::new(),
            time_taken: t0.elapsed().as_secs_f64(),
            url: url.to_string(),
            content_type,
            error: "Target URL returned HTML page instead of binary file".to_string(),
        };
    }

    if let Some(cd) = resp.headers().get("content-disposition").and_then(|v| v.to_str().ok()) {
        if let Ok(re_cd) = Regex::new(r#"(?i)filename\*?=(?:UTF-8'')?["']?([^"';\r\n]+)"#) {
            if let Some(caps) = re_cd.captures(cd) {
                if let Some(m) = caps.get(1) {
                    let dec = urlencoding::decode(m.as_str().trim()).map(|s| s.into_owned()).unwrap_or_else(|_| m.as_str().to_string());
                    let san = sanitize_filename(&dec);
                    if !san.is_empty() {
                        filename = san;
                    }
                }
            }
        }
    }

    if Path::new(&filename).extension().is_none() {
        let ext = if !preferred_ext.is_empty() && preferred_ext != "bin" {
            preferred_ext
        } else if content_type.contains("pdf") {
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
        filename = format!("{}.{}", filename, ext);
    }

    let max_bytes = max_size_mb * 1024 * 1024;
    if let Some(cl) = resp.content_length() {
        if cl > max_bytes {
            return DownloadResult {
                success: false,
                filename,
                filepath: String::new(),
                size_bytes: cl,
                size_mb: (cl as f64) / (1024.0 * 1024.0),
                sha256: String::new(),
                time_taken: t0.elapsed().as_secs_f64(),
                url: url.to_string(),
                content_type,
                error: format!("File exceeds {} MB limit", max_size_mb),
            };
        }
        if cl < 1024 {
            return DownloadResult {
                success: false,
                filename,
                filepath: String::new(),
                size_bytes: cl,
                size_mb: 0.0,
                sha256: String::new(),
                time_taken: t0.elapsed().as_secs_f64(),
                url: url.to_string(),
                content_type,
                error: "File too small (< 1 KB)".to_string(),
            };
        }
    }

    let filepath_buf = Path::new(out_dir).join(&filename);
    let filepath = filepath_buf.to_string_lossy().to_string();

    let mut file = match File::create(&filepath_buf) {
        Ok(f) => f,
        Err(e) => {
            return DownloadResult {
                success: false,
                filename,
                filepath,
                size_bytes: 0,
                size_mb: 0.0,
                sha256: String::new(),
                time_taken: t0.elapsed().as_secs_f64(),
                url: url.to_string(),
                content_type,
                error: format!("Disk write error: {}", e),
            };
        }
    };

    let mut hasher = Sha256::new();
    let mut total_bytes: u64 = 0;
    let mut first_chunk_checked = false;
    let mut stream = resp.bytes_stream();

    while let Some(item) = stream.next().await {
        match item {
            Ok(chunk) => {
                if !first_chunk_checked && !chunk.is_empty() {
                    first_chunk_checked = true;
                    let head_preview = String::from_utf8_lossy(&chunk[..chunk.len().min(256)]).to_lowercase();
                    if head_preview.contains("<!doctype html") || head_preview.contains("<html") {
                        drop(file);
                        let _ = std::fs::remove_file(&filepath_buf);
                        return DownloadResult {
                            success: false,
                            filename,
                            filepath,
                            size_bytes: 0,
                            size_mb: 0.0,
                            sha256: String::new(),
                            time_taken: t0.elapsed().as_secs_f64(),
                            url: url.to_string(),
                            content_type,
                            error: "Payload starts with <!DOCTYPE html>".to_string(),
                        };
                    }
                }
                total_bytes += chunk.len() as u64;
                if total_bytes > max_bytes {
                    drop(file);
                    let _ = std::fs::remove_file(&filepath_buf);
                    return DownloadResult {
                        success: false,
                        filename,
                        filepath,
                        size_bytes: total_bytes,
                        size_mb: (total_bytes as f64) / (1024.0 * 1024.0),
                        sha256: String::new(),
                        time_taken: t0.elapsed().as_secs_f64(),
                        url: url.to_string(),
                        content_type,
                        error: format!("File stream exceeded {} MB limit", max_size_mb),
                    };
                }
                hasher.update(&chunk);
                if let Err(e) = file.write_all(&chunk) {
                    drop(file);
                    let _ = std::fs::remove_file(&filepath_buf);
                    return DownloadResult {
                        success: false,
                        filename,
                        filepath,
                        size_bytes: total_bytes,
                        size_mb: 0.0,
                        sha256: String::new(),
                        time_taken: t0.elapsed().as_secs_f64(),
                        url: url.to_string(),
                        content_type,
                        error: format!("Write error: {}", e),
                    };
                }
            }
            Err(e) => {
                drop(file);
                let _ = std::fs::remove_file(&filepath_buf);
                return DownloadResult {
                    success: false,
                    filename,
                    filepath,
                    size_bytes: total_bytes,
                    size_mb: 0.0,
                    sha256: String::new(),
                    time_taken: t0.elapsed().as_secs_f64(),
                    url: url.to_string(),
                    content_type,
                    error: format!("Stream error: {}", e),
                };
            }
        }
    }

    if total_bytes < 1024 {
        drop(file);
        let _ = std::fs::remove_file(&filepath_buf);
        return DownloadResult {
            success: false,
            filename,
            filepath,
            size_bytes: total_bytes,
            size_mb: 0.0,
            sha256: String::new(),
            time_taken: t0.elapsed().as_secs_f64(),
            url: url.to_string(),
            content_type,
            error: "Downloaded file < 1 KB".to_string(),
        };
    }

    let elapsed = (t0.elapsed().as_secs_f64() * 100.0).round() / 100.0;
    let size_mb = ((total_bytes as f64) / (1024.0 * 1024.0) * 100.0).round() / 100.0;

    DownloadResult {
        success: true,
        filename,
        filepath,
        size_bytes: total_bytes,
        size_mb,
        sha256: hasher.finalize_hex(),
        time_taken: elapsed,
        url: url.to_string(),
        content_type,
        error: String::new(),
    }
}

// ──────────────────── CONCURRENT LANDING PAGE CRAWLER ────────────────────

async fn crawl_landing_page_for_files(client: Client, page_url: String, preferred_ext: String) -> Vec<String> {
    let mut found = Vec::new();
    let resp = match client.get(&page_url).timeout(Duration::from_secs(6)).send().await {
        Ok(r) if r.status().is_success() => r,
        _ => return found,
    };

    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();

    // If the landing page URL itself is a binary file!
    if !ct.is_empty() && !ct.contains("text/html") && !ct.contains("text/plain") && !ct.contains("json") {
        found.push(page_url);
        return found;
    }

    let body = match resp.text().await {
        Ok(t) => t,
        Err(_) => return found,
    };

    // 1. Parse HTML tags (<a href>, <audio src>, <video src>, <source src>)
    {
        let doc = Html::parse_document(&body);
        if let Ok(sel) = Selector::parse("a[href], audio[src], video[src], source[src]") {
            for el in doc.select(&sel) {
                let raw_link = el.value().attr("href").or_else(|| el.value().attr("src")).unwrap_or("");
                if let Some(full) = resolve_relative_url(&page_url, raw_link) {
                    if url_matches_downloadable_ext(&full, &preferred_ext).is_some() {
                        found.push(full);
                    }
                }
            }
        }
    }

    // 2. Regex scan for direct file links embedded in JS/HTML attributes
    let ext_pattern = SUPPORTED_EXTS.join("|");
    if let Ok(re_raw) = Regex::new(&format!(r#"https?://[^\s"'<>()]+?\.(?:{})(?:\?[^\s"'<>()]*)?"#, ext_pattern)) {
        for m in re_raw.find_iter(&body) {
            found.push(m.as_str().to_string());
        }
    }

    found
}

// ──────────────────── AUTONOMOUS WEB FILE HUNTER ────────────────────

async fn hunt_files_on_web(query: &str, explicit_ext: &str, out_dir: &str) -> serde_json::Value {
    let t0 = Instant::now();
    let ext = detect_file_extension(query, explicit_ext);
    let re_prefix = Regex::new(r"^(?:سرچ کن|پیدا کن|دانلود کن|دانلود فایل|دانلود کتاب|دانلود آهنگ|دانلود اهنگ|دانلود برنامه|دانلود|فایل|کتاب|آهنگ|اهنگ|برنامه|بگرد دنبال)\s*").unwrap();
    let re_suffix = Regex::new(r"\s*(?:رو برام بفرست|برام بفرست|پیدا کن و بفرست|رو پیدا کن|بفرست|دانلود کن)$").unwrap();
    let clean_q = re_suffix.replace(&re_prefix.replace(query.trim(), ""), "").trim().to_string();
    let clean_q = if clean_q.is_empty() { query.trim().to_string() } else { clean_q };

    let client = build_client(8);
    let mut direct_candidates: Vec<String> = Vec::new();
    let mut landing_pages: Vec<String> = Vec::new();

    // 1. If query itself contains a direct URL
    if let Ok(re_u) = Regex::new(r"https?://[^\s]+") {
        if let Some(m) = re_u.find(query) {
            direct_candidates.push(m.as_str().to_string());
        }
    }

    // 2. Specialized Music Portals (UpMusics & Music-FA) for MP3
    if ext == "mp3" {
        let music_portals = vec![
            format!("https://upmusics.com/?s={}", urlencoding::encode(&clean_q)),
            format!("https://music-fa.com/?s={}", urlencoding::encode(&clean_q)),
        ];
        let mut m_futs = Vec::new();
        for m_url in music_portals {
            let c = client.clone();
            m_futs.push(tokio::spawn(async move {
                let mut posts = Vec::new();
                if let Ok(r) = c.get(&m_url).send().await {
                    if let Ok(txt) = r.text().await {
                        if let Ok(re_p) = Regex::new(r#"<h2[^>]*>\s*<a\s+href="([^"]+)""#) {
                            for cap in re_p.captures_iter(&txt).take(3) {
                                if let Some(u) = cap.get(1) {
                                    posts.push(u.as_str().to_string());
                                }
                            }
                        }
                        if let Ok(re_p2) = Regex::new(r#"<a\s+href="([^"]+)"\s+rel="bookmark""#) {
                            for cap in re_p2.captures_iter(&txt).take(3) {
                                if let Some(u) = cap.get(1) {
                                    posts.push(u.as_str().to_string());
                                }
                            }
                        }
                    }
                }
                posts
            }));
        }
        for res in join_all(m_futs).await {
            if let Ok(posts) = res {
                for p in posts {
                    if !p.contains("/category/") {
                        landing_pages.push(p);
                    }
                }
            }
        }
    }

    // 3. Internet Archive for PDF / EPUB books
    if ext == "pdf" || ext == "epub" {
        let ar_url = format!(
            "https://archive.org/advancedsearch.php?q={}+mediatype:texts&output=json&rows=4",
            urlencoding::encode(&clean_q)
        );
        if let Ok(r) = client.get(&ar_url).send().await {
            if let Ok(v) = r.json::<serde_json::Value>().await {
                if let Some(docs) = v.pointer("/response/docs").and_then(|d| d.as_array()) {
                    for d in docs {
                        if let Some(iid) = d.get("identifier").and_then(|i| i.as_str()) {
                            direct_candidates.push(format!("https://archive.org/download/{}/{}.{}", iid, iid, ext));
                        }
                    }
                }
            }
        }
    }

    // 4. GitHub Releases API for APK / ZIP / EXE tools
    if ext == "apk" || ext == "zip" || ext == "exe" {
        let gh_url = format!(
            "https://api.github.com/search/repositories?q={}&per_page=3",
            urlencoding::encode(&clean_q)
        );
        if let Ok(r) = client.get(&gh_url).header("Accept", "application/vnd.github.v3+json").send().await {
            if let Ok(v) = r.json::<serde_json::Value>().await {
                if let Some(items) = v.get("items").and_then(|i| i.as_array()) {
                    for repo in items.iter().take(2) {
                        if let Some(full_name) = repo.get("full_name").and_then(|s| s.as_str()) {
                            let rel_url = format!("https://api.github.com/repos/{}/releases/latest", full_name);
                            if let Ok(rr) = client.get(&rel_url).send().await {
                                if let Ok(rv) = rr.json::<serde_json::Value>().await {
                                    if let Some(assets) = rv.get("assets").and_then(|a| a.as_array()) {
                                        for asset in assets {
                                            if let Some(dl_u) = asset.get("browser_download_url").and_then(|u| u.as_str()) {
                                                if url_matches_downloadable_ext(dl_u, &ext).is_some() {
                                                    direct_candidates.push(dl_u.to_string());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 5. Concurrent DuckDuckGo Search Queries
    let ddg_queries = vec![
        format!("دانلود {} با لینک مستقیم {}", clean_q, ext),
        format!("{} direct download .{}", clean_q, ext),
        format!("{} filetype:{}", clean_q, ext),
    ];

    let mut ddg_futs = Vec::new();
    for sq in ddg_queries {
        let c = client.clone();
        ddg_futs.push(tokio::spawn(async move {
            let mut found_direct = Vec::new();
            let mut found_pages = Vec::new();
            let u = format!("https://html.duckduckgo.com/html/?q={}", urlencoding::encode(&sq));
            if let Ok(r) = c.get(&u).send().await {
                if let Ok(html_txt) = r.text().await {
                    let doc = Html::parse_document(&html_txt);
                    if let Ok(sel) = Selector::parse("a[href]") {
                        let re_uddg = Regex::new(r"uddg=([^&]+)").unwrap();
                        for a in doc.select(&sel) {
                            let mut href = a.value().attr("href").unwrap_or("").to_string();
                            if href.contains("uddg=") {
                                if let Some(cap) = re_uddg.captures(&href) {
                                    if let Some(m) = cap.get(1) {
                                        href = urlencoding::decode(m.as_str()).map(|s| s.into_owned()).unwrap_or(href);
                                    }
                                }
                            }
                            if !href.starts_with("http") || href.contains("duckduckgo.com") {
                                continue;
                            }
                            if url_matches_downloadable_ext(&href, "").is_some() {
                                found_direct.push(href);
                            } else if found_pages.len() < 4 {
                                found_pages.push(href);
                            }
                        }
                    }
                }
            }
            (found_direct, found_pages)
        }));
    }

    for res in join_all(ddg_futs).await {
        if let Ok((d_list, p_list)) = res {
            direct_candidates.extend(d_list);
            for p in p_list {
                if landing_pages.len() < 8 {
                    landing_pages.push(p);
                }
            }
        }
    }

    // 6. Crawl up to 8 landing pages concurrently in Rust!
    let mut crawl_futs = Vec::new();
    let mut seen_pages = HashSet::new();
    for lp in landing_pages {
        if seen_pages.insert(lp.clone()) && seen_pages.len() <= 8 {
            let c = client.clone();
            let pref = ext.clone();
            crawl_futs.push(tokio::spawn(async move {
                crawl_landing_page_for_files(c, lp, pref).await
            }));
        }
    }
    for res in join_all(crawl_futs).await {
        if let Ok(links) = res {
            direct_candidates.extend(links);
        }
    }

    // Deduplicate and rank candidates (exact extension match first)
    let mut seen = HashSet::new();
    let mut exact_matches = Vec::new();
    let mut other_matches = Vec::new();
    for c in direct_candidates {
        let trimmed = c.trim().to_string();
        if trimmed.starts_with("http") && seen.insert(trimmed.clone()) {
            let base_low = trimmed.split('?').next().unwrap_or(&trimmed).to_lowercase();
            if base_low.ends_with(&format!(".{}", ext)) {
                exact_matches.push(trimmed);
            } else {
                other_matches.push(trimmed);
            }
        }
    }
    exact_matches.extend(other_matches);
    let ranked_candidates = exact_matches;

    // 7. Try downloading the top candidates (up to 4 tries) until a verified binary file succeeds
    let mut downloaded_res: Option<DownloadResult> = None;
    for cand_url in ranked_candidates.iter().take(4) {
        let dl = stream_download_file(cand_url, out_dir, 50, &ext).await;
        if dl.success {
            downloaded_res = Some(dl);
            break;
        }
    }

    let elapsed = (t0.elapsed().as_secs_f64() * 100.0).round() / 100.0;
    json!({
        "ok": true,
        "engine": "Rust Native Agent v6.0 (Tokio Async)",
        "query": clean_q,
        "detected_ext": ext,
        "elapsed_sec": elapsed,
        "candidates": ranked_candidates.into_iter().take(10).collect::<Vec<_>>(),
        "downloaded": downloaded_res
    })
}

// ──────────────────── CONCURRENT WEB SEARCH (DDG + WIKIPEDIA) ────────────────────

async fn rust_web_search(query: &str, max_results: usize) -> serde_json::Value {
    let client = build_client(8);
    let clean_q = query.trim().to_string();
    let mut snippets: Vec<String> = Vec::new();
    let mut sources: Vec<String> = Vec::new();

    // 1. DuckDuckGo HTML Search
    let ddg_url = format!("https://html.duckduckgo.com/html/?q={}", urlencoding::encode(&clean_q));
    if let Ok(r) = client.get(&ddg_url).send().await {
        if let Ok(raw_html) = r.text().await {
            let doc = Html::parse_document(&raw_html);
            if let (Ok(res_sel), Ok(a_sel), Ok(snip_sel)) = (
                Selector::parse("div.result"),
                Selector::parse("a.result__a"),
                Selector::parse("a.result__snippet, div.result__snippet"),
            ) {
                let re_uddg = Regex::new(r"uddg=([^&]+)").unwrap();
                for res_el in doc.select(&res_sel) {
                    if snippets.len() >= max_results {
                        break;
                    }
                    let t_opt = res_el.select(&a_sel).next();
                    let s_opt = res_el.select(&snip_sel).next();
                    if let (Some(t_el), Some(s_el)) = (t_opt, s_opt) {
                        let title = t_el.text().collect::<Vec<_>>().join(" ").trim().to_string();
                        let snippet = s_el.text().collect::<Vec<_>>().join(" ").trim().to_string();
                        let mut href = t_el.value().attr("href").unwrap_or("").to_string();
                        if href.contains("uddg=") {
                            if let Some(cap) = re_uddg.captures(&href) {
                                if let Some(m) = cap.get(1) {
                                    href = urlencoding::decode(m.as_str()).map(|s| s.into_owned()).unwrap_or(href);
                                }
                            }
                        }
                        if !title.is_empty() && !snippet.is_empty() {
                            snippets.push(format!("• {}: {}", title, snippet));
                            if href.starts_with("http") {
                                let short_t: String = title.chars().take(38).collect();
                                sources.push(format!("🔗 [{}]({})", short_t, href));
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. DuckDuckGo Instant Answer API fallback
    if snippets.is_empty() {
        let api_url = format!(
            "https://api.duckduckgo.com/?q={}&format=json&no_html=1",
            urlencoding::encode(&clean_q)
        );
        if let Ok(r) = client.get(&api_url).send().await {
            if let Ok(data) = r.json::<serde_json::Value>().await {
                let abstract_txt = data.get("AbstractText").and_then(|v| v.as_str()).unwrap_or("");
                let heading = data.get("Heading").and_then(|v| v.as_str()).unwrap_or(&clean_q);
                let abs_url = data.get("AbstractURL").and_then(|v| v.as_str()).unwrap_or("");
                if !abstract_txt.is_empty() {
                    snippets.push(format!("• {}: {}", heading, abstract_txt));
                    if !abs_url.is_empty() {
                        sources.push(format!("🔗 [{}]({})", heading, abs_url));
                    }
                }
            }
        }
    }

    // 3. Wikipedia FA Fallback
    if snippets.is_empty() {
        let w_url = format!(
            "https://fa.wikipedia.org/w/api.php?action=query&list=search&srsearch={}&format=json",
            urlencoding::encode(&clean_q)
        );
        if let Ok(r) = client.get(&w_url).send().await {
            if let Ok(data) = r.json::<serde_json::Value>().await {
                if let Some(hits) = data.pointer("/query/search").and_then(|v| v.as_array()) {
                    let re_tags = Regex::new(r"<[^>]+>").unwrap();
                    for h in hits.iter().take(max_results) {
                        let title = h.get("title").and_then(|v| v.as_str()).unwrap_or("");
                        let raw_s = h.get("snippet").and_then(|v| v.as_str()).unwrap_or("");
                        let clean_s = re_tags.replace_all(raw_s, "").to_string();
                        if !title.is_empty() {
                            snippets.push(format!("• {}: {}", title, clean_s));
                            sources.push(format!(
                                "🔗 [ویکی‌پدیا: {}](https://fa.wikipedia.org/wiki/{})",
                                title,
                                urlencoding::encode(title)
                            ));
                        }
                    }
                }
            }
        }
    }

    json!({
        "ok": true,
        "engine": "Rust Native Search",
        "snippets": snippets,
        "sources": sources
    })
}

// ──────────────────── NETWORK & DEVOPS TOOLS IN RUST ────────────────────

async fn rust_net_tool(mode: &str, target: &str, extra: &str) -> serde_json::Value {
    let client = build_client(8);
    let clean_target = target
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or(target.trim());

    match mode {
        "port" => {
            let port: u16 = extra.parse().unwrap_or(80);
            let addr = format!("{}:{}", clean_target, port);
            let t0 = Instant::now();
            match tokio::time::timeout(Duration::from_secs(4), TcpStream::connect(&addr)).await {
                Ok(Ok(_stream)) => {
                    let ms = (t0.elapsed().as_secs_f64() * 1000.0 * 10.0).round() / 10.0;
                    json!({
                        "ok": true,
                        "open": true,
                        "latency_ms": ms,
                        "report": format!("🟢 پورت <code>{}</code> روی میزبان <code>{}</code> <b>باز (OPEN)</b> است.\n⚡️ زمان هندشیک TCP (موتور Rust): <b>{} ms</b>", port, clean_target, ms)
                    })
                }
                _ => {
                    json!({
                        "ok": true,
                        "open": false,
                        "report": format!("🔴 پورت <code>{}</code> روی میزبان <code>{}</code> <b>بسته (CLOSED / FILTERED)</b> است یا فایروال مانع اتصال شد.", port, clean_target)
                    })
                }
            }
        }
        "dns" => {
            let mut ips = Vec::new();
            if let Ok(addrs) = (clean_target, 80u16).to_socket_addrs() {
                for a in addrs {
                    let ip_s = a.ip().to_string();
                    if !ips.contains(&ip_s) {
                        ips.push(ip_s);
                    }
                }
            }
            let doh_url = format!("https://dns.google/resolve?name={}&type=ANY", urlencoding::encode(clean_target));
            let mut records = Vec::new();
            if let Ok(r) = client.get(&doh_url).send().await {
                if let Ok(v) = r.json::<serde_json::Value>().await {
                    if let Some(ans) = v.get("Answer").and_then(|a| a.as_array()) {
                        for rec in ans.iter().take(8) {
                            let d = rec.get("data").and_then(|x| x.as_str()).unwrap_or("");
                            let t = rec.get("type").and_then(|x| x.as_u64()).unwrap_or(0);
                            let t_name = match t {
                                1 => "A",
                                2 => "NS",
                                5 => "CNAME",
                                15 => "MX",
                                16 => "TXT",
                                28 => "AAAA",
                                _ => "REC",
                            };
                            if !d.is_empty() {
                                records.push(format!("• [{}] <code>{}</code>", t_name, d));
                            }
                        }
                    }
                }
            }
            let ip_list = if ips.is_empty() { "یافت نشد".to_string() } else { ips.join(" , ") };
            let rec_list = if records.is_empty() { String::new() } else { format!("\n\n📋 <b>رکوردهای DNS:</b>\n{}", records.join("\n")) };
            json!({
                "ok": true,
                "ips": ips,
                "report": format!("🌐 <b>تحلیل DNS دامنه (Rust Engine):</b> <code>{}</code>\n📍 آدرس‌های IP: <code>{}</code>{}", clean_target, ip_list, rec_list)
            })
        }
        "http" => {
            let url = if target.starts_with("http://") || target.starts_with("https://") {
                target.to_string()
            } else {
                format!("https://{}", target.trim())
            };
            let t0 = Instant::now();
            match client.get(&url).send().await {
                Ok(r) => {
                    let ms = (t0.elapsed().as_secs_f64() * 1000.0 * 10.0).round() / 10.0;
                    let status = r.status().as_u16();
                    let reason = r.status().canonical_reason().unwrap_or("OK");
                    let srv = r.headers().get("server").and_then(|v| v.to_str().ok()).unwrap_or("نامشخص").to_string();
                    json!({
                        "ok": true,
                        "status": status,
                        "latency_ms": ms,
                        "report": format!(
                            "🌐 <b>بررسی سلامت وب‌سایت (Rust Engine):</b>\n• آدرس: <code>{}</code>\n• وضعیت HTTP: <b>{} {}</b>\n• زمان پاسخ: <b>{} ms</b>\n• وب‌سرور: <code>{}</code>",
                            url, status, reason, ms, srv
                        )
                    })
                }
                Err(e) => json!({
                    "ok": false,
                    "report": format!("❌ خطا در اتصال به <code>{}</code>: {}", url, e)
                }),
            }
        }
        "weather" => {
            let city = target.trim();
            let wttr_url = format!("https://wttr.in/{}?format=j1&lang=fa", urlencoding::encode(city));
            if let Ok(r) = client.get(&wttr_url).send().await {
                if let Ok(v) = r.json::<serde_json::Value>().await {
                    if let Some(cc) = v.pointer("/current_condition/0") {
                        let temp_c = cc.get("temp_C").and_then(|x| x.as_str()).unwrap_or("?");
                        let feels = cc.get("FeelsLikeC").and_then(|x| x.as_str()).unwrap_or(temp_c);
                        let hum = cc.get("humidity").and_then(|x| x.as_str()).unwrap_or("?");
                        let wind = cc.get("windspeedKmph").and_then(|x| x.as_str()).unwrap_or("?");
                        let desc = cc
                            .pointer("/lang_fa/0/value")
                            .or_else(|| cc.pointer("/weatherDesc/0/value"))
                            .and_then(|x| x.as_str())
                            .unwrap_or("صاف");
                        return json!({
                            "ok": true,
                            "report": format!(
                                "🌤 <b>وضعیت زنده آب‌وهوای {} (Rust Engine):</b>\n\n• 🌡 <b>دما:</b> <code>{}°C</code> (دمای محسوس: <code>{}°C</code>)\n• ☁️ <b>آسمان:</b> {}\n• 💧 <b>رطوبت:</b> <code>{}%</code>\n• 💨 <b>سرعت باد:</b> <code>{} km/h</code>",
                                city, temp_c, feels, desc, hum, wind
                            )
                        });
                    }
                }
            }
            json!({"ok": false, "report": format!("⚠️ اطلاعات آب و هوای {} دریافت نشد.", city)})
        }
        "wiki" => {
            let term = target.trim();
            let w_url = format!(
                "https://fa.wikipedia.org/api/rest_v1/page/summary/{}",
                urlencoding::encode(&term.replace(' ', "_"))
            );
            if let Ok(r) = client.get(&w_url).send().await {
                if let Ok(v) = r.json::<serde_json::Value>().await {
                    let extract = v.get("extract").and_then(|x| x.as_str()).unwrap_or("");
                    let title = v.get("title").and_then(|x| x.as_str()).unwrap_or(term);
                    let page_url = v
                        .pointer("/content_urls/desktop/page")
                        .and_then(|x| x.as_str())
                        .unwrap_or("");
                    if !extract.is_empty() {
                        let mut rep = format!("📚 <b>دانشنامه ویکی‌پدیا ({}) [Rust Engine]:</b>\n\n{}", title, extract);
                        if !page_url.is_empty() {
                            rep.push_str(&format!("\n\n🔗 <a href=\"{}\">مطالعه مقاله کامل در ویکی‌پدیا</a>", page_url));
                        }
                        return json!({"ok": true, "report": rep});
                    }
                }
            }
            let search_res = rust_web_search(&format!("ویکی پدیا {}", term), 3).await;
            let snips = search_res
                .get("snippets")
                .and_then(|s| s.as_array())
                .map(|arr| arr.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join("\n\n"))
                .unwrap_or_default();
            json!({
                "ok": !snips.is_empty(),
                "report": if snips.is_empty() { format!("مقاله‌ای برای «{}» یافت نشد.", term) } else { format!("📚 <b>خلاصه دانشنامه برای «{}»:</b>\n\n{}", term, snips) }
            })
        }
        _ => json!({"ok": false, "report": "Unknown net mode"}),
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("{}", json!({"ok": false, "error": "Usage: amir-rust-agent <hunt|download|search|net> [args...]"}));
        return;
    }

    let cmd = args[1].as_str();
    match cmd {
        "hunt" => {
            let query = args.get(2).map(|s| s.as_str()).unwrap_or("");
            let ext = args.get(3).map(|s| s.as_str()).unwrap_or("");
            let out_dir = args.get(4).map(|s| s.as_str()).unwrap_or("./server_storage");
            let res = hunt_files_on_web(query, ext, out_dir).await;
            println!("{}", serde_json::to_string(&res).unwrap_or_default());
        }
        "download" => {
            let url = args.get(2).map(|s| s.as_str()).unwrap_or("");
            let out_dir = args.get(3).map(|s| s.as_str()).unwrap_or("./server_storage");
            let max_mb: u64 = args.get(4).and_then(|s| s.parse().ok()).unwrap_or(300);
            let res = stream_download_file(url, out_dir, max_mb, "").await;
            println!("{}", serde_json::to_string(&res).unwrap_or_default());
        }
        "search" => {
            let query = args.get(2).map(|s| s.as_str()).unwrap_or("");
            let max_res: usize = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(4);
            let res = rust_web_search(query, max_res).await;
            println!("{}", serde_json::to_string(&res).unwrap_or_default());
        }
        "net" => {
            let mode = args.get(2).map(|s| s.as_str()).unwrap_or("http");
            let target = args.get(3).map(|s| s.as_str()).unwrap_or("google.com");
            let extra = args.get(4).map(|s| s.as_str()).unwrap_or("80");
            let res = rust_net_tool(mode, target, extra).await;
            println!("{}", serde_json::to_string(&res).unwrap_or_default());
        }
        _ => {
            println!("{}", json!({"ok": false, "error": format!("Unknown command: {}", cmd)}));
        }
    }
}
