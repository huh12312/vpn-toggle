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
  api_key: string;
  api_secret: string;
  gateways: VpnGateway[];
}

function App() {
  const [showSettings, setShowSettings] = useState(false);
  const [settings, setSettings] = useState<AppSettings | null>(null);

  useEffect(() => {
    loadSettings();
  }, []);

  const loadSettings = async () => {
    try {
      const settings = await invoke<AppSettings>("get_settings");
      setSettings(settings);
      // Show settings if not configured
      if (!settings.api_key || !settings.api_secret) {
        setShowSettings(true);
      }
    } catch (error) {
      console.error("Failed to load settings:", error);
    }
  };

  const handleSaveSettings = async (newSettings: AppSettings) => {
    try {
      await invoke("save_settings", { settings: newSettings });
      setSettings(newSettings);
      setShowSettings(false);
    } catch (error) {
      console.error("Failed to save settings:", error);
      alert("Failed to save settings: " + error);
    }
  };

  if (!settings) {
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
            onSave={handleSaveSettings}
            onCancel={() => setShowSettings(false)}
          />
        ) : (
          <VpnList settings={settings} />
        )}
      </main>
    </div>
  );
}

export default App;
