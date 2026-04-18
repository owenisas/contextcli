import { useEffect, useState, useCallback } from "react";
import { api } from "./lib/api";
import type { App as AppType, Profile, AdapterInfo } from "./lib/types";
import Sidebar from "./components/Sidebar";
import AppDetail from "./components/AppDetail";
import StatusBar from "./components/StatusBar";

export default function App() {
  const [apps, setApps] = useState<AppType[]>([]);
  const [selectedAppId, setSelectedAppId] = useState<string | null>(null);
  const [profilesMap, setProfilesMap] = useState<Record<string, Profile[]>>({});
  const [adapterInfoMap, setAdapterInfoMap] = useState<Record<string, AdapterInfo>>({});
  const [error, setError] = useState<string | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);

  // Full reload of all data
  const refreshAll = useCallback(async () => {
    try {
      const loadedApps = await api.listApps();
      setApps(loadedApps);
      if (loadedApps.length > 0 && !selectedAppId) {
        setSelectedAppId(loadedApps[0].id);
      }
      // Load all profiles in parallel
      const profileEntries = await Promise.all(
        loadedApps.map(async (app) => {
          const profiles = await api.listProfiles(app.id);
          return [app.id, profiles] as const;
        })
      );
      setProfilesMap(Object.fromEntries(profileEntries));
    } catch (e) {
      setError(String(e));
    }
  }, [selectedAppId]);

  // Load on mount + when refreshKey changes
  useEffect(() => {
    refreshAll();
  }, [refreshKey]);

  // Auto-refresh on window focus (catches CLI changes)
  useEffect(() => {
    const onFocus = () => setRefreshKey((k) => k + 1);
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, []);

  // Load adapter info for selected app
  useEffect(() => {
    if (selectedAppId && !adapterInfoMap[selectedAppId]) {
      api.getAdapterInfo(selectedAppId).then((info) => {
        setAdapterInfoMap((prev) => ({ ...prev, [selectedAppId]: info }));
      });
    }
  }, [selectedAppId]);

  const refreshProfiles = useCallback(
    async (appId: string) => {
      const profiles = await api.listProfiles(appId);
      setProfilesMap((prev) => ({ ...prev, [appId]: profiles }));
    },
    []
  );

  const handleCreateProfile = useCallback(
    async (profileName: string, label?: string) => {
      if (!selectedAppId) return;
      await api.createProfile(selectedAppId, profileName, label);
      await refreshProfiles(selectedAppId);
    },
    [selectedAppId, refreshProfiles]
  );

  const handleSetDefault = useCallback(
    async (profileName: string) => {
      if (!selectedAppId) return;
      await api.setDefault(selectedAppId, profileName);
      await refreshProfiles(selectedAppId);
    },
    [selectedAppId, refreshProfiles]
  );

  const handleValidate = useCallback(
    async (profileName: string) => {
      if (!selectedAppId) return;
      await api.validateProfile(selectedAppId, profileName);
      await refreshProfiles(selectedAppId);
    },
    [selectedAppId, refreshProfiles]
  );

  const handleDelete = useCallback(
    async (profileName: string) => {
      if (!selectedAppId) return;
      await api.deleteProfile(selectedAppId, profileName);
      await refreshProfiles(selectedAppId);
    },
    [selectedAppId, refreshProfiles]
  );

  const handleLogout = useCallback(
    async (profileName: string) => {
      if (!selectedAppId) return;
      await api.triggerLogout(selectedAppId, profileName);
      await refreshProfiles(selectedAppId);
    },
    [selectedAppId, refreshProfiles]
  );

  const handleImport = useCallback(
    async (profileName: string) => {
      if (!selectedAppId) return;
      await api.importProfile(selectedAppId, profileName);
      await refreshProfiles(selectedAppId);
    },
    [selectedAppId, refreshProfiles]
  );

  const handleRename = useCallback(
    async (oldName: string, newName: string) => {
      if (!selectedAppId) return;
      await api.renameProfile(selectedAppId, oldName, newName);
      await refreshProfiles(selectedAppId);
    },
    [selectedAppId, refreshProfiles]
  );

  const selectedApp = apps.find((a) => a.id === selectedAppId) ?? null;
  const selectedProfiles = selectedAppId ? profilesMap[selectedAppId] ?? [] : [];
  const selectedAdapterInfo = selectedAppId ? adapterInfoMap[selectedAppId] ?? null : null;

  return (
    <div className="h-full flex flex-col">
      {/* Error banner */}
      {error && (
        <div className="bg-danger/15 border-b border-danger/30 px-4 py-2 text-xs text-danger flex justify-between items-center">
          <span>{error}</span>
          <button onClick={() => setError(null)} className="text-danger/60 hover:text-danger">
            ✕
          </button>
        </div>
      )}

      <div className="flex-1 flex min-h-0">
        <Sidebar
          apps={apps}
          profilesMap={profilesMap}
          selectedAppId={selectedAppId}
          onSelect={setSelectedAppId}
          onRefresh={() => setRefreshKey((k) => k + 1)}
        />

        {selectedApp ? (
          <AppDetail
            app={selectedApp}
            profiles={selectedProfiles}
            adapterInfo={selectedAdapterInfo}
            onCreateProfile={handleCreateProfile}
            onSetDefault={handleSetDefault}
            onValidate={handleValidate}
            onDelete={handleDelete}
            onLogout={handleLogout}
            onImport={handleImport}
            onRename={handleRename}
          />
        ) : (
          <div className="flex-1 flex items-center justify-center text-text-secondary text-sm">
            Select an app from the sidebar
          </div>
        )}
      </div>

      <StatusBar apps={apps} profilesMap={profilesMap} />
    </div>
  );
}
