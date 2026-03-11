# VPN Toggle

A Tauri 2.0 desktop application for toggling VPN gateways on OPNsense routers via API.

## Features

- **System Tray App**: Lives in the system tray, closing the window hides to tray
- **OPNsense Integration**: Toggle VPN gateways via OPNsense API
- **Multiple Gateways**: Manage multiple VPN gateways from a single interface
- **Real-time Status**: See connection status with visual indicators
- **Persistent Settings**: API credentials and gateway configurations are saved locally

## Prerequisites

- Node.js (v18 or later)
- Rust (latest stable)
- Tauri CLI

## Development

1. Install dependencies:
   ```bash
   npm install
   ```

2. Run in development mode:
   ```bash
   npm run tauri dev
   ```

## Building

Build the application:
```bash
npm run tauri build
```

The built executable will be in `src-tauri/target/release/`.

## OPNsense Setup

1. Log into your OPNsense router
2. Navigate to System > Access > Users
3. Create an API key/secret pair
4. Ensure you have firewall aliases configured for your VPN gateways
5. Use the alias names in the app's gateway configuration

## Configuration

1. Open the app and click "Settings"
2. Enter your OPNsense base URL (e.g., `https://10.0.0.1:444`)
3. Enter your API Key and API Secret
4. Add VPN gateways with:
   - Display Name: Human-readable name
   - Gateway Name: The alias name in OPNsense (must match exactly)

## How It Works

The app toggles VPN gateways by adding/removing the device's local IPv4 address to firewall aliases:
- VPN is "enabled" when the alias contains the device's local IP
- Toggle ON: adds the device's local IPv4 to the alias
- Toggle OFF: removes the device's local IPv4 from the alias
- After each toggle, the firewall is reconfigured to apply changes
- The local IP is detected automatically at runtime

## Tech Stack

- **Frontend**: React + TypeScript + Tailwind CSS
- **Backend**: Rust (Tauri 2.0)
- **Bundler**: Vite
- **HTTP Client**: reqwest (with TLS cert verification disabled for self-signed certs)

## License

MIT
