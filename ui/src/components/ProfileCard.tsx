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
}

export default function ProfileCard({
  profile,
  projectLinks = [],
  onSetDefault,
  onValidate,
  onDelete,
  onLogout,
}: ProfileCardProps) {
  const [validating, setValidating] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);

  const handleValidate = async () => {
    setValidating(true);
    try {
      await onValidate();
    } finally {
      setValidating(false);
    }
  };

  return (
    <div className="border border-border rounded-lg p-4 hover:border-accent/30 transition-colors">
      <div className="flex items-start justify-between">
        <div className="flex items-center gap-2">
          {/* Default star */}
          <span className={`text-sm ${profile.is_default ? "text-warning" : "text-border"}`}>
            {profile.is_default ? "★" : "☆"}
          </span>
          <div>
            <div className="flex items-center gap-2">
              <span className="font-medium text-sm">{profile.profile_name}</span>
              {profile.is_default && (
                <span className="text-[10px] text-text-secondary uppercase tracking-wide">
                  default
                </span>
              )}
            </div>
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
