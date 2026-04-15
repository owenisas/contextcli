import type { Profile, ProjectLink } from "../lib/types";
import StatusBadge from "./StatusBadge";
import { useState } from "react";

interface ProfileCardProps {
  profile: Profile;
  projectLinks?: ProjectLink[];
  onSetDefault: () => void;
  onValidate: () => Promise<void>;
  onDelete: () => void;
  onLogout: () => void;
  onRename: (newName: string) => Promise<void>;
}

export default function ProfileCard({
  profile,
  projectLinks = [],
  onSetDefault,
  onValidate,
  onDelete,
  onLogout,
  onRename,
}: ProfileCardProps) {
  const [validating, setValidating] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [editing, setEditing] = useState(false);
  const [newName, setNewName] = useState(profile.profile_name);
  const [renameError, setRenameError] = useState<string | null>(null);

  const handleValidate = async () => {
    setValidating(true);
    try {
      await onValidate();
    } finally {
      setValidating(false);
    }
  };

  const handleRename = async () => {
    const trimmed = newName.trim();
    if (!trimmed || trimmed === profile.profile_name) {
      setEditing(false);
      setNewName(profile.profile_name);
      return;
    }
    setRenameError(null);
    try {
      await onRename(trimmed);
      setEditing(false);
    } catch (e) {
      setRenameError(String(e));
    }
  };

  return (
    <div className={`border rounded-lg p-4 transition-colors ${
      profile.needs_keychain_auth
        ? "border-warning/40 hover:border-warning/60"
        : "border-border hover:border-accent/30"
    }`}>
      {/* Keychain auth warning banner */}
      {profile.needs_keychain_auth && (
        <div className="flex items-start gap-2 mb-3 px-2.5 py-2 rounded bg-warning/8 border border-warning/20 text-warning text-xs">
          <span className="mt-0.5 shrink-0">⚠</span>
          <div>
            <span className="font-medium">Needs one-time keychain authorization.</span>
            <span className="text-warning/70 ml-1">
              Run any command with this profile and click <strong>Always Allow</strong> — never prompted again.
            </span>
          </div>
        </div>
      )}
      <div className="flex items-start justify-between">
        <div className="flex items-center gap-2">
          {/* Default star */}
          <span className={`text-sm ${profile.is_default ? "text-warning" : "text-border"}`}>
            {profile.is_default ? "★" : "☆"}
          </span>
          <div>
            <div className="flex items-center gap-2">
              {editing ? (
                <input
                  type="text"
                  value={newName}
                  onChange={(e) => setNewName(e.target.value)}
                  onBlur={handleRename}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") handleRename();
                    if (e.key === "Escape") {
                      setEditing(false);
                      setNewName(profile.profile_name);
                    }
                  }}
                  autoFocus
                  className="font-medium text-sm bg-[#0a0a0a] border border-accent rounded px-1.5 py-0.5 text-text-primary focus:outline-none w-32"
                />
              ) : (
                <span
                  className="font-medium text-sm cursor-pointer hover:text-accent transition-colors"
                  onClick={() => setEditing(true)}
                  title="Click to rename"
                >
                  {profile.profile_name}
                </span>
              )}
              {profile.is_default && !editing && (
                <span className="text-[10px] text-text-secondary uppercase tracking-wide">
                  default
                </span>
              )}
            </div>
            {renameError && (
              <div className="text-[10px] text-danger mt-0.5">{renameError}</div>
            )}
            {profile.auth_user && (
              <div className="text-xs text-text-secondary mt-0.5">{profile.auth_user}</div>
            )}
            {projectLinks.length > 0 && (
              <div className="flex flex-wrap gap-1 mt-1">
                {projectLinks.map((link) => (
                  <span
                    key={link.project_dir}
                    className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded bg-accent/8 text-[10px] text-accent/70 font-mono"
                    title={link.project_dir}
                  >
                    📁 {link.project_dir.split("/").pop()}
                  </span>
                ))}
              </div>
            )}
          </div>
        </div>
        <StatusBadge state={profile.auth_state} />
      </div>

      {/* Actions */}
      <div className="flex items-center gap-2 mt-3 pt-3 border-t border-border">
        {!profile.is_default && (
          <button
            onClick={onSetDefault}
            className="text-xs px-2 py-1 rounded bg-surface-hover hover:bg-accent/10 hover:text-accent transition-colors"
          >
            Set Default
          </button>
        )}
        <button
          onClick={handleValidate}
          disabled={validating}
          className="text-xs px-2 py-1 rounded bg-surface-hover hover:bg-accent/10 hover:text-accent transition-colors disabled:opacity-50"
        >
          {validating ? "Testing..." : "Test"}
        </button>
        {profile.auth_state === "authenticated" && (
          <button
            onClick={onLogout}
            className="text-xs px-2 py-1 rounded bg-surface-hover hover:bg-danger/10 hover:text-danger transition-colors"
          >
            Logout
          </button>
        )}

        {/* Delete with confirmation */}
        {confirmDelete ? (
          <div className="ml-auto flex items-center gap-1">
            <span className="text-xs text-danger mr-1">Delete?</span>
            <button
              onClick={onDelete}
              className="text-xs px-2 py-1 rounded bg-danger/15 text-danger hover:bg-danger/25 transition-colors"
            >
              Yes
            </button>
            <button
              onClick={() => setConfirmDelete(false)}
              className="text-xs px-2 py-1 rounded bg-surface-hover transition-colors"
            >
              No
            </button>
          </div>
        ) : (
          <button
            onClick={() => setConfirmDelete(true)}
            className="ml-auto text-xs px-2 py-1 rounded bg-surface-hover hover:bg-danger/10 hover:text-danger transition-colors"
          >
            Delete
          </button>
        )}
      </div>
    </div>
  );
}
