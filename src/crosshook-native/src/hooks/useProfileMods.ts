import { useCallback, useEffect, useRef, useState } from 'react';
import { formatInvokeError } from '@/hooks/profile/formatInvokeError';
import { useDebounce } from '@/hooks/useDebounce';
import { callCommand } from '@/lib/ipc';
import type { LaunchValidationIssue } from '@/types/launch';
import type { DetectionScanReport, ProfileModInput, ProfileModRecord, ProfileModsResponse } from '@/types/mods';

export const METADATA_UNAVAILABLE_PREFIX = 'metadata_unavailable:';
export const NO_GAME_PATH_PREFIX = 'no_game_path:';

const UNAVAILABLE_MESSAGE = 'Mod registry is unavailable — the metadata database could not be opened.';
const ADVISORY_DEBOUNCE_MS = 300;

export interface UseProfileModsResult {
  /** null = loading */
  mods: ProfileModRecord[] | null;
  error: string | null;
  unavailable: boolean;
  refresh: () => Promise<void>;
  addMod: (input: ProfileModInput) => Promise<ProfileModRecord>;
  updateMod: (modId: string, input: ProfileModInput) => Promise<ProfileModRecord>;
  removeMod: (modId: string) => Promise<void>;
  /** Optimistic enabled flip; reverts and sets `error` on rejection. */
  toggleMod: (mod: ProfileModRecord) => Promise<void>;
  detect: {
    scanning: boolean;
    report: DetectionScanReport | null;
    error: string | null;
    run: () => Promise<void>;
    clear: () => void;
  };
  /** null = not yet computed */
  advisories: LaunchValidationIssue[] | null;
  advisoriesError: string | null;
  refreshAdvisories: () => void;
}

