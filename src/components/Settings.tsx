import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AppSettings, VpnGateway, Credentials } from "../App";

const EyeIcon = ({ open }: { open: boolean }) =>
  open ? (
    <svg xmlns="http://www.w3.org/2000/svg" className="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
      <path d="M10 3C5 3 1.73 7.11 1.05 10c.68 2.89 3.95 7 8.95 7s8.27-4.11 8.95-7C18.27 7.11 15 3 10 3zm0 12a5 5 0 110-10 5 5 0 010 10zm0-8a3 3 0 100 6 3 3 0 000-6z" />
    </svg>
  ) : (
    <svg xmlns="http://www.w3.org/2000/svg" className="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
      <path fillRule="evenodd" d="M3.28 2.22a.75.75 0 00-1.06 1.06l14.5 14.5a.75.75 0 101.06-1.06l-1.745-1.745a10.029 10.029 0 003.3-4.38 1.25 1.25 0 000-.9C18.267 7.11 15 3 10 3a9.958 9.958 0 00-4.512 1.074L3.28 2.22zM10 5a5 5 0 014.592 3.022L12.18 5.61A3 3 0 007.64 10.18L5.228 7.768A7.954 7.954 0 0110 5zm-5.657 4.568A7.955 7.955 0 002 10c.68 2.89 3.95 7 8 7a7.95 7.95 0 004.206-1.196l-1.515-1.516A3 3 0 016.39 9.61L4.343 9.568z" clipRule="evenodd" />
    </svg>
  );

interface SettingsProps {
  settings: AppSettings;
  credentials: Credentials;
  onSave: (settings: AppSettings, credentials: Credentials) => Promise<void>;
  onCancel: () => void;
  onClearCredentials: () => void;
}

function validateSettings(data: AppSettings, creds: Credentials): string | null {
  if (!data.base_url.trim()) return "Base URL is required.";
  if (!/^https?:\/\/.+/.test(data.base_url.trim())) return "Base URL must start with http:// or https://";
  if (!creds.api_key.trim()) return "API Key is required.";
  if (!creds.api_secret.trim()) return "API Secret is required.";
  if (data.interface_ip.trim()) {
    const parts = data.interface_ip.trim().split('.');
    const validIpv4 = /^(\d{1,3}\.){3}\d{1,3}$/.test(data.interface_ip.trim()) && parts.every(p => parseInt(p, 10) <= 255);
    if (!validIpv4) return "Local IP Override must be a valid IPv4 address (e.g. 192.168.1.50).";
  }
  for (let i = 0; i < data.gateways.length; i++) {
    const g = data.gateways[i];
    if (!g.display_name.trim()) return `Gateway ${i + 1}: Display Name is required.`;
    if (!g.gateway_name.trim()) return `Gateway ${i + 1}: Gateway Name is required.`;
    if (!g.alias_name.trim()) return `Gateway ${i + 1}: Alias Name is required.`;
    if (!/^[A-Za-z_][A-Za-z0-9_]{0,31}$/.test(g.alias_name.trim()))
      return `Gateway ${i + 1}: Alias Name must start with a letter or underscore, contain only letters, numbers, and underscores, and be at most 32 characters.`;
  }
  const aliasNames = data.gateways.map((g) => g.alias_name.trim());
  const unique = new Set(aliasNames);
  if (unique.size !== aliasNames.length) return "Duplicate Alias Names detected — each gateway must have a unique alias.";
  return null;
}

