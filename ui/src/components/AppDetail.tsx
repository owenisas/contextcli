import { useState, useEffect } from "react";
import type { App, Profile, AdapterInfo, ProjectLink, AuthCapabilities } from "../lib/types";
import { api } from "../lib/api";
import ProfileCard from "./ProfileCard";
import AddProfileDialog from "./AddProfileDialog";

interface AppDetailProps {
  app: App;
  profiles: Profile[];
  adapterInfo: AdapterInfo | null;
  onCreateProfile: (profileName: string, label?: string) => Promise<void>;
  onSetDefault: (profileName: string) => Promise<void>;
  onValidate: (profileName: string) => Promise<void>;
  onDelete: (profileName: string) => Promise<void>;
  onLogout: (profileName: string) => Promise<void>;
  onImport: (profileName: string) => Promise<void>;
  onRename: (oldName: string, newName: string) => Promise<void>;
}

const AUTH_BADGES: { key: keyof AuthCapabilities; label: string; tip: string }[] = [
  { key: "interactive_login", label: "Login", tip: "Interactive CLI login flow" },
  { key: "manual_token", label: "Manual", tip: "Paste a token or API key" },
  { key: "import_file", label: "File", tip: "Import from native config file" },
  { key: "import_keychain", label: "Keychain", tip: "Import from macOS Keychain" },
  { key: "import_command", label: "Command", tip: "Import via token command" },
  { key: "multi_account", label: "Multi-acct", tip: "Multi-account import" },
  { key: "config_dir_isolation", label: "Config dir", tip: "Isolated config per profile" },
  { key: "validate_whoami", label: "Whoami", tip: "Can validate identity" },
];

function AuthBadges({ auth }: { auth: AuthCapabilities }) {
  const authPaths = [
    auth.interactive_login,
    auth.manual_token,
    auth.import_file,
    auth.import_keychain,
    auth.import_command,
  ].filter(Boolean).length;

  const isLoginOnly = authPaths <= 1;

  return (
    <div className="mt-2">
      <div className="flex flex-wrap gap-1.5">
        {AUTH_BADGES.map(({ key, label, tip }) => {
          const enabled = auth[key];
          return (
            <span
              key={key}
              title={tip}
              className={`text-[10px] px-1.5 py-0.5 rounded font-medium transition-colors ${
                enabled
                  ? "bg-accent/12 text-accent"
                  : "bg-surface text-text-secondary/30"
              }`}
            >
              {label}
            </span>
          );
        })}
      </div>
      {isLoginOnly && (
        <p className="text-[10px] text-warning mt-1.5">
          Limited auth — logout forces full re-authentication. No import or manual token path.
        </p>
      )}
    </div>
  );
}

