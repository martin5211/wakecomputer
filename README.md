# wakecomputer

Wake-On-Lan Skill that wakes and shuts down machines on your local network via Amazon Alexa voice commands. Runs on a Raspberry Pi Zero 2 W.

## Architecture

Two components:

- **`wakecomputer`** (server) — runs on the Pi, receives Alexa requests, sends WoL packets and shutdown commands
- **`wakecomputer-agent`** — runs on each target machine, listens for shutdown commands over HTTP

## Requirements

- Raspberry Pi Zero 2 W (aarch64)
- Target machines with WoL enabled in BIOS
- `wakecomputer-agent` running on each target machine
- An Alexa skill configured to send requests to this server

## Configuration

### Server (`/etc/wakecomputer/config.yaml`)

```yaml
listen: "127.0.0.1:9876"
api_token: "your-server-token"
machines:
  - name: workstation
    mac: "00:AA:BB:CC:DD:EE"
    ip: "192.168.1.2"
    agent_port: 9877
    agent_token: "your-agent-token"
```

- `listen` — address and port to bind (default: `127.0.0.1:9876`)
- `name` — machine identifier used in API requests
- `mac` — MAC address for WoL (colon or dash separated)
- `ip` — IP address of the target machine
- `agent_port` — port the agent listens on (default: `9877`)
- `agent_token` — bearer token for authenticating with the agent

### Agent

Run on each target machine:

```bash
wakecomputer-agent --token <secret-token> --listen 0.0.0.0:9877
```

The agent exposes:
- `GET /health` — returns `200 {"status":"ok"}` (unauthenticated)
- `POST /shutdown` — triggers OS shutdown (requires `Authorization: Bearer <token>`)

#### Windows

Run `agent\install.bat` as Administrator:

```powershell
install.bat <token>
```

This installs the binary to `%ProgramFiles%\wakecomputer-agent\`, creates a Windows service, and opens firewall port 9877.

#### macOS

Run `agent/install_macos.sh` as root:

```bash
sudo ./install_macos.sh <token>
```

This installs the binary to `/usr/local/bin/`, creates a launchd daemon, and starts it. Logs go to `/var/log/wakecomputer-agent.log`.

#### Linux (Debian/Ubuntu)

Run `agent/install.sh` as root:

```bash
sudo ./install.sh <token>
```

This installs the binary to `/usr/local/bin/`, creates a systemd service, and starts it.

## Build

Server cross-compilation requires [ziglang](https://ziglang.org/), `cargo-zigbuild`, and `cargo-deb`.

Agent for Windows requires either MSVC (build natively on Windows) or [MinGW-w64](https://www.mingw-w64.org/) (cross-compile from Linux).

```bash
# Server (for Pi)
rustup target add aarch64-unknown-linux-gnu
cargo zigbuild -p wakecomputer --release --target aarch64-unknown-linux-gnu.2.36
cargo deb -p wakecomputer --no-build --target aarch64-unknown-linux-gnu

# Agent (for Windows, native build on Windows with MSVC)
cargo build -p wakecomputer-agent --release --target x86_64-pc-windows-msvc

# Agent (for Windows, cross-compile from Linux with MinGW-w64)
rustup target add x86_64-pc-windows-gnu
cargo build -p wakecomputer-agent --release --target x86_64-pc-windows-gnu

# Agent (for macOS ARM, native build on Apple Silicon)
cargo build -p wakecomputer-agent --release --target aarch64-apple-darwin

# Agent (for Linux)
cargo build -p wakecomputer-agent --release
```

## Install

```bash
scp target/aarch64-unknown-linux-gnu/debian/wakecomputer_*.deb pi@<ip>:~/
ssh pi@<ip> "sudo dpkg -i ~/wakecomputer_*.deb"
sudo nano /etc/wakecomputer/config.yaml
sudo systemctl start wakecomputer
```

## Usage

All endpoints require `Authorization: Bearer <api_token>`:

```
POST /wake      {"machine":"workstation"}  — sends a Wake-on-LAN magic packet
POST /shutdown  {"machine":"workstation"}  — sends shutdown command to the agent
POST /status    {"machine":"workstation"}  — checks if the agent is reachable
GET  /machines                             — lists configured machines
```

## Logs

```bash
journalctl -u wakecomputer -f
```

Set `RUST_LOG=debug` in the service environment for verbose output.

## License

MIT
