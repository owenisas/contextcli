import { useState } from "react";

interface AddProfileDialogProps {
  appId: string;
  appName: string;
  onCreated: (profileName: string, label?: string) => Promise<void>;
  onClose: () => void;
}

export default function AddProfileDialog({
  appId,
  appName,
  onCreated,
  onClose,
}: AddProfileDialogProps) {
  const [name, setName] = useState("");
  const [label, setLabel] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;
    setLoading(true);
    setError(null);
    try {
      await onCreated(name.trim(), label.trim() || undefined);
      onClose();
    } catch (err) {
      setError(String(err));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-surface border border-border rounded-xl p-6 w-96 shadow-2xl">
        <h2 className="text-base font-semibold mb-4">
          Add Profile — {appName}
        </h2>

        <form onSubmit={handleSubmit}>
          <div className="space-y-3">
            <div>
              <label className="block text-xs text-text-secondary mb-1">
                Profile Name
              </label>
              <input
                type="text"
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="e.g. work, personal, client-a"
                autoFocus
                className="w-full bg-[#0a0a0a] border border-border rounded-lg px-3 py-2 text-sm text-text-primary placeholder:text-text-secondary/50 focus:outline-none focus:border-accent"
              />
            </div>
            <div>
              <label className="block text-xs text-text-secondary mb-1">
                Label (optional)
              </label>
              <input
                type="text"
                value={label}
                onChange={(e) => setLabel(e.target.value)}
                placeholder="e.g. Acme Corp, My Personal"
                className="w-full bg-[#0a0a0a] border border-border rounded-lg px-3 py-2 text-sm text-text-primary placeholder:text-text-secondary/50 focus:outline-none focus:border-accent"
              />
            </div>
          </div>

          {error && (
            <div className="mt-3 text-xs text-danger bg-danger/10 rounded-lg px-3 py-2">
              {error}
            </div>
          )}

          <div className="flex justify-end gap-2 mt-5">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 text-sm rounded-lg bg-surface-hover hover:bg-border transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={loading || !name.trim()}
              className="px-4 py-2 text-sm rounded-lg bg-accent text-white hover:bg-accent/80 transition-colors disabled:opacity-50"
            >
              {loading ? "Creating..." : "Create"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
