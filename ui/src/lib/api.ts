import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type { App, Profile, ValidationResult, AdapterInfo, ProjectLink } from "./types";

// Detect if running inside Tauri webview
const isTauri = typeof window !== "undefined" && !!(window as any).__TAURI_INTERNALS__;

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri) {
    try {
      return await tauriInvoke<T>(cmd, args);
    } catch (e) {
      console.error(`IPC ${cmd} failed:`, e);
      throw e;
    }
  }
  // Browser dev mode — return mock data
  return mockInvoke<T>(cmd, args);
}

// ── Mock data for browser dev/testing ────────────────────

const MOCK_APPS: App[] = [
  {
    id: "vercel",
    display_name: "Vercel",
    binary_path: "/opt/homebrew/bin/vercel",
    adapter_version: "0.1.0",
    support_level: "tier1",
    created_at: "2025-01-01T00:00:00",
    updated_at: "2025-01-01T00:00:00",
  },
  {
    id: "gh",
    display_name: "GitHub CLI",
    binary_path: "/opt/homebrew/bin/gh",
    adapter_version: "0.1.0",
    support_level: "tier1",
    created_at: "2025-01-01T00:00:00",
    updated_at: "2025-01-01T00:00:00",
  },
  {
    id: "supabase",
    display_name: "Supabase",
    binary_path: "/opt/homebrew/bin/supabase",
    adapter_version: "0.1.0",
    support_level: "tier1",
    created_at: "2025-01-01T00:00:00",
    updated_at: "2025-01-01T00:00:00",
  },
];

let mockProfiles: Record<string, Profile[]> = {
  vercel: [
    {
      id: "1",
      app_id: "vercel",
      profile_name: "work",
      label: "Work Account",
      is_default: true,
      auth_state: "authenticated",
      auth_user: "owenisas",
      config_dir: null,
      created_at: "2025-01-01T00:00:00",
      updated_at: "2025-01-01T00:00:00",
    },
    {
      id: "2",
      app_id: "vercel",
      profile_name: "personal",
      label: null,
      is_default: false,
      auth_state: "authenticated",
      auth_user: "owen-personal",
      config_dir: null,
      created_at: "2025-01-01T00:00:00",
      updated_at: "2025-01-01T00:00:00",
    },
  ],
  gh: [
    {
      id: "3",
      app_id: "gh",
      profile_name: "default",
      label: "Auto-imported",
      is_default: true,
      auth_state: "authenticated",
      auth_user: "owenisas",
      config_dir: null,
      created_at: "2025-01-01T00:00:00",
      updated_at: "2025-01-01T00:00:00",
    },
  ],
  supabase: [
    {
      id: "4",
      app_id: "supabase",
      profile_name: "default",
      label: "Auto-imported",
      is_default: true,
      auth_state: "authenticated",
      auth_user: null,
      config_dir: null,
      created_at: "2025-01-01T00:00:00",
      updated_at: "2025-01-01T00:00:00",
    },
  ],
};

const MOCK_ADAPTER_INFO: Record<string, AdapterInfo> = {
  vercel: { id: "vercel", display_name: "Vercel", binary_names: ["vercel"], support_level: "tier1" },
  gh: { id: "gh", display_name: "GitHub CLI", binary_names: ["gh"], support_level: "tier1" },
  supabase: { id: "supabase", display_name: "Supabase", binary_names: ["supabase"], support_level: "tier1" },
};

async function mockInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  // Simulate small network delay
  await new Promise((r) => setTimeout(r, 50));

  const appId = args?.appId as string;
  const profileName = args?.profileName as string;

  switch (cmd) {
    case "list_apps":
      return MOCK_APPS as T;
    case "list_profiles":
      return (mockProfiles[appId] ?? []) as T;
    case "get_adapter_info":
      return (MOCK_ADAPTER_INFO[appId] ?? MOCK_ADAPTER_INFO.vercel) as T;
    case "create_profile": {
      const newProfile: Profile = {
        id: String(Date.now()),
        app_id: appId,
        profile_name: profileName,
        label: (args?.label as string) ?? null,
        is_default: false,
        auth_state: "unauthenticated",
        auth_user: null,
        config_dir: null,
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      };
      if (!mockProfiles[appId]) mockProfiles[appId] = [];
      mockProfiles[appId].push(newProfile);
      return newProfile as T;
    }
    case "set_default": {
      const profiles = mockProfiles[appId] ?? [];
      for (const p of profiles) p.is_default = p.profile_name === profileName;
      return undefined as T;
    }
    case "delete_profile": {
      mockProfiles[appId] = (mockProfiles[appId] ?? []).filter(
        (p) => p.profile_name !== profileName
      );
      return undefined as T;
    }
    case "validate_profile":
      return { valid: true, identity: "owenisas", message: null } as T;
    case "trigger_logout": {
      const prof = (mockProfiles[appId] ?? []).find((p) => p.profile_name === profileName);
      if (prof) {
        prof.auth_state = "unauthenticated";
        prof.auth_user = null;
      }
      return undefined as T;
    }
    case "import_profile":
      return true as T;
    case "list_project_links":
      if (appId === "vercel") {
        return [
          { project_dir: "/Users/user/projects/my-app", app_id: "vercel", profile_name: "work" },
          { project_dir: "/Users/user/projects/side-project", app_id: "vercel", profile_name: "personal" },
        ] as T;
      }
      return [] as T;
    default:
      throw new Error(`Unknown command: ${cmd}`);
  }
}

// ── Public API ───────────────────────────────────────────

export const api = {
  listApps: () => invoke<App[]>("list_apps"),

  getAdapterInfo: (appId: string) =>
    invoke<AdapterInfo>("get_adapter_info", { appId }),

  listProfiles: (appId: string) =>
    invoke<Profile[]>("list_profiles", { appId }),

  createProfile: (appId: string, profileName: string, label?: string) =>
    invoke<Profile>("create_profile", { appId, profileName, label }),

  setDefault: (appId: string, profileName: string) =>
    invoke<void>("set_default", { appId, profileName }),

  deleteProfile: (appId: string, profileName: string) =>
    invoke<void>("delete_profile", { appId, profileName }),

  validateProfile: (appId: string, profileName: string) =>
    invoke<ValidationResult>("validate_profile", { appId, profileName }),

  triggerLogout: (appId: string, profileName: string) =>
    invoke<void>("trigger_logout", { appId, profileName }),

  importProfile: (appId: string, profileName: string) =>
    invoke<boolean>("import_profile", { appId, profileName }),

  listProjectLinks: (appId: string) =>
    invoke<ProjectLink[]>("list_project_links", { appId }),

  renameProfile: (appId: string, oldName: string, newName: string) =>
    invoke<Profile>("rename_profile", { appId, oldName, newName }),

  openDirectory: (path: string) =>
    invoke<void>("open_directory", { path }),

  openTerminalAt: (path: string) =>
    invoke<void>("open_terminal_at", { path }),

  checkCliInstalled: () => invoke<boolean>("check_cli_installed"),

  installCli: () => invoke<string>("install_cli"),
};