export function useProfileMods(profileName: string | undefined): UseProfileModsResult {
  const trimmedName = profileName?.trim() ?? '';

  const [mods, setMods] = useState<ProfileModRecord[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [unavailable, setUnavailable] = useState(false);
  const [scanning, setScanning] = useState(false);
  const [report, setReport] = useState<DetectionScanReport | null>(null);
  const [detectError, setDetectError] = useState<string | null>(null);
  const [advisories, setAdvisories] = useState<LaunchValidationIssue[] | null>(null);
  const [advisoriesError, setAdvisoriesError] = useState<string | null>(null);

  const activeRef = useRef(true);
  const unavailableRef = useRef(false);
  /** Active profile at any given moment — advisory results for another profile are discarded. */
  const activeProfileRef = useRef(trimmedName);
  const advisoryDebounce = useDebounce(ADVISORY_DEBOUNCE_MS);

  useEffect(() => {
    activeRef.current = true;
    return () => {
      activeRef.current = false;
    };
  }, []);

  const markUnavailable = useCallback(() => {
    unavailableRef.current = true;
    if (activeRef.current) {
      setUnavailable(true);
    }
  }, []);

  const runAdvisories = useCallback(
    async (profileSnapshot: string) => {
      if (!profileSnapshot || unavailableRef.current) {
        return;
      }
      const stillCurrent = () => activeRef.current && activeProfileRef.current === profileSnapshot;
      try {
        const issues = await callCommand<LaunchValidationIssue[]>('analyze_mod_coexistence', {
          profileName: profileSnapshot,
        });
        if (stillCurrent()) {
          setAdvisories(issues);
          setAdvisoriesError(null);
        }
      } catch (e) {
        const message = formatInvokeError(e);
        if (message.startsWith(METADATA_UNAVAILABLE_PREFIX)) {
          markUnavailable();
          if (stillCurrent()) {
            setAdvisories(null);
          }
          return;
        }
        if (stillCurrent()) {
          setAdvisoriesError(message);
        }
      }
    },
    [markUnavailable]
  );

  const refreshAdvisories = useCallback(() => {
    const profileSnapshot = trimmedName;
    advisoryDebounce.schedule(() => void runAdvisories(profileSnapshot));
  }, [trimmedName, advisoryDebounce, runAdvisories]);

  const refresh = useCallback(async () => {
    if (!trimmedName) {
      setMods([]);
      setError(null);
      return;
    }
    try {
      const response = await callCommand<ProfileModsResponse>('list_profile_mods', {
        profileName: trimmedName,
      });
      if (!activeRef.current) {
        return;
      }
      if (!response.available) {
        markUnavailable();
        setMods([]);
        return;
      }
      setMods(response.mods);
      setError(null);
      if (response.mods.length > 0) {
        refreshAdvisories();
      } else {
        setAdvisories([]);
      }
    } catch (e) {
      if (!activeRef.current) {
        return;
      }
      const message = formatInvokeError(e);
      if (message.startsWith(METADATA_UNAVAILABLE_PREFIX)) {
        markUnavailable();
        setMods([]);
        return;
      }
      setError(message);
      setMods([]);
    }
  }, [trimmedName, markUnavailable, refreshAdvisories]);

  useEffect(() => {
    activeProfileRef.current = trimmedName;
    advisoryDebounce.cancel();
    unavailableRef.current = false;
    setUnavailable(false);
    setMods(null);
    setError(null);
    setAdvisories(null);
    setAdvisoriesError(null);
    setReport(null);
    setDetectError(null);
    if (!trimmedName) {
      setMods([]);
      return;
    }
    void refresh();
  }, [trimmedName, advisoryDebounce, refresh]);

  const guardAvailable = useCallback(() => {
    if (unavailableRef.current) {
      throw new Error(UNAVAILABLE_MESSAGE);
    }
    if (!trimmedName) {
      throw new Error('Select a saved profile to manage its mod registry.');
    }
  }, [trimmedName]);

  /** Applies a returned record (or a removal) to local state and schedules one debounced advisory refresh. */
  const applyMutation = useCallback(
    (mutate: (current: ProfileModRecord[]) => ProfileModRecord[]) => {
      if (!activeRef.current) {
        return;
      }
      setMods((current) => mutate(current ?? []));
      refreshAdvisories();
    },
    [refreshAdvisories]
  );

  const addMod = useCallback(
    async (input: ProfileModInput): Promise<ProfileModRecord> => {
      guardAvailable();
      try {
        const record = await callCommand<ProfileModRecord>('add_profile_mod', {
          profileName: trimmedName,
          input,
        });
        applyMutation((current) => [...current.filter((entry) => entry.mod_id !== record.mod_id), record]);
        return record;
      } catch (e) {
        const message = formatInvokeError(e);
        if (message.startsWith(METADATA_UNAVAILABLE_PREFIX)) {
          markUnavailable();
        }
        throw new Error(message);
      }
    },
    [guardAvailable, trimmedName, applyMutation, markUnavailable]
  );

  const updateMod = useCallback(
    async (modId: string, input: ProfileModInput): Promise<ProfileModRecord> => {
      guardAvailable();
      try {
        const record = await callCommand<ProfileModRecord>('update_profile_mod', {
          profileName: trimmedName,
          modId,
          input,
        });
        applyMutation((current) => current.map((entry) => (entry.mod_id === record.mod_id ? record : entry)));
        return record;
      } catch (e) {
        const message = formatInvokeError(e);
        if (message.startsWith(METADATA_UNAVAILABLE_PREFIX)) {
          markUnavailable();
        }
        throw new Error(message);
      }
    },
    [guardAvailable, trimmedName, applyMutation, markUnavailable]
  );

  const removeMod = useCallback(
    async (modId: string): Promise<void> => {
      guardAvailable();
      try {
        await callCommand<void>('remove_profile_mod', {
          profileName: trimmedName,
          modId,
        });
        applyMutation((current) => current.filter((entry) => entry.mod_id !== modId));
      } catch (e) {
        const message = formatInvokeError(e);
        if (message.startsWith(METADATA_UNAVAILABLE_PREFIX)) {
          markUnavailable();
        }
        throw new Error(message);
      }
    },
    [guardAvailable, trimmedName, applyMutation, markUnavailable]
  );

  const toggleMod = useCallback(
    async (mod: ProfileModRecord): Promise<void> => {
      guardAvailable();
      const nextEnabled = !mod.enabled;
      setMods((current) =>
        current
          ? current.map((entry) => (entry.mod_id === mod.mod_id ? { ...entry, enabled: nextEnabled } : entry))
          : current
      );
      try {
        await callCommand<ProfileModRecord>('update_profile_mod', {
          profileName: trimmedName,
          modId: mod.mod_id,
          input: {
            name: mod.name,
            category: mod.category,
            paths: mod.paths,
            enabled: nextEnabled,
            source_url: mod.source_url,
            provenance: mod.provenance,
          },
        });
        refreshAdvisories();
      } catch (e) {
        if (!activeRef.current) {
          return;
        }
        setMods((current) =>
          current
            ? current.map((entry) => (entry.mod_id === mod.mod_id ? { ...entry, enabled: mod.enabled } : entry))
            : current
        );
        const message = formatInvokeError(e);
        if (message.startsWith(METADATA_UNAVAILABLE_PREFIX)) {
          markUnavailable();
          return;
        }
        setError(message);
      }
    },
    [guardAvailable, trimmedName, refreshAdvisories, markUnavailable]
  );

  const runDetect = useCallback(async () => {
    guardAvailable();
    setScanning(true);
    setDetectError(null);
    try {
      const next = await callCommand<DetectionScanReport>('detect_profile_mods', {
        profileName: trimmedName,
      });
      if (activeRef.current) {
        setReport(next);
      }
    } catch (e) {
      const message = formatInvokeError(e);
      if (activeRef.current) {
        setDetectError(
          message.startsWith(NO_GAME_PATH_PREFIX) ? message.slice(NO_GAME_PATH_PREFIX.length).trim() : message
        );
      }
    } finally {
      if (activeRef.current) {
        setScanning(false);
      }
    }
  }, [guardAvailable, trimmedName]);

  const clearDetect = useCallback(() => {
    setReport(null);
    setDetectError(null);
  }, []);

  return {
    mods,
    error,
    unavailable,
    refresh,
    addMod,
    updateMod,
    removeMod,
    toggleMod,
    detect: {
      scanning,
      report,
      error: detectError,
      run: runDetect,
      clear: clearDetect,
    },
    advisories,
    advisoriesError,
    refreshAdvisories,
  };
}
