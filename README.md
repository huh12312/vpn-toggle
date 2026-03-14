# OPNsense FW Alias Toggle

A Tauri 2.0 desktop application that controls per-device gateway routing on **OPNsense** by adding or removing your machine's IP address from a firewall alias. This is an OPNsense-specific tool — it uses the OPNsense API directly and will not work with other routers or firewalls.

## What It Actually Does

At its core, this app manages one thing: **your device's presence in an OPNsense firewall alias**.

OPNsense lets you create firewall rules that route traffic differently based on alias membership. This app makes it easy to add or remove your current device's local IPv4 address from those aliases — toggling which gateway your traffic uses without touching the OPNsense UI.

**Common use cases:**
- Route your device through a VPN gateway (WireGuard, OpenVPN, etc.)
- Move a device off VPN and back to a standard WAN connection
- Switch between multiple gateways (e.g., VPN A vs. VPN B vs. direct WAN)
- Quickly isolate a device to a specific network segment or gateway policy

**What the toggle does:**
- **ON**: Adds this device's local IP to the firewall alias → traffic routes per your firewall rules
- **OFF**: Removes this device's local IP from the alias → traffic routes normally (default gateway)

The local IP is detected automatically at runtime — no manual entry required.

## Gateway Status Indicator

Separately from the toggle, each configured gateway shows a live status indicator:
- 🟢 **Green** — gateway is online (via `/api/routes/gateway/status`)
- 🟡 **Yellow** — gateway is down
- 🔴 **Red** — error reaching OPNsense API

The toggle is disabled when the gateway is offline, preventing you from routing traffic to a dead gateway.

## Features

- **System tray**: Minimizes to tray on close, always accessible
- **Multiple gateways**: Configure as many gateway/alias pairs as you need
- **Persistent settings**: API credentials and gateway config saved locally
- **Auto IP detection**: Detects your current local IPv4, no manual config needed

## OPNsense Requirements

- OPNsense router with API access enabled
- An API key/secret pair (System → Access → Users → your user → API Keys)
- One or more **firewall aliases** with associated routing rules already configured in OPNsense
  - The alias type should be `Host(s)` to hold IP addresses
  - Your firewall/NAT rules must already be set up to route alias members through the desired gateway

> **Note:** This app manipulates alias membership only. The firewall rules that act on those aliases must exist in OPNsense before the toggle will have any effect.

## Configuration

1. Open the app and click **Settings**
2. Enter your OPNsense base URL (e.g., `https://192.168.1.1`)
3. Enter your **API Key** and **API Secret**
4. Add one or more gateways:
   - **Display Name** — label shown in the UI (e.g., `Mullvad VPN`)
   - **Gateway Name** — OPNsense gateway name for status checks (e.g., `WAN_WIREGUARD`)
   - **Alias Name** — firewall alias this device's IP is added to/removed from (e.g., `vpn_devices`)

> TLS certificate verification is disabled to support self-signed OPNsense certs.

## Installation

Download the latest `.exe` installer from the [Releases](../../releases) page and run it. Windows only.

## Development

**Prerequisites:** Node.js v18+, Rust (stable), Tauri CLI

```bash
npm install
npm run tauri dev   # dev mode
npm run tauri build # production build
```

## Tech Stack

- **Frontend**: React + TypeScript + Tailwind CSS
- **Backend**: Rust (Tauri 2.0)
- **Bundler**: Vite
- **HTTP**: reqwest (TLS verification disabled for self-signed certs)

## License

MIT