function Settings({ settings, credentials, onSave, onCancel, onClearCredentials }: SettingsProps) {
  const [formData, setFormData] = useState<AppSettings>(settings);
  const [formCredentials, setFormCredentials] = useState<Credentials>(credentials);
  const [validationError, setValidationError] = useState<string | null>(null);
  const [showCredentials, setShowCredentials] = useState(false);
  const [confirmClearCredentials, setConfirmClearCredentials] = useState(false);
  const [clearError, setClearError] = useState<string | null>(null);
  // Index of gateway pending removal confirmation; null = none
  const [pendingRemoveIndex, setPendingRemoveIndex] = useState<number | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [isTesting, setIsTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ ok: boolean; message: string } | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (isSaving) return;
    const err = validateSettings(formData, formCredentials);
    if (err) {
      setValidationError(err);
      return;
    }
    setValidationError(null);
    setIsSaving(true);
    try {
      await onSave(formData, formCredentials);
    } finally {
      setIsSaving(false);
    }
  };

  const [isClearing, setIsClearing] = useState(false);

  const handleTestConnection = async () => {
    setIsTesting(true);
    setTestResult(null);
    try {
      const msg = await invoke<string>("test_connection");
      setTestResult({ ok: true, message: msg });
    } catch (e) {
      setTestResult({ ok: false, message: String(e) });
    } finally {
      setIsTesting(false);
    }
  };

  const handleClearCredentials = async () => {
    setClearError(null);
    setIsClearing(true);
    try {
      await invoke("delete_credentials");
      onClearCredentials();
    } catch (e) {
      setClearError(String(e));
      setConfirmClearCredentials(false);
    } finally {
      setIsClearing(false);
    }
  };

  const addGateway = () => {
    setFormData({
      ...formData,
      gateways: [
        ...formData.gateways,
        { display_name: "", gateway_name: "", alias_name: "", id: crypto.randomUUID() },
      ],
    });
  };

  const confirmRemoveGateway = (index: number) => {
    setPendingRemoveIndex(index);
  };

  const doRemoveGateway = (index: number) => {
    setFormData({
      ...formData,
      gateways: formData.gateways.filter((_, i) => i !== index),
    });
    setPendingRemoveIndex(null);
  };

  const updateGateway = (index: number, field: keyof VpnGateway, value: string) => {
    const newGateways = [...formData.gateways];
    newGateways[index] = { ...newGateways[index], [field]: value };
    setFormData({ ...formData, gateways: newGateways });
  };

  return (
    <div className="p-6 max-w-3xl mx-auto">
      <h2 className="text-2xl font-bold mb-6 text-gray-800">Settings</h2>

      {validationError && (
        <div className="mb-4 bg-red-50 border border-red-300 text-red-700 rounded p-3 text-sm">
          {validationError}
        </div>
      )}

      {clearError && (
        <div className="mb-4 bg-red-50 border border-red-300 text-red-700 rounded p-3 text-sm flex justify-between items-start">
          <span>Failed to clear credentials: {clearError}</span>
          <button onClick={() => setClearError(null)} className="ml-3 text-red-500 hover:text-red-700 font-bold leading-none">×</button>
        </div>
      )}

      <form onSubmit={handleSubmit} className="space-y-6">
        <div className="bg-white rounded-lg shadow p-6 space-y-4">
          <h3 className="text-lg font-semibold text-gray-700 mb-4">OPNsense Configuration</h3>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Base URL <span className="text-red-500">*</span>
            </label>
            <input
              type="text"
              value={formData.base_url}
              onChange={(e) => { setFormData({ ...formData, base_url: e.target.value }); setTestResult(null); }}
              className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
              placeholder="https://10.0.0.1:444"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              Local IP Override <span className="text-gray-400 font-normal">(optional)</span>
            </label>
            <input
              type="text"
              value={formData.interface_ip}
              onChange={(e) => { setFormData({ ...formData, interface_ip: e.target.value }); setTestResult(null); }}
              className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
              placeholder="e.g. 192.168.1.50 — leave blank to auto-detect"
            />
            <p className="text-xs text-gray-400 mt-1">
              Override the IP added to OPNsense aliases. Use when auto-detection picks the wrong interface (e.g. Docker, WSL, or multiple NICs).
            </p>
          </div>

          <div className="flex items-center gap-3">
            <button
              type="button"
              onClick={handleTestConnection}
              disabled={isTesting}
              className={`text-sm px-4 py-2 rounded transition-colors ${
                isTesting
                  ? "bg-gray-300 text-gray-500 cursor-not-allowed"
                  : "bg-gray-100 hover:bg-gray-200 text-gray-700 border border-gray-300"
              }`}
            >
              {isTesting ? "Testing..." : "Test Connection"}
            </button>
            {testResult ? (
              <span className={`text-xs ${testResult.ok ? "text-green-600" : "text-red-600"}`}>
                {testResult.message}
              </span>
            ) : (
              <span className="text-xs text-gray-400">Tests currently saved settings</span>
            )}
          </div>

          <div className="flex justify-between items-center">
            <span className="text-sm font-medium text-gray-700">
              API Credentials <span className="text-red-500">*</span>
            </span>
            <button
              type="button"
              onClick={() => setShowCredentials((v) => !v)}
              className="flex items-center gap-1 text-xs text-gray-500 hover:text-gray-700 transition-colors"
              aria-label={showCredentials ? "Hide credentials" : "Show credentials"}
            >
              <EyeIcon open={showCredentials} />
              {showCredentials ? "Hide" : "Show"}
            </button>
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              API Key <span className="text-red-500">*</span>
            </label>
            <input
              type={showCredentials ? "text" : "password"}
              value={formCredentials.api_key}
              onChange={(e) => { setFormCredentials({ ...formCredentials, api_key: e.target.value }); setTestResult(null); }}
              className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
              placeholder="Your OPNsense API Key"
            />
          </div>

          <div>
            <label className="block text-sm font-medium text-gray-700 mb-2">
              API Secret <span className="text-red-500">*</span>
            </label>
            <input
              type={showCredentials ? "text" : "password"}
              value={formCredentials.api_secret}
              onChange={(e) => { setFormCredentials({ ...formCredentials, api_secret: e.target.value }); setTestResult(null); }}
              className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
              placeholder="Your OPNsense API Secret"
            />
          </div>

          <div className="pt-2 border-t border-gray-100">
            <div className="flex items-center justify-between">
              <div>
                <span className="text-sm font-medium text-gray-700">Verify TLS Certificate</span>
                <p className="text-xs text-gray-400 mt-0.5">Disable only if OPNsense uses a self-signed cert</p>
              </div>
              <button
                type="button"
                onClick={() => { setFormData({ ...formData, verify_tls: !formData.verify_tls }); setTestResult(null); }}
                className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 ${
                  formData.verify_tls ? "bg-blue-600" : "bg-gray-300"
                }`}
                aria-label="Toggle TLS certificate verification"
                role="switch"
                aria-checked={formData.verify_tls}
              >
                <span className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  formData.verify_tls ? "translate-x-6" : "translate-x-1"
                }`} />
              </button>
            </div>
            {!formData.verify_tls && (
              <div className="mt-2 bg-yellow-50 border border-yellow-300 text-yellow-800 rounded p-2 text-xs">
                TLS verification is disabled — only use on a trusted network. Your API credentials are sent over an unverified connection.
              </div>
            )}
          </div>

          {/* Clear credentials — inline confirm, no native dialog */}
          <div className="pt-2 border-t border-gray-100">
            {!confirmClearCredentials ? (
              <button
                type="button"
                onClick={() => setConfirmClearCredentials(true)}
                className="text-sm text-red-500 hover:text-red-700 transition-colors"
              >
                Clear saved credentials…
              </button>
            ) : (
              <div className="flex items-center gap-3 bg-red-50 border border-red-200 rounded p-3">
                <span className="text-sm text-red-700 flex-1">
                  This will delete credentials from the OS keyring. You'll need to re-enter them to use the app.
                </span>
                <button
                  type="button"
                  onClick={handleClearCredentials}
                  disabled={isClearing}
                  className={`text-white text-sm px-3 py-1.5 rounded transition-colors ${
                    isClearing ? "bg-red-400 cursor-not-allowed" : "bg-red-600 hover:bg-red-700"
                  }`}
                >
                  {isClearing ? "Clearing..." : "Clear"}
                </button>
                <button
                  type="button"
                  onClick={() => setConfirmClearCredentials(false)}
                  disabled={isClearing}
                  className={`text-sm px-3 py-1.5 rounded transition-colors ${
                    isClearing ? "bg-gray-100 text-gray-400 cursor-not-allowed" : "bg-gray-200 hover:bg-gray-300 text-gray-800"
                  }`}
                >
                  Cancel
                </button>
              </div>
            )}
          </div>
        </div>

        <div className="bg-white rounded-lg shadow p-6">
          <div className="flex justify-between items-center mb-4">
            <h3 className="text-lg font-semibold text-gray-700">VPN Gateways</h3>
            <button
              type="button"
              onClick={addGateway}
              className="bg-green-500 hover:bg-green-600 text-white px-4 py-2 rounded transition-colors"
            >
              Add Gateway
            </button>
          </div>

          {formData.gateways.length === 0 ? (
            <p className="text-gray-500 text-center py-4">
              No gateways configured. Click "Add Gateway" to get started.
            </p>
          ) : (
            <div className="space-y-4">
              {formData.gateways.map((gateway, index) => (
                <div key={gateway.id || `gateway-${index}`} className="border border-gray-200 rounded-md p-4 space-y-3">
                  <div className="flex justify-between items-center mb-2">
                    <span className="text-sm font-medium text-gray-600">Gateway {index + 1}</span>
                    {pendingRemoveIndex === index ? (
                      <div className="flex items-center gap-2">
                        <span className="text-xs text-red-600">Remove "{gateway.display_name || `Gateway ${index + 1}`}"?</span>
                        <button
                          type="button"
                          onClick={() => doRemoveGateway(index)}
                          className="bg-red-500 hover:bg-red-600 text-white text-xs px-2 py-1 rounded transition-colors"
                        >
                          Remove
                        </button>
                        <button
                          type="button"
                          onClick={() => setPendingRemoveIndex(null)}
                          className="bg-gray-200 hover:bg-gray-300 text-gray-700 text-xs px-2 py-1 rounded transition-colors"
                        >
                          Cancel
                        </button>
                      </div>
                    ) : (
                      <button
                        type="button"
                        onClick={() => confirmRemoveGateway(index)}
                        className="text-red-500 hover:text-red-700 text-sm"
                      >
                        Remove
                      </button>
                    )}
                  </div>

                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">
                      Display Name <span className="text-red-500">*</span>
                    </label>
                    <input
                      type="text"
                      value={gateway.display_name}
                      onChange={(e) => updateGateway(index, "display_name", e.target.value)}
                      className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                      placeholder="e.g., US East VPN"
                    />
                  </div>

                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">
                      Gateway Name <span className="text-red-500">*</span>
                    </label>
                    <input
                      type="text"
                      value={gateway.gateway_name}
                      onChange={(e) => updateGateway(index, "gateway_name", e.target.value)}
                      className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                      placeholder="OPNsense gateway name (e.g. WAN_VPN)"
                    />
                    <p className="text-xs text-gray-400 mt-1">Used to check if the gateway is online</p>
                  </div>

                  <div>
                    <label className="block text-sm font-medium text-gray-700 mb-1">
                      Alias Name <span className="text-red-500">*</span>
                    </label>
                    <input
                      type="text"
                      value={gateway.alias_name}
                      onChange={(e) => updateGateway(index, "alias_name", e.target.value)}
                      className="w-full px-3 py-2 border border-gray-300 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
                      placeholder="Firewall alias name (e.g. vpn_devices)"
                    />
                    <p className="text-xs text-gray-400 mt-1">Used to add/remove this device's IP for VPN routing</p>
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>

        <div className="flex space-x-3">
          <button
            type="submit"
            disabled={isSaving}
            className={`flex-1 font-semibold py-3 rounded-lg transition-colors ${
              isSaving
                ? "bg-blue-400 text-white cursor-not-allowed"
                : "bg-blue-600 hover:bg-blue-700 text-white"
            }`}
          >
            {isSaving ? "Saving..." : "Save Settings"}
          </button>
          <button
            type="button"
            onClick={onCancel}
            disabled={isSaving}
            className={`flex-1 font-semibold py-3 rounded-lg transition-colors ${
              isSaving
                ? "bg-gray-200 text-gray-400 cursor-not-allowed"
                : "bg-gray-300 hover:bg-gray-400 text-gray-800"
            }`}
          >
            Cancel
          </button>
        </div>
      </form>
    </div>
  );
}

export default Settings;
