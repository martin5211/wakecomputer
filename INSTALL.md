# Installation Guide: Alexa Smart Home Skill

This guide sets up voice control for your PC ("Alexa, turn on workstation") using:

- **AWS Lambda** — translates Alexa Smart Home directives into REST calls
- **Pi server** — receives REST calls and performs WOL / agent shutdown
- **wakecomputer-agent** — runs on each target machine, accepts shutdown commands
- **ngrok** — tunnels Lambda requests to the Pi on your LAN

## 1. AWS Lambda Setup (Free Tier)

AWS Free Tier includes 1M Lambda requests/month — more than enough.

### Create the function

1. Go to [AWS Lambda Console](https://console.aws.amazon.com/lambda)
2. **Create function** → Author from scratch
   - Name: `wakecomputer-alexa`
   - Runtime: **Python 3.14**
   - Architecture: **arm64** (Graviton, cheaper)
3. Under **General configuration**, set:
   - Memory: **128 MB**
   - Timeout: **8 seconds**

### Upload the code

```bash
# Linux/macOS
cd lambda && zip -r ../lambda.zip lambda_function.py

# Windows (7-Zip)
cd lambda && 7z a ..\lambda.zip lambda_function.py
```

Upload `lambda.zip` via the Lambda console (Code tab → Upload from → .zip file).

### Configure environment variables

In the Lambda console → Configuration → Environment variables:

| Key | Value |
|-----|-------|
| `PI_BASE_URL` | Your ngrok URL, e.g. `https://abc123.ngrok-free.app` |
| `PI_API_TOKEN` | Same token as in Pi's `config.yaml` |

### Add Alexa Smart Home trigger

1. In the Lambda console → Configuration → Triggers → **Add trigger**
2. Select **Alexa Smart Home**
3. Enter the **Skill ID** (from Section 2 below)
4. Note the **Lambda ARN** (top-right of the page) — needed for Section 2

## 2. Alexa Developer Console Setup

### Create the skill

1. Go to [Alexa Developer Console](https://developer.amazon.com/alexa/console/ask)
2. **Create Skill** → Skill name: `PC Control` → Type: **Smart Home** → Create
3. On the skill page, set **Default endpoint** to your Lambda ARN
4. Note the **Skill ID** — add it back to the Lambda trigger (Section 1)

### Set up Account Linking (required for Smart Home skills)

#### Create a Login with Amazon (LWA) Security Profile

1. Go to [Login with Amazon](https://developer.amazon.com/loginwithamazon/console/site/lwa/overview.html)
2. **Create a New Security Profile**
   - Name: `Wakecomputer`
   - Description: `PC wake/shutdown control`
   - Privacy Notice URL: any URL (e.g. your GitHub repo)
3. Note the **Client ID** and **Client Secret**

#### Configure Account Linking in the Alexa skill

1. In the Alexa Developer Console → your skill → **Account Linking**
2. Fill in:
   - **Authorization URI**: `https://www.amazon.com/ap/oa`
   - **Access Token URI**: `https://api.amazon.com/auth/o2/token`
   - **Client ID**: from LWA above
   - **Client Secret**: from LWA above
   - **Scope**: `profile`
3. Copy the **Alexa Redirect URLs** shown on this page

#### Add Redirect URLs to LWA

1. Back in the LWA console → your security profile → **Web Settings**
2. Add each Alexa Redirect URL to **Allowed Return URLs**

## 3. Enable & Test

1. Open the **Alexa app** on your phone
2. Go to **More → Skills & Games → Your Skills → Dev**
3. Enable the skill → complete **Account Linking** (sign in with your Amazon account)
4. Say: **"Alexa, discover my devices"**
5. Your machines should appear as devices
6. Test: **"Alexa, turn on workstation"** / **"Alexa, turn off workstation"**

## 4. Agent Setup (Target Machines)

### Build the agent

```bash
# For Windows
cargo build -p wakecomputer-agent --release --target x86_64-pc-windows-gnu

# For Linux
cargo build -p wakecomputer-agent --release
```

### Deploy on Windows

1. Copy `wakecomputer-agent.exe` to the target machine
2. Generate a random token:
   ```powershell
   python3 -c "import secrets; print(secrets.token_urlsafe(32))"
   ```
3. Install as a Windows service:
   ```powershell
   sc.exe create wakecomputer-agent binPath= "C:\path\to\wakecomputer-agent.exe --token YOUR_TOKEN --listen 0.0.0.0:9877" start= auto
   sc.exe start wakecomputer-agent
   ```
4. Open the firewall port:
   ```powershell
   netsh advfirewall firewall add rule name="wakecomputer-agent" dir=in action=allow protocol=TCP localport=9877
   ```

### Deploy on macOS

1. Build the agent on your Mac (or cross-compile for `aarch64-apple-darwin`):
   ```bash
   cargo build -p wakecomputer-agent --release
   ```
2. Run the installer:
   ```bash
   sudo ./agent/install_macos.sh YOUR_TOKEN
   ```
   This installs a launchd daemon that starts on boot and restarts if it crashes.
3. Verify it's running:
   ```bash
   curl http://localhost:9877/health
   ```
4. Logs: `/var/log/wakecomputer-agent.log`

To uninstall:
```bash
sudo launchctl bootout system/com.wakecomputer.agent
sudo rm /usr/local/bin/wakecomputer-agent /Library/LaunchDaemons/com.wakecomputer.agent.plist
```

### Deploy on Linux

1. Copy the binary and create a systemd service, or run directly:
   ```bash
   ./wakecomputer-agent --token YOUR_TOKEN --listen 0.0.0.0:9877
   ```

## 5. Pi Server Deployment

### Build

```bash
cargo zigbuild -p wakecomputer --release --target aarch64-unknown-linux-gnu.2.36
cargo deb -p wakecomputer --no-build --target aarch64-unknown-linux-gnu
```

### Deploy

```bash
scp target/aarch64-unknown-linux-gnu/debian/*.deb pi:~
ssh pi 'sudo dpkg -i ~/wakecomputer_*.deb'
```

### Configure

Edit `/etc/wakecomputer/config.yaml` on the Pi:

```yaml
listen: "127.0.0.1:9876"
api_token: "your-secret-token-here"
machines:
  - name: workstation
    mac: "04:D9:F5:21:88:66"
    ip: "192.168.1.217"
    agent_port: 9877
    agent_token: "the-agent-token-you-generated"
```

Generate a random token:

```bash
python3 -c "import secrets; print(secrets.token_urlsafe(32))"
```

### ngrok

Ensure ngrok is running and the URL matches Lambda's `PI_BASE_URL`:

```bash
ngrok http 9876
```

For a stable URL, use a paid ngrok plan or update `PI_BASE_URL` in Lambda whenever the URL changes.

## Testing locally

```bash
# Test the agent
curl http://localhost:9877/health
curl -X POST -H "Authorization: Bearer <agent-token>" http://localhost:9877/shutdown

# Test the server
curl -H "Authorization: Bearer <server-token>" http://localhost:9876/machines

curl -X POST -H "Authorization: Bearer <server-token>" -H "Content-Type: application/json" \
  http://localhost:9876/wake -d '{"machine":"workstation"}'

curl -X POST -H "Authorization: Bearer <server-token>" -H "Content-Type: application/json" \
  http://localhost:9876/status -d '{"machine":"workstation"}'

curl -X POST -H "Authorization: Bearer <server-token>" -H "Content-Type: application/json" \
  http://localhost:9876/shutdown -d '{"machine":"workstation"}'
```
