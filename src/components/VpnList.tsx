import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AppSettings } from "../App";

interface VpnStatus {
  gateway_name: string;
  alias_name: string;
  display_name: string;
  enabled: boolean;
  online: boolean;
  gateway_status: string; // "online" | "latency" | "offline" | "unknown"
  rtt?: string;
  rttd?: string;
  loss?: string;
  error?: string;
}

interface VpnListProps {
  settings: AppSettings;
}

const AUTO_REFRESH_MS = 30_000;

function VpnList({ settings }: VpnListProps) {
  const [vpnStatuses, setVpnStatuses] = useState<VpnStatus[]>([]);
  const [loading, setLoading] = useState(true); // true on mount — fetch fires immediately
  const [toggling, setToggling] = useState<string | null>(null);
  const [fetchError, setFetchError] = useState<string | null>(null);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const refreshStatuses = useCallback(async () => {
    if (settings.gateways.length === 0) {
      setVpnStatuses([]);
      setFetchError(null);
      return;
    }

    setLoading(true);
    try {
      const statuses = await invoke<VpnStatus[]>("get_all_vpn_status");
      setVpnStatuses(statuses);
      setFetchError(null);
    } catch (error) {
      setFetchError(String(error));
    } finally {
      setLoading(false);
    }
  }, [settings]);

  useEffect(() => { refreshStatuses(); }, [refreshStatuses]);

  useEffect(() => {
    intervalRef.current = setInterval(refreshStatuses, AUTO_REFRESH_MS);
    return () => { if (intervalRef.current) clearInterval(intervalRef.current); };
  }, [refreshStatuses]);

  const handleToggle = async (aliasName: string, currentState: boolean) => {
    setToggling(aliasName);
    try {
      await invoke("toggle_vpn", { aliasName, enable: !currentState });
      await refreshStatuses();
    } catch (error) {
      setFetchError(`Toggle failed: ${error}`);
    } finally {
      setToggling(null);
    }
  };

  const getStatusLabel = (vpn: VpnStatus) => {
    if (vpn.error) return "Error";
    if (vpn.gateway_status === "latency") return "Degraded";
    if (!vpn.online) return "Gateway Down";
    return vpn.enabled ? "Active" : "Inactive";
  };

  const getStatusColor = (vpn: VpnStatus) => {
    if (vpn.error) return "bg-red-500";
    if (!vpn.online) return "bg-red-400";
    if (vpn.gateway_status === "latency") return "bg-yellow-400";
    return vpn.enabled ? "bg-green-500" : "bg-gray-400";
  };

  const formatMetrics = (vpn: VpnStatus) => {
    const parts: string[] = [];
    if (vpn.rtt)  parts.push(`RTT ${vpn.rtt}`);
    if (vpn.rttd) parts.push(`±${vpn.rttd}`);
    if (vpn.loss) parts.push(`loss ${vpn.loss}`);
    return parts.join("  ·  ");
  };

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

      {fetchError && (
        <div className="mb-4 bg-red-50 border border-red-300 text-red-700 rounded p-3 flex justify-between items-start">
          <span className="text-sm">{fetchError}</span>
          <button onClick={() => setFetchError(null)} className="ml-3 text-red-500 hover:text-red-700 font-bold leading-none">×</button>
        </div>
      )}

      <div className="space-y-3">
        {vpnStatuses.map((vpn) => {
          const stableKey = `${vpn.gateway_name}::${vpn.alias_name}`;
          const metrics = formatMetrics(vpn);
          return (
            <div key={stableKey} className="bg-white rounded-lg shadow p-4 flex items-center justify-between">
              <div className="flex-1 min-w-0">
                <h3 className="font-semibold text-gray-800">{vpn.display_name}</h3>
                <p className="text-xs text-gray-400">
                  Gateway: {vpn.gateway_name} · Alias: {vpn.alias_name}
                </p>
                {metrics && (
                  <p className="text-xs text-gray-500 mt-0.5 font-mono">{metrics}</p>
                )}
                {vpn.error && (
                  <p className="text-xs text-red-500 mt-1">{vpn.error}</p>
                )}
              </div>

              <div className="flex items-center space-x-3 ml-3 shrink-0">
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
                  <span className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                    vpn.enabled ? "translate-x-6" : "translate-x-1"
                  }`} />
                </button>
              </div>
            </div>
          );
        })}
      </div>

      <p className="text-xs text-gray-400 mt-4 text-center">Auto-refreshes every 30s</p>
    </div>
  );
}

export default VpnList;
