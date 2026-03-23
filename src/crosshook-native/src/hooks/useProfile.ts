import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { GameProfile } from "../types";

export interface UseProfileResult {
  profiles: string[];
  selectedProfile: string;
  profileName: string;
  profile: GameProfile;
  dirty: boolean;
  loading: boolean;
  saving: boolean;
  deleting: boolean;
  error: string | null;
  profileExists: boolean;
  setProfileName: (name: string) => void;
  selectProfile: (name: string) => Promise<void>;
  updateProfile: (updater: (current: GameProfile) => GameProfile) => void;
  saveProfile: () => Promise<void>;
  deleteProfile: () => Promise<void>;
  refreshProfiles: () => Promise<void>;
}

function createEmptyProfile(): GameProfile {
  return {
    game: {
      name: "",
      executable_path: "",
    },
    trainer: {
      path: "",
      type: "",
    },
    injection: {
      dll_paths: [],
      inject_on_launch: [false, false],
    },
    steam: {
      enabled: false,
      app_id: "",
      compatdata_path: "",
      proton_path: "",
      launcher: {
        icon_path: "",
        display_name: "",
      },
    },
    launch: {
      method: "",
    },
  };
}

export function useProfile(): UseProfileResult {
  const [profiles, setProfiles] = useState<string[]>([]);
  const [selectedProfile, setSelectedProfile] = useState("");
  const [profileName, setProfileName] = useState("");
  const [profile, setProfile] = useState<GameProfile>(createEmptyProfile);
  const [dirty, setDirty] = useState(false);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const loadProfile = useCallback(async (name: string) => {
    const trimmed = name.trim();
    if (!trimmed) {
      setSelectedProfile("");
      setProfileName("");
      setProfile(createEmptyProfile());
      setDirty(false);
      return;
    }

    setLoading(true);
    setError(null);

    try {
      const loaded = await invoke<GameProfile>("profile_load", { name: trimmed });
      setSelectedProfile(trimmed);
      setProfileName(trimmed);
      setProfile(loaded);
      setDirty(false);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  const refreshProfiles = useCallback(async () => {
    const names = await invoke<string[]>("profile_list");
    setProfiles(names);

    if (names.length === 0) {
      setSelectedProfile("");
      setProfileName("");
      setProfile(createEmptyProfile());
      setDirty(false);
      return;
    }

    if (selectedProfile && names.includes(selectedProfile)) {
      return;
    }

    await loadProfile(names[0]);
  }, [loadProfile, selectedProfile]);

  const selectProfile = useCallback(
    async (name: string) => {
      await loadProfile(name);
    },
    [loadProfile],
  );

  const updateProfile = useCallback((updater: (current: GameProfile) => GameProfile) => {
    setProfile((current) => updater(current));
    setDirty(true);
  }, []);

  const saveProfile = useCallback(async () => {
    const name = profileName.trim();
    if (!name) {
      setError("Profile name is required.");
      return;
    }

    setSaving(true);
    setError(null);

    try {
      await invoke("profile_save", { name, data: profile });
      await refreshProfiles();
      await loadProfile(name);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  }, [profile, profileName, refreshProfiles]);

  const deleteProfile = useCallback(async () => {
    const name = profileName.trim();
    if (!name) {
      setError("Select a profile to delete.");
      return;
    }

    setDeleting(true);
    setError(null);

    try {
      await invoke("profile_delete", { name });
      const names = await invoke<string[]>("profile_list");
      setProfiles(names);

      if (names.length === 0) {
        setSelectedProfile("");
        setProfileName("");
        setProfile(createEmptyProfile());
        setDirty(false);
        return;
      }

      await loadProfile(names[0]);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setDeleting(false);
    }
  }, [loadProfile, profileName]);

  useEffect(() => {
    void refreshProfiles().catch((err: unknown) => {
      setError(err instanceof Error ? err.message : String(err));
    });
  }, [refreshProfiles]);

  const profileExists = useMemo(
    () => profiles.includes(profileName.trim()),
    [profileName, profiles],
  );

  return {
    profiles,
    selectedProfile,
    profileName,
    profile,
    dirty,
    loading,
    saving,
    deleting,
    error,
    profileExists,
    setProfileName,
    selectProfile,
    updateProfile,
    saveProfile,
    deleteProfile,
    refreshProfiles,
  };
}
