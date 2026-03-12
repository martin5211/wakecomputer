#!/usr/bin/env bash
set -euo pipefail

SERVICE_LABEL="com.wakecomputer.agent"
PLIST_PATH="/Library/LaunchDaemons/${SERVICE_LABEL}.plist"
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
    # Search target subdirectories (e.g. target/aarch64-apple-darwin/release/)
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

# Stop existing service if loaded
launchctl bootout "system/${SERVICE_LABEL}" 2>/dev/null || true

# Install binary
cp "$BIN" "$INSTALL_DIR/$EXE_NAME"
chmod 755 "$INSTALL_DIR/$EXE_NAME"

# Create launchd plist
cat > "$PLIST_PATH" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>${SERVICE_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>${INSTALL_DIR}/${EXE_NAME}</string>
        <string>--token</string>
        <string>${TOKEN}</string>
        <string>--listen</string>
        <string>0.0.0.0:${PORT}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardErrorPath</key>
    <string>/var/log/wakecomputer-agent.log</string>
</dict>
</plist>
EOF

# Load service
launchctl bootstrap system "$PLIST_PATH"

echo
echo "Installed and started $SERVICE_LABEL"
echo "  Binary:  $INSTALL_DIR/$EXE_NAME"
echo "  Port:    $PORT"
echo "  Service: $SERVICE_LABEL"
echo
echo "Check status: launchctl print system/$SERVICE_LABEL"
echo "Logs: /var/log/wakecomputer-agent.log"
