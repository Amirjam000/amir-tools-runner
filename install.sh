#!/usr/bin/env bash
set -e

echo "=========================================================="
echo "🦀 نصب و راه‌اندازی خودکار ربات امیر تولز (نسخه Rust v6.0)"
echo "=========================================================="

# 1. نصب پیش‌نیازهای سیستم، ffmpeg و yt-dlp
if command -v apt-get >/dev/null 2>&1; then
    sudo apt-get update -y
    sudo apt-get install -y curl git build-essential pkg-config libssl-dev ffmpeg python3-pip ca-certificates
fi

# نصب آخرین نسخه yt-dlp و edge-tts
sudo curl -L https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -o /usr/local/bin/yt-dlp || true
sudo chmod a+rx /usr/local/bin/yt-dlp || true
pip3 install -U edge-tts --break-system-packages >/dev/null 2>&1 || pip3 install -U edge-tts >/dev/null 2>&1 || true

# 2. نصب کامپایلر Rust در صورت عدم وجود
if ! command -v cargo >/dev/null 2>&1; then
    echo "📦 در حال نصب کامپایلر Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi
export PATH="$HOME/.cargo/bin:$PATH"

# 3. کلون یا بروزرسانی سورس از مخزن
INSTALL_DIR="$HOME/amir-tools-rust"
if [ ! -f "Cargo.toml" ]; then
    rm -rf "$INSTALL_DIR"
    git clone https://github.com/Amirjam000/amir-tools-runner.git "$INSTALL_DIR"
    cd "$INSTALL_DIR"
fi

# 4. کامپایل نسخه نهایی پرسرعت (Release)
echo "⚡ در حال کامپایل هسته Rust با حداکثر بهینه‌سازی..."
cargo build --release

# 5. ساخت سرویس دائمی systemd (اجرای ۲۴/۷ در پس‌زمینه)
SERVICE_FILE="/etc/systemd/system/amir-tools-rust.service"
WORK_DIR="$(pwd)"
BIN_PATH="$WORK_DIR/target/release/amir_tools_bot"

if [ -d "/etc/systemd/system" ]; then
    sudo bash -c "cat > $SERVICE_FILE" <<EOF
[Unit]
Description=Amir Tools Ultimate Telegram Bot (Native Rust v6.0)
After=network.target

[Service]
Type=simple
WorkingDirectory=$WORK_DIR
ExecStart=$BIN_PATH
Restart=always
RestartSec=3
Environment=RUST_BACKTRACE=1

[Install]
WantedBy=multi-user.target
EOF
    sudo systemctl daemon-reload
    sudo systemctl enable amir-tools-rust
    sudo systemctl restart amir-tools-rust
    echo "=========================================================="
    echo "✅ ربات امیر تولز (Rust v6.0) با موفقیت نصب و در پس‌زمینه فعال شد!"
    echo "📊 مشاهده لاگ زنده: sudo journalctl -u amir-tools-rust -f"
    echo "=========================================================="
else
    echo "🚀 در حال اجرای مستقیم ربات..."
    nohup "$BIN_PATH" > bot.log 2>&1 &
    echo "✅ ربات در پس‌زمینه اجرا شد! (فایل لاگ: $WORK_DIR/bot.log)"
fi
