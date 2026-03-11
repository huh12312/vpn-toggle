import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AppSettings } from "../App";

interface VpnStatus {
  gateway_name: string;
  alias_name: string;
  display_name: string;
  enabled: boolean; // device IP is in the alias
  online: boolean;  // OPNsense gateway is up
  error?: string;
}

interface VpnListProps {
  settings: AppSettings;
}

function VpnList({ settings }: VpnListProps) {
  const [vpnStatuses, setVpnStatuses] = useState<VpnStatus[]>([]);
  const [loading, setLoading] = useState(false);
  const [toggling, setToggling] = useState<string | null>(null);

  useEffect(() => {
    refreshStatuses();
  }, [settings]);

  const refreshStatuses = async () => {
    if (settings.gateways.length === 0) {
      setVpnStatuses([]);
      return;
    }

    setLoading(true);
    try {
      const statuses = await invoke<VpnStatus[]>("get_all_vpn_status");
      setVpnStatuses(statuses);
    } catch (error) {
      console.error("Failed to get VPN statuses:", error);
      alert("Failed to get VPN statuses: " + error);
    } finally {
      setLoading(false);
    }
  };

  const handleToggle = async (aliasName: string, currentState: boolean) => {
    setToggling(aliasName);
    try {
      await invoke("toggle_vpn", {
        aliasName: aliasName,
        enable: !currentState,
      });
      // Refresh status after toggle
      await refreshStatuses();
    } catch (error) {
      console.error("Failed to toggle VPN:", error);
      alert("Failed to toggle VPN: " + error);
    } finally {
      setToggling(null);
    }
  };

  const getStatusLabel = (vpn: VpnStatus) => {
    if (vpn.error) return "Error";
    if (!vpn.online) return "Gateway Down";
    return vpn.enabled ? "Active" : "Inactive";
  };

  const getStatusColor = (vpn: VpnStatus) => {
    if (vpn.error) return "bg-red-500";
    if (!vpn.online) return "bg-yellow-500";
    return vpn.enabled ? "bg-green-500" : "bg-gray-400";
  };

  if (!settings.api_key || !settings.api_secret) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-center text-gray-600">
          <p className="text-lg mb-2">Please configure your API credentials</p>
          <p className="text-sm">Click the Settings button above to get started</p>
        </div>
      </div>
    );
  }

  if (settings.gateways.length === 0) {
    return (
      <div className="flex items-center justify-center h-full">
        <div className="text-center text-gray-600">
          <p className="text-lg mb-2">No VPN gateways configured</p>
          <p className="text-sm">Click the Settings button to add gateways</p>
        </div>
      </div>
    );
  }

  return (
    <div className="p-4">
      <div className="mb-4 flex justify-between items-center">
        <h2 className="text-lg font-semibold text-gray-700">VPN Gateways</h2>
        <button
          onClick={refreshStatuses}
          disabled={loading}
          className="bg-blue-500 hover:bg-blue-600 text-white px-4 py-2 rounded disabled:bg-gray-400 disabled:cursor-not-allowed transition-colors"
        >
          {loading ? "Refreshing..." : "Refresh"}
        </button>
      </div>

      <div className="space-y-3">
        {vpnStatuses.map((vpn) => (
          <div
            key={vpn.alias_name}
            className="bg-white rounded-lg shadow p-4 flex items-center justify-between"
          >
            <div className="flex-1">
              <h3 className="font-semibold text-gray-800">{vpn.display_name}</h3>
              <p className="text-xs text-gray-400">
                Gateway: {vpn.gateway_name} · Alias: {vpn.alias_name}
              </p>
              {vpn.error && (
                <p className="text-sm text-red-500 mt-1">Error: {vpn.error}</p>
              )}
            </div>

            <div className="flex items-center space-x-3">
              <div className="flex items-center">
                <div className={`w-3 h-3 rounded-full mr-2 ${getStatusColor(vpn)}`} />
                <span className="text-sm text-gray-600">{getStatusLabel(vpn)}</span>
              </div>

              <button
                onClick={() => handleToggle(vpn.alias_name, vpn.enabled)}
                disabled={toggling === vpn.alias_name || !!vpn.error || !vpn.online}
                className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-offset-2 disabled:opacity-50 disabled:cursor-not-allowed ${
                  vpn.enabled ? "bg-blue-600" : "bg-gray-300"
                }`}
              >
                <span
                  className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                    vpn.enabled ? "translate-x-6" : "translate-x-1"
                  }`}
                />
              </button>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

export default VpnList;
