# ESP32 PC Remote — Dashboard

A private web dashboard to control your PC through the ESP32 remote over MQTT.

## Setup

### 1. Configure

Copy `.env.example` to `.env` and fill in your MQTT broker details:

```bash
cp .env.example .env
```

```
VITE_MQTT_BROKER_URL=wss://your-broker.hivemq.cloud:8884/mqtt
VITE_MQTT_USERNAME=your-username
VITE_MQTT_PASSWORD=your-password
VITE_MQTT_BASE_TOPIC=esp-32-remote
VITE_PC_NAME=DESKTOP-QIIIOTN
```

The broker URL for HiveMQ Cloud WebSocket is typically `wss://<host>:8884/mqtt`.

### 2. Build

```bash
bun run build
```

### 3. Deploy to Cloudflare Pages

**Option A — CLI:**

```bash
bunx wrangler pages deploy dist --branch production
```

**Option B — Dashboard (easier):**
1. Go to **Cloudflare Dashboard > Workers & Pages > Create application > Pages**
2. Connect your Git repository
3. Framework preset: **Vite**
4. Build command: `bun run build`
5. Build output directory: `dist`
6. Environment variables: add `VITE_*` variables above

### 4. Lock it down with Cloudflare Access

1. In **Cloudflare Dashboard**, go to **Zero Trust > Access > Applications**
2. Add a **Self-hosted** application
3. Set the domain to your Pages domain (e.g. `esp32-pc-remote-dashboard.pages.dev`)
4. Add a policy:
   - Policy name: `Only me`
   - Action: **Allow**
   - Configure rules: **Emails** → `your-github-email@example.com`
   - Or use **GitHub** as the identity provider and allow your GitHub account
5. Save — now only you can access the site

> **Tip:** Use the same domain in Cloudflare Access as your Pages deployment, or set up a custom domain.

### 5. MQTT Broker ACLs (important)

Create a dedicated MQTT user for the dashboard with topic ACLs:

- Allow **subscribe**: `esp-32-remote/state`, `esp-32-remote/reply`, `esp-32-remote/event`, `esp-32-remote/log`
- Allow **publish**: `esp-32-remote/cmd`

This limits damage if the credentials are extracted from the browser.

## Risk

**The MQTT credentials are visible in the browser.** Anyone who can open the site can inspect them via DevTools. Mitigations:

- Cloudflare Access blocks everyone except you
- MQTT ACLs restrict the credentials to only the required topics
- Use a separate MQTT user for the dashboard (not the same as the ESP32)
- The `VITE_*` env vars are embedded in the JS bundle at build time

## Development

```bash
bun run dev     # local dev server
bun run test    # run tests
bun run build   # production build
```
