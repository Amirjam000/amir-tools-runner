#!/usr/bin/env bash
set -e

echo "============================================================"
echo "🚀 Amir Tools Hybrid (Python Core + Rust Native Agent + yt-dlp)"
echo "🤖 Bot: @Game_iran_amir_bot | Owner: 7243980602"
echo "============================================================"

export DEBIAN_FRONTEND=noninteractive
apt-get update -y
apt-get install -y curl git build-essential pkg-config libssl-dev ffmpeg python3 python3-pip python3-venv ca-certificates

# 1. Install / Update latest yt-dlp binary
echo "📥 [1/5] Installing latest yt-dlp..."
curl -L https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp -o /usr/local/bin/yt-dlp || true
chmod a+rx /usr/local/bin/yt-dlp || true

# 2. Clone or update repository
INSTALL_DIR="/opt/amir-tools-bot"
echo "📦 [2/5] Syncing repository into $INSTALL_DIR..."
if [ -d "$INSTALL_DIR/.git" ]; then
    cd "$INSTALL_DIR"
    git fetch --all
    git reset --hard origin/main
else
    rm -rf "$INSTALL_DIR"
    git clone https://github.com/Amirjam000/amir-tools-runner.git "$INSTALL_DIR"
    cd "$INSTALL_DIR"
fi

mkdir -p "$INSTALL_DIR/server_storage"

# 3. Install Python dependencies (venv or system)
echo "🐍 [3/5] Installing Python dependencies..."
if [ ! -d "$INSTALL_DIR/venv" ]; then
    python3 -m venv "$INSTALL_DIR/venv"
fi
"$INSTALL_DIR/venv/bin/pip" install --upgrade pip
"$INSTALL_DIR/venv/bin/pip" install --upgrade -r "$INSTALL_DIR/requirements.txt" yt-dlp

# 4. Create & Start systemd service immediately so Bot is LIVE right now!
echo "⚡ [4/5] Starting Amir Tools Bot systemd service..."
cat >/etc/systemd/system/amir-tools.service <<EOF
[Unit]
Description=Amir Tools Telegram Bot (Hybrid Python + Rust Agent + yt-dlp)
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=$INSTALL_DIR
ExecStart=$INSTALL_DIR/venv/bin/python $INSTALL_DIR/app.py
Restart=always
RestartSec=3
Environment="PYTHONUNBUFFERED=1"
Environment="PATH=/usr/local/bin:/usr/bin:/bin:$HOME/.cargo/bin"

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable amir-tools.service
systemctl restart amir-tools.service

echo "✅ Python Bot Core is now ONLINE and polling Telegram!"

# 5. Compile Native Rust Agent Engine (amir-rust-agent)
echo "🦀 [5/5] Compiling Native Rust Agent Engine (amir-rust-agent)..."
if ! command -v cargo >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
fi
export PATH="$HOME/.cargo/bin:$PATH"

if [ -d "$INSTALL_DIR/rust_agent" ]; then
    cd "$INSTALL_DIR/rust_agent"
    cargo build --release
    cp -f "$INSTALL_DIR/rust_agent/target/release/amir-rust-agent" "$INSTALL_DIR/amir-rust-agent"
    cp -f "$INSTALL_DIR/rust_agent/target/release/amir-rust-agent" "/usr/local/bin/amir-rust-agent"
    chmod +x "$INSTALL_DIR/amir-rust-agent" "/usr/local/bin/amir-rust-agent"
    echo "🦀 Native Rust Agent Binary compiled and activated at $INSTALL_DIR/amir-rust-agent!"
fi

echo "============================================================"
echo "✅ INSTALLATION COMPLETE! Hybrid Python + Rust Agent is ACTIVE!"
echo "📊 Check status: systemctl status amir-tools --no-pager"
echo "📜 Live logs:    journalctl -u amir-tools -f"
echo "============================================================"
