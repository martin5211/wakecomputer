#!/usr/bin/env bash
set -euo pipefail

SERVICE_NAME="wakecomputer-agent"
INSTALL_DIR="/usr/local/bin"
EXE_NAME="wakecomputer-agent"
PORT=9877

if [ "$(id -u)" -ne 0 ]; then
    echo "ERROR: Run this script as root (sudo)." >&2
    exit 1
fi

TOKEN="${1:-}"
if [ -z "$TOKEN" ]; then
    read -rp "Enter agent token: " TOKEN
fi

if [ -z "$TOKEN" ]; then
    echo "ERROR: Token is required." >&2
    exit 1
fi

# Find binary
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BIN=""
if [ -f "$SCRIPT_DIR/$EXE_NAME" ]; then
    BIN="$SCRIPT_DIR/$EXE_NAME"
elif [ -f "$SCRIPT_DIR/../target/release/$EXE_NAME" ]; then
    BIN="$SCRIPT_DIR/../target/release/$EXE_NAME"
else
    # Search target subdirectories (e.g. target/x86_64-unknown-linux-gnu/release/)
    for candidate in "$SCRIPT_DIR"/../target/*/release/"$EXE_NAME"; do
        if [ -f "$candidate" ]; then
            BIN="$candidate"
            break
        fi
    done
fi

if [ -z "$BIN" ]; then
    echo "ERROR: Cannot find $EXE_NAME." >&2
    echo "Place it next to this script or build with: cargo build -p wakecomputer-agent --release" >&2
    exit 1
fi

# Stop existing service
if systemctl is-active --quiet "$SERVICE_NAME" 2>/dev/null; then
    echo "Stopping existing service..."
    systemctl stop "$SERVICE_NAME"
fi

# Install binary
install -m 755 "$BIN" "$INSTALL_DIR/$EXE_NAME"

# Create systemd unit
cat > "/etc/systemd/system/${SERVICE_NAME}.service" <<EOF
[Unit]
Description=WakeComputer Agent
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=$INSTALL_DIR/$EXE_NAME --token $TOKEN --listen 0.0.0.0:$PORT
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable "$SERVICE_NAME"
systemctl start "$SERVICE_NAME"

echo
echo "Installed and started $SERVICE_NAME"
echo "  Binary:  $INSTALL_DIR/$EXE_NAME"
echo "  Port:    $PORT"
echo "  Service: $SERVICE_NAME"
echo
echo "Check status: systemctl status $SERVICE_NAME"
