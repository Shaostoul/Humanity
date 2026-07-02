# Self-Hosting a Humanity Server

Run your own Humanity Network server in under 10 minutes. Single binary, zero external dependencies, SQLite built-in.

## What you get (in plain words)

Running a server means **your computer hosts its own copy of the HumanityOS
website and chat network** -- the same thing you see at united-humanity.us,
but yours. Your friends and family connect to YOUR address, your messages
live on YOUR machine, and nobody can take it away from you. "Relay" and
"server" mean the same thing in these docs: the program that stays on and
passes messages between people.

You do NOT need to be a programmer. If you can download a file and paste a
few commands, you can run a server.

---

## Requirements

- **OS:** Linux (Debian/Ubuntu recommended), macOS, or Windows
- **RAM:** 256MB minimum
- **Disk:** 1GB+ (grows with messages and uploads)
- **Domain + TLS:** Required for production (Let's Encrypt is free); NOT
  needed to try it out on your own machine first
- **Rust compiler:** only if you choose to build from source (Option B
  below) -- the normal path is a ready-made download, no compiler needed

---

## Quick Start

### Option A: download the ready-made program (recommended)

Every release ships prebuilt binaries -- no compiler, no build step.

```bash
# Linux (x64): download, make it runnable, run it
wget https://github.com/Shaostoul/Humanity/releases/latest/download/HumanityOS-linux-x64
chmod +x HumanityOS-linux-x64
./HumanityOS-linux-x64 --headless
```

On **Windows**, download `HumanityOS-windows-x64.exe` from the
[releases page](https://github.com/Shaostoul/Humanity/releases/latest),
then in a terminal run `HumanityOS-windows-x64.exe --headless`.
On **macOS**, download `HumanityOS-macos-arm64` (Apple Silicon) or
`HumanityOS-macos-x64` (Intel), `chmod +x` it, and run it with
`--headless` the same way.

### Option B: build from source (for developers)

```bash
git clone https://github.com/Shaostoul/Humanity.git
cd Humanity
cargo build --release --features relay --no-default-features
./target/release/HumanityOS --headless
```

Either way, that's it. The relay starts on `http://localhost:3210` --
open that address in your browser and you are looking at **your own copy
of the website**, served by your machine:
- Web client (the whole site) at `/`
- WebSocket at `/ws`
- Bot API at `/api/`
- SQLite database auto-created at `data/relay.db`

---

## Configuration

All configuration is via environment variables. Create a `.env` file or set them directly:

```bash
# Required for production
ADMIN_KEYS=your_dilithium3_public_key_hex # Comma-separated admin public keys (Dilithium3 / ML-DSA-65 hex)
API_SECRET=generate_a_random_64_char_hex  # For bot API authentication

# Optional
WEBHOOK_URL=https://your-webhook-endpoint # Notified on new messages
WEBHOOK_TOKEN=your_webhook_bearer_token   # Auth for webhook calls
WEBHOOK_SECRET=random_hex_for_github      # HMAC-SHA256 for GitHub webhooks
RUST_LOG=info                              # Logging level (trace/debug/info/warn/error)
```

### Generate Secrets

```bash
# Generate a random API secret
openssl rand -hex 32

# Your admin key is your Dilithium3 / ML-DSA-65 public key from the chat client
# (visible in sidebar after connecting)
```

---

## Production Setup (Linux + nginx)

### 1. Create a dedicated user

```bash
sudo useradd -r -s /bin/false humanity
sudo mkdir -p /opt/Humanity
sudo chown humanity:humanity /opt/Humanity
```

### 2. Build and install

```bash
cd /opt/Humanity
git clone https://github.com/Shaostoul/Humanity.git .
cargo build --release --features relay --no-default-features
```

### 3. Create systemd service

```ini
# /etc/systemd/system/humanity-relay.service
[Unit]
Description=Humanity Network Relay
After=network.target

[Service]
Type=simple
User=humanity
Group=humanity
WorkingDirectory=/opt/Humanity
ExecStart=/opt/Humanity/target/release/HumanityOS --headless
EnvironmentFile=/opt/Humanity/.env
Restart=always
RestartSec=5

# Security hardening
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/Humanity/data
NoNewPrivileges=true
PrivateTmp=true

[Install]
WantedBy=multi-user.target
```

```bash
# Create .env with secrets
sudo tee /opt/Humanity/.env << 'EOF'
ADMIN_KEYS=your_public_key_here
API_SECRET=$(openssl rand -hex 32)
RUST_LOG=info
EOF
sudo chmod 600 /opt/Humanity/.env

# Enable and start
sudo systemctl daemon-reload
sudo systemctl enable humanity-relay
sudo systemctl start humanity-relay
```

### 4. Set up nginx with TLS

```bash
# Install nginx and certbot
sudo apt install nginx certbot python3-certbot-nginx
```

```nginx
# /etc/nginx/sites-available/humanity
server {
    listen 80;
    server_name your-domain.com;
    return 301 https://$host$request_uri;
}

server {
    listen 443 ssl http2;
    server_name your-domain.com;

    ssl_certificate /etc/letsencrypt/live/your-domain.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/your-domain.com/privkey.pem;
    ssl_protocols TLSv1.2 TLSv1.3;

    # Security headers
    add_header X-Content-Type-Options nosniff;
    add_header X-Frame-Options SAMEORIGIN;
    add_header Referrer-Policy strict-origin-when-cross-origin;
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;

    # Rate limiting
    limit_req_zone $binary_remote_addr zone=general:10m rate=10r/s;
    limit_req_zone $binary_remote_addr zone=upload:10m rate=2r/m;

    # WebSocket proxy
    location /ws {
        proxy_pass http://127.0.0.1:3210/ws;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_read_timeout 86400;
    }

    # API proxy
    location /api/ {
        limit_req zone=general burst=20 nodelay;
        proxy_pass http://127.0.0.1:3210/api/;
        proxy_set_header Host $host;
    }

    # Upload proxy
    location /api/upload {
        limit_req zone=upload burst=5 nodelay;
        client_max_body_size 10M;
        proxy_pass http://127.0.0.1:3210/api/upload;
        proxy_set_header Host $host;
    }

    # Serve uploads
    location /uploads/ {
        proxy_pass http://127.0.0.1:3210/uploads/;
        add_header X-Content-Type-Options nosniff;
    }

    # Static files (chat client)
    location / {
        proxy_pass http://127.0.0.1:3210/;
        proxy_set_header Host $host;
    }
}
```

```bash
# Enable site and get TLS certificate
sudo ln -s /etc/nginx/sites-available/humanity /etc/nginx/sites-enabled/
sudo certbot --nginx -d your-domain.com
sudo systemctl restart nginx
```

### 5. Open firewall ports

```bash
sudo ufw allow 22    # SSH
sudo ufw allow 80    # HTTP (redirects to HTTPS)
sudo ufw allow 443   # HTTPS
sudo ufw enable
```

Do NOT expose port 3210, nginx handles all public traffic.

---

## Federation

### What federation means (in plain words)

Federation is servers agreeing to talk to each other. Without it, your
server is a private island: only people who connect directly to your
address can see each other. With it, your server and another server
exchange **signed profiles** (so people on both servers can recognise each
other) and discover one another's existence. Your messages and data STAY
on your server -- federation shares identities and server discovery, not
your chat history.

You do not need federation to use your server. A family server that never
federates works fine forever. Federate when you want your community to be
visible to the wider network.

### The technical part

Your server automatically generates a Dilithium3 / ML-DSA-65 keypair on first run (federation object signing and profile gossip are Dilithium3). Other servers can discover yours via:

```
GET https://your-domain.com/api/server-info
```

### Joining the Federation

1. Run your server publicly with a domain and TLS (the Production Setup
   section above).
2. Ask an existing server's admin to add you. **The easiest first partner
   is united-humanity.us itself**: join its chat at
   [united-humanity.us/chat](https://united-humanity.us/chat), say hello
   in `#general`, and ask Shaostoul (the operator) to federate with your
   domain. There is no application form and it costs nothing.
3. They run `/server-add https://your-domain.com` on their side. You can
   also add THEM from your side the same way -- federation links are
   per-direction.
4. **Check that it worked:** open
   `https://your-domain.com/api/federation/servers` in a browser -- the
   other server should be listed. On theirs, yours should appear the same
   way. Profile gossip then flows automatically; there is nothing else to
   switch on.
5. Trust tiers are assigned based on verification and Accord adoption:
   - **Tier 3 (🟢):** Verified identity + publicly adopted the Humanity Accord
   - **Tier 2 (🟡):** Verified identity only
   - **Tier 1 (🔵):** Unverified + Accord adopted
   - **Tier 0 (⚪):** Unverified

To earn the highest trust tier, publicly adopt the [Humanity Accord](../accord/humanity_accord.md) and verify your server identity with an existing trusted server admin.

---

## Admin Commands

Once connected with your admin key, you have access to:

| Command | Description |
|---------|-------------|
| `/verify <name>` | Grant verified status to a user |
| `/mod <name>` | Promote user to moderator |
| `/kick <name>` | Disconnect a user |
| `/ban <name>` | Permanently ban a user |
| `/mute <name> <seconds>` | Temporarily mute a user |
| `/lockdown` | Toggle lockdown (block new registrations) |
| `/invite` | Generate a one-time invite code (24h validity) for lockdown bypass; admins and mods |
| `/channel-create <name>` | Create a new channel |
| `/channel-delete <name>` | Delete a channel |
| `/wipe` | Clear all messages in the CURRENT channel (admin only) |
| `/wipe-all` | Clear all messages across every channel (admin only) |
| `/server-add <url>` | Add a federated server |
| `/server-trust <name> <tier>` | Set federation trust tier |

---

## Updating

```bash
cd /opt/Humanity
git pull
cargo build --release --features relay --no-default-features
sudo systemctl restart humanity-relay
```

Clients auto-detect the server update and reload automatically.

---

## Troubleshooting

**Server won't start:**
- Check logs: `journalctl -u humanity-relay -f`
- Ensure the data directory is writable by the humanity user
- Verify `.env` file exists and has correct permissions (chmod 600)

**WebSocket won't connect:**
- Ensure nginx is proxying `/ws` correctly
- Check that `proxy_read_timeout` is set high (86400 for 24h)
- Verify TLS certificate is valid

**Users can't register names:**
- Check if lockdown is enabled (`/lockdown` to toggle)
- Check server logs for rate limiting messages

**Uploads failing:**
- Ensure `client_max_body_size` is set in nginx
- Check data/uploads/ directory permissions
- Users must be verified to upload

---

## Architecture

```
Browser ↔ nginx (TLS) ↔ Relay (port 3210)
                              ↓
                         SQLite DB
                         data/uploads/
```

- **Single binary**, no external dependencies
- **SQLite**, embedded database, no setup needed
- **WebSocket**, real-time bidirectional communication
- **Dilithium3 / ML-DSA-65** (FIPS 204), post-quantum cryptographic identity (derived from the BIP39 seed), no passwords
- **Kyber768 / ML-KEM-768** (FIPS 203), end-to-end encrypted DMs (pure ML-KEM to BLAKE3-KDF to AES-256-GCM)

> Cryptography here is summarized. The canonical, always-current crypto inventory lives in the "Cryptography" section of `CLAUDE.md`. Read it before quoting any algorithm.

---

## Privacy

By default, the relay:
- Does NOT log IP addresses
- Does NOT track users
- Does NOT require email or phone
- Stores messages in SQLite (encrypted DMs stored as ciphertext)
- Stores uploaded files in data/uploads/ (4-image FIFO per user)

You control your server. You control the data. Delete the database file and everything's gone.

---

*Public domain. No permission needed. Run your own server and join the federation.*
