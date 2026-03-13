import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import VpnList from "./components/VpnList";
import Settings from "./components/Settings";

export interface VpnGateway {
  display_name: string;
  gateway_name: string; // OPNsense gateway name for status (e.g. WAN_VPN)
  alias_name: string;   // Firewall alias name for toggle (e.g. vpn_devices)
}

export interface AppSettings {
  base_url: string;
  gateways: VpnGateway[];
}

export interface Credentials {
  api_key: string;
  api_secret: string;
}

function App() {
  const [showSettings, setShowSettings] = useState(false);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [credentials, setCredentials] = useState<Credentials | null>(null);

  useEffect(() => {
    loadSettings();
    loadCredentials();
  }, []);

  const loadSettings = async () => {
    try {
      const settings = await invoke<AppSettings>("get_settings");
      setSettings(settings);
    } catch (error) {
      console.error("Failed to load settings:", error);
    }
  };

  const loadCredentials = async () => {
    try {
      const creds = await invoke<[string, string] | null>("load_credentials");
      if (creds) {
        setCredentials({ api_key: creds[0], api_secret: creds[1] });
      } else {
        setCredentials(null);
        setShowSettings(true);
      }
    } catch (error) {
      console.error("Failed to load credentials:", error);
      setCredentials(null);
      setShowSettings(true);
    }
  };

  const handleSaveSettings = async (newSettings: AppSettings, newCredentials: Credentials) => {
    try {
      await invoke("save_settings", { settings: newSettings });
      await invoke("save_credentials", {
        apiKey: newCredentials.api_key,
        apiSecret: newCredentials.api_secret
      });
      setSettings(newSettings);
      setCredentials(newCredentials);
      setShowSettings(false);
    } catch (error) {
      console.error("Failed to save settings:", error);
      alert("Failed to save settings: " + error);
    }
  };

  if (!settings || credentials === null) {
    return (
      <div className="flex items-center justify-center h-screen bg-gray-100">
        <div className="text-gray-600">Loading...</div>
      </div>
    );
  }

  return (
    <div className="h-screen bg-gray-100 flex flex-col">
      <header className="bg-blue-600 text-white p-4 flex justify-between items-center">
        <h1 className="text-xl font-bold">VPN Toggle</h1>
        <button
          onClick={() => setShowSettings(!showSettings)}
          className="bg-blue-700 hover:bg-blue-800 px-4 py-2 rounded transition-colors"
        >
          {showSettings ? "Close Settings" : "Settings"}
        </button>
      </header>

      <main className="flex-1 overflow-auto">
        {showSettings ? (
          <Settings
            settings={settings}
            credentials={credentials}
            onSave={handleSaveSettings}
            onCancel={() => setShowSettings(false)}
            onClearCredentials={() => {
              setCredentials(null);
              setShowSettings(true);
            }}
          />
        ) : (
          <VpnList settings={settings} />
        )}
      </main>
    </div>
  );
}

export default App;
