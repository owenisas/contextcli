import type { App, Profile } from "../lib/types";

interface StatusBarProps {
  apps: App[];
  profilesMap: Record<string, Profile[]>;
}

export default function StatusBar({ apps, profilesMap }: StatusBarProps) {
  const totalProfiles = Object.values(profilesMap).flat().length;
  const allProfiles = Object.values(profilesMap).flat();
  const unhealthyCount = allProfiles.filter(
    (p) => p.auth_state === "error" || p.auth_state === "expired"
  ).length;

  const healthText =
    unhealthyCount > 0
      ? `${unhealthyCount} issue${unhealthyCount > 1 ? "s" : ""}`
      : totalProfiles > 0
        ? "All healthy"
        : "No profiles";

  const healthColor =
    unhealthyCount > 0 ? "text-warning" : "text-text-secondary";

  return (
    <footer className="h-8 border-t border-border bg-surface flex items-center px-4 text-xs text-text-secondary gap-3">
      <span>{apps.length} app{apps.length !== 1 ? "s" : ""}</span>
      <span className="text-border">·</span>
      <span>{totalProfiles} profile{totalProfiles !== 1 ? "s" : ""}</span>
      <span className="text-border">·</span>
      <span className={healthColor}>{healthText}</span>
    </footer>
  );
}
