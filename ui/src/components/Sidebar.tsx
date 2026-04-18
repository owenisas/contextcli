import { useState, useEffect } from "react";
import type { App, Profile, InstallResult } from "../lib/types";
import { api } from "../lib/api";

interface SidebarProps {
  apps: App[];
  profilesMap: Record<string, Profile[]>;
  selectedAppId: string | null;
  onSelect: (appId: string) => void;
  onRefresh: () => void;
}

function appStatusRank(app: App, profiles: Profile[]): number {
  if (!app.binary_path) return 2; // gray — not installed
  const hasAuth = profiles.some((p) => p.auth_state === "authenticated");
  if (hasAuth) return 0; // green
  return 1; // amber — installed, no auth
}

function appStatusDot(app: App, profiles: Profile[]): string {
  const rank = appStatusRank(app, profiles);
  if (rank === 0) return "bg-success";
  if (rank === 1) return "bg-warning";
  return "bg-neutral-500";
}

function sortApps(apps: App[], profilesMap: Record<string, Profile[]>): App[] {
  return [...apps].sort((a, b) => {
    const rankA = appStatusRank(a, profilesMap[a.id] ?? []);
    const rankB = appStatusRank(b, profilesMap[b.id] ?? []);
    if (rankA !== rankB) return rankA - rankB;
    return a.display_name.localeCompare(b.display_name);
  });
}

export default function Sidebar({ apps, profilesMap, selectedAppId, onSelect, onRefresh }: SidebarProps) {
  const [cliInstalled, setCliInstalled] = useState<boolean | null>(null);
  const [installing, setInstalling] = useState(false);
  const [installResult, setInstallResult] = useState<InstallResult | null>(null);
  const [installError, setInstallError] = useState<string | null>(null);
  const [legacyInstall, setLegacyInstall] = useState<string | null>(null);

  useEffect(() => {
    api.checkCliInstalled().then(setCliInstalled).catch(() => setCliInstalled(null));
    api.detectLegacyInstall().then(setLegacyInstall).catch(() => setLegacyInstall(null));
  }, []);

  const handleInstallCli = async () => {
    setInstalling(true);
    setInstallResult(null);
    setInstallError(null);
    try {
      const result = await api.installCli();
      setCliInstalled(true);
      setInstallResult(result);
      setLegacyInstall(result.legacy_install_at);
    } catch (e) {
      setInstallError(String(e));
    } finally {
      setInstalling(false);
    }
  };

  return (
    <aside className="w-52 h-full border-r border-border bg-surface flex flex-col">
      <div className="px-4 pt-5 pb-3 flex items-center justify-between">
        <h1 className="text-sm font-semibold text-text-secondary uppercase tracking-wider">
          Apps
        </h1>
        <button
          onClick={onRefresh}
          title="Refresh"
          className="text-text-secondary hover:text-accent transition-colors text-sm"
        >
          ↻
        </button>
      </div>

      <nav className="flex-1 px-2 space-y-0.5 overflow-y-auto">
        {sortApps(apps, profilesMap).map((app) => {
          const profiles = profilesMap[app.id] ?? [];
          const isSelected = app.id === selectedAppId;
          return (
            <button
              key={app.id}
              onClick={() => onSelect(app.id)}
              className={`w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-sm transition-colors ${
                isSelected
                  ? "bg-accent/10 text-accent"
                  : "text-text-primary hover:bg-surface-hover"
              }`}
            >
              <span className={`w-2 h-2 rounded-full flex-shrink-0 ${appStatusDot(app, profiles)}`} />
              <span className="truncate">{app.display_name}</span>
              {profiles.length > 0 && (
                <span className="ml-auto text-xs text-text-secondary">{profiles.length}</span>
              )}
            </button>
          );
        })}
      </nav>

      <div className="px-2 pb-3 border-t border-border pt-2 space-y-2">
        {/* CLI install banner */}
        {cliInstalled === false && (
          <button
            onClick={handleInstallCli}
            disabled={installing}
            className="w-full px-3 py-2 text-xs rounded-lg bg-accent/10 text-accent hover:bg-accent/20 transition-colors disabled:opacity-50"
          >
            {installing ? "Installing..." : "Install CLI Tool"}
          </button>
        )}
        {installError && (
          <div className="px-3 py-1.5 text-[10px] text-danger bg-danger/10 rounded-lg break-words">
            {installError}
          </div>
        )}
        {cliInstalled === true && installResult && (
          <div className="px-3 py-2 text-[10px] text-success bg-success/10 rounded-lg space-y-1">
            <div>✓ Installed at <span className="font-mono">{installResult.path}</span></div>
            {installResult.needs_shell_restart && (
              <div className="text-text-secondary">
                Open a new terminal tab to use <span className="font-mono">contextcli</span>.
              </div>
            )}
            {installResult.path_shells_updated.length > 0 && (
              <div className="text-text-secondary">
                Updated {installResult.path_shells_updated.join(", ")}.
              </div>
            )}
          </div>
        )}
        {legacyInstall && (
          <div className="px-3 py-2 text-[10px] text-warning bg-warning/10 rounded-lg space-y-1">
            <div>Old install detected at <span className="font-mono">{legacyInstall}</span>.</div>
            <div className="text-text-secondary">Run <span className="font-mono">sudo rm {legacyInstall}</span> to remove it.</div>
          </div>
        )}

        <div className="px-3 py-1 text-xs text-text-secondary flex items-center justify-between">
          <span>v0.2.2</span>
          {cliInstalled === true && (
            <span className="text-[10px] text-success">CLI ✓</span>
          )}
        </div>
      </div>
    </aside>
  );
}