export default function AppDetail({
  app,
  profiles,
  adapterInfo,
  onCreateProfile,
  onSetDefault,
  onValidate,
  onDelete,
  onLogout,
  onImport,
  onRename,
}: AppDetailProps) {
  const [showAddDialog, setShowAddDialog] = useState(false);
  const [projectLinks, setProjectLinks] = useState<ProjectLink[]>([]);
  const defaultProfile = profiles.find((p) => p.is_default);

  // Load project links for this app
  useEffect(() => {
    api.listProjectLinks(app.id).then(setProjectLinks).catch(() => {});
  }, [app.id, profiles]);

  return (
    <div className="flex-1 overflow-y-auto p-6">
      {/* Header */}
      <div className="mb-6">
        <h2 className="text-xl font-semibold">{app.display_name}</h2>
        <div className="flex items-center gap-4 mt-2 text-sm text-text-secondary">
          <span className="flex items-center gap-1.5">
            {app.binary_path ? (
              <>
                <span className="text-success">✓</span>
                <span className="font-mono text-xs">{app.binary_path}</span>
              </>
            ) : (
              <>
                <span className="text-danger">✗</span>
                <span>not installed</span>
              </>
            )}
          </span>
          {adapterInfo && (
            <>
              <span className="text-border">·</span>
              <span>Tier {adapterInfo.support_level.replace("tier", "")}</span>
            </>
          )}
        </div>

        {/* Auth capability badges */}
        {adapterInfo?.auth && (
          <AuthBadges auth={adapterInfo.auth} />
        )}
      </div>

      {/* Profiles section */}
      <div className="mb-6">
        <div className="flex items-center justify-between mb-3">
          <h3 className="text-sm font-semibold text-text-secondary uppercase tracking-wider">
            Profiles
          </h3>
          <button
            onClick={() => setShowAddDialog(true)}
            className="text-xs px-3 py-1.5 rounded-lg bg-accent/10 text-accent hover:bg-accent/20 transition-colors"
          >
            + Add Profile
          </button>
        </div>

        {profiles.length === 0 ? (
          <div className="border border-dashed border-border rounded-lg p-8 text-center">
            <p className="text-sm text-text-secondary">No profiles yet</p>
            <p className="text-xs text-text-secondary mt-1">
              Create a profile or import from your existing CLI config
            </p>
            <button
              onClick={() => onImport("default")}
              className="mt-3 text-xs px-3 py-1.5 rounded-lg bg-accent/10 text-accent hover:bg-accent/20 transition-colors"
            >
              Import from CLI
            </button>
          </div>
        ) : (
          <div className="space-y-2">
            {profiles.map((p) => (
              <ProfileCard
                key={p.id}
                profile={p}
                projectLinks={projectLinks.filter(
                  (l) => l.profile_name === p.profile_name
                )}
                onSetDefault={() => onSetDefault(p.profile_name)}
                onValidate={() => onValidate(p.profile_name)}
                onDelete={() => onDelete(p.profile_name)}
                onLogout={() => onLogout(p.profile_name)}
                onImport={() => onImport(p.profile_name)}
                onRename={(newName) => onRename(p.profile_name, newName)}
              />
            ))}
          </div>
        )}
      </div>

      {/* Project Links summary */}
      {projectLinks.length > 0 && (
        <div className="mb-6">
          <h3 className="text-sm font-semibold text-text-secondary uppercase tracking-wider mb-3">
            Project Mappings
          </h3>
          <div className="space-y-1">
            {projectLinks.map((link) => (
              <div
                key={link.project_dir}
                className="flex items-center gap-3 px-3 py-2 rounded-lg bg-surface text-sm group"
              >
                <span className="text-accent text-xs">📁</span>
                <span className="font-mono text-xs text-text-secondary truncate flex-1">
                  {link.project_dir.replace(/^\/Users\/[^/]+/, "~")}
                </span>
                <span className="text-xs text-text-primary">
                  → {link.profile_name}
                </span>
                <div className="hidden group-hover:flex items-center gap-1">
                  <button
                    onClick={() => api.openDirectory(link.project_dir)}
                    title="Open in Finder"
                    className="text-[10px] px-1.5 py-0.5 rounded bg-surface-hover hover:bg-accent/10 hover:text-accent transition-colors"
                  >
                    Finder
                  </button>
                  <button
                    onClick={() => api.openTerminalAt(link.project_dir)}
                    title="Open in Terminal"
                    className="text-[10px] px-1.5 py-0.5 rounded bg-surface-hover hover:bg-accent/10 hover:text-accent transition-colors"
                  >
                    Terminal
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Quick Start */}
      {profiles.length > 0 && (
        <div>
          <h3 className="text-sm font-semibold text-text-secondary uppercase tracking-wider mb-3">
            Quick Start
          </h3>
          <div className="bg-[#0a0a0a] border border-border rounded-lg p-4 font-mono text-xs text-text-secondary">
            <div className="text-text-primary">
              <span className="text-accent">$</span> contextcli --app {app.id}
              {defaultProfile && (
                <span> --profile {defaultProfile.profile_name}</span>
              )}{" "}
              <span className="text-text-secondary">&lt;command&gt;</span>
            </div>
          </div>
        </div>
      )}

      {/* Add Profile Dialog */}
      {showAddDialog && (
        <AddProfileDialog
          appId={app.id}
          appName={app.display_name}
          onCreated={onCreateProfile}
          onClose={() => setShowAddDialog(false)}
        />
      )}
    </div>
  );
}
