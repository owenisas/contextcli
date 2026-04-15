import type { AuthState } from "../lib/types";

const config: Record<AuthState, { bg: string; text: string; label: string }> = {
  authenticated: { bg: "bg-success/15", text: "text-success", label: "authenticated" },
  unauthenticated: { bg: "bg-warning/15", text: "text-warning", label: "unauthenticated" },
  expired: { bg: "bg-danger/15", text: "text-danger", label: "expired" },
  error: { bg: "bg-danger/15", text: "text-danger", label: "error" },
};

export default function StatusBadge({ state }: { state: AuthState }) {
  const c = config[state] ?? config.error;
  return (
    <span className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${c.bg} ${c.text}`}>
      <span className={`w-1.5 h-1.5 rounded-full mr-1.5 ${c.text.replace("text-", "bg-")}`} />
      {c.label}
    </span>
  );
}
