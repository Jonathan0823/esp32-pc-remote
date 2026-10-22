# ESP32 PC Remote — Dashboard

A private web dashboard to wake your PC and monitor PC/ESP32 status over MQTT. It does not control Windows media or launch configured applications/scripts; those actions are voice-triggered through the paired Windows companion.

## Setup

### 1. Configure

From the repository root, enter the dashboard directory, install dependencies with [Bun](https://bun.sh), and copy the configuration template:

```bash
cd dashboard
bun install
cp .env.example .env
```

Fill in your MQTT broker details in `.env`. Use the same base topic as the firmware's `MQTT_BASE_TOPIC`.

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
3. Root directory: `dashboard`
4. Framework preset: **Vite**
5. Build command: `bun run build`
6. Build output directory: `dist`

> **Before adding `VITE_*` credentials**, protect Git-connected Pages previews: configure Cloudflare Access for every preview hostname, disable preview builds, or use separate restricted preview credentials. Do not expose production MQTT credentials in an unprotected preview; `VITE_*` values are embedded in the browser bundle.

7. Environment variables: add `VITE_*` variables above

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

Create a dedicated MQTT user for the dashboard with topic ACLs. Replace `<MQTT_BASE_TOPIC>` in every entry below with the exact value configured in both firmware `MQTT_BASE_TOPIC` and dashboard `VITE_MQTT_BASE_TOPIC` (for example, `esp-32-remote`):

- Allow **subscribe**: `<MQTT_BASE_TOPIC>/state`, `<MQTT_BASE_TOPIC>/reply`, `<MQTT_BASE_TOPIC>/event`, `<MQTT_BASE_TOPIC>/log`
- Allow **publish**: `<MQTT_BASE_TOPIC>/cmd`

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
