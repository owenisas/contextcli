export type AuthState = "unauthenticated" | "authenticated" | "expired" | "error";

export interface App {
  id: string;
  display_name: string;
  binary_path: string | null;
  adapter_version: string;
  support_level: string;
  created_at: string;
  updated_at: string;
}

export interface Profile {
  id: string;
  app_id: string;
  profile_name: string;
  label: string | null;
  is_default: boolean;
  auth_state: AuthState;
  auth_user: string | null;
  config_dir: string | null;
  created_at: string;
  updated_at: string;
  /** True when the credential requires one-time macOS Keychain authorization. */
  needs_keychain_auth: boolean;
}

export interface ValidationResult {
  valid: boolean;
  identity: string | null;
  message: string | null;
}

export interface ProjectLink {
  project_dir: string;
  app_id: string;
  profile_name: string;
}

export interface AdapterInfo {
  id: string;
  display_name: string;
  binary_names: string[];
  support_level: string;
}
