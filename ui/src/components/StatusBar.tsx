import type { App, Profile } from "../lib/types";

interface StatusBarProps {
  apps: App[];
  profilesMap: Record<string, Profile[]>;
}

export default function StatusBar({ apps, profilesMap }: StatusBarProps) {
  const totalProfiles = Object.values(profilesMap).flat().length;
  const allProfiles = Object.values(profilesMap).flat();
  const now = Math.floor(Date.now() / 1000);

  const unhealthyCount = allProfiles.filter(
    (p) => p.auth_state === "error" || p.auth_state === "expired"
  ).length;

  const expiredTokens = allProfiles.filter(
    (p) => p.token_expires_at !== null && p.token_expires_at <= now
  ).length;

  const expiringSoon = allProfiles.filter(
    (p) =>
      p.token_expires_at !== null &&
      p.token_expires_at > now &&
      p.token_expires_at <= now + 7 * 86400
  ).length;

  const issueCount = unhealthyCount + expiredTokens;

  const healthText =
    issueCount > 0
      ? `${issueCount} issue${issueCount > 1 ? "s" : ""}`
      : totalProfiles > 0
        ? "All healthy"
        : "No profiles";

  const healthColor =
    issueCount > 0 ? "text-danger" : "text-text-secondary";

  return (
    <footer className="h-8 border-t border-border bg-surface flex items-center px-4 text-xs text-text-secondary gap-3">
      <span>{apps.length} app{apps.length !== 1 ? "s" : ""}</span>
      <span className="text-border">·</span>
      <span>{totalProfiles} profile{totalProfiles !== 1 ? "s" : ""}</span>
      <span className="text-border">·</span>
      <span className={healthColor}>{healthText}</span>
      {expiredTokens > 0 && (
        <>
          <span className="text-border">·</span>
          <span className="text-danger">{expiredTokens} expired token{expiredTokens > 1 ? "s" : ""}</span>
        </>
      )}
      {expiringSoon > 0 && (
        <>
          <span className="text-border">·</span>
          <span className="text-warning">{expiringSoon} expiring soon</span>
        </>
      )}
    </footer>
  );
}
