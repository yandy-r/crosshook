/**
 * Active profile state and selection.
 *
 * ProfileContext owns profile CRUD, selection, and derived values (launch method,
 * Steam paths). App settings and recent files are handled by PreferencesContext.
 *
 * On mount it pulls the startup report (`get_startup_report`); the report's
 * `auto_load_profile` replaces the removed startup event, so a slow mount
 * cannot miss the auto-load.
 */
import { createContext, type ReactNode, useContext, useEffect, useMemo, useRef, useState } from 'react';
import { claimStartupAction, fetchStartupReport } from '@/lib/startupReport';

import { type UseProfileResult, useProfile } from '../hooks/useProfile';
import { type ResolvedLaunchMethod, resolveLaunchMethod } from '../utils/launch';
import { deriveSteamClientInstallPath, deriveTargetHomePath } from '../utils/steam';

export interface ProfileContextValue extends UseProfileResult {
  launchMethod: ResolvedLaunchMethod;
  steamClientInstallPath: string;
  targetHomePath: string;
  activeCollectionId: string | null;
  setActiveCollectionId: (id: string | null) => void;
}

interface ProfileProviderProps {
  children: ReactNode;
}

const ProfileContext = createContext<ProfileContextValue | null>(null);

export function ProfileProvider({ children }: ProfileProviderProps) {
  const profileState = useProfile({ autoSelectFirstProfile: false });
  const launchMethod = resolveLaunchMethod(profileState.profile);
  const steamClientInstallPath = deriveSteamClientInstallPath(profileState.profile.steam.compatdata_path);
  const targetHomePath = deriveTargetHomePath(steamClientInstallPath);
  const [activeCollectionId, setActiveCollectionId] = useState<string | null>(null);

  const selectProfileRef = useRef(profileState.selectProfile);
  selectProfileRef.current = profileState.selectProfile;
  const selectedProfileRef = useRef(profileState.selectedProfile);
  selectedProfileRef.current = profileState.selectedProfile;
  // Set synchronously by `useProfile`'s `loadProfile` before its first await:
  // true once any named load (user pick, save-and-reload, rename) has started.
  // The auto-load guard below checks it so the startup report cannot fire a
  // second `loadProfile` racing an in-flight user choice. The ref object is
  // stable across renders, so reading it never re-triggers the effect.
  const { selectionRequestedRef } = profileState;

  useEffect(() => {
    let active = true;
    void fetchStartupReport()
      .then((report) => {
        if (!active) {
          return;
        }
        const name = report.auto_load_profile;
        if (name === null || name.trim() === '') {
          return;
        }
        if (!claimStartupAction('auto_load_profile')) {
          return;
        }
        // A user selection is already in flight (or done): the auto-load must
        // not race it. `selectionRequestedRef` is set synchronously by
        // `loadProfile`, so this holds even while `profile_load` is pending.
        if (selectionRequestedRef.current || selectedProfileRef.current.trim() !== '') {
          return;
        }
        void selectProfileRef.current(name);
      })
      .catch(() => {
        // Startup report unavailable — the profile list still loads normally.
      });

    return () => {
      active = false;
    };
  }, [selectedProfileRef, selectionRequestedRef, selectProfileRef]);

  const value = useMemo<ProfileContextValue>(
    () => ({
      ...profileState,
      launchMethod,
      steamClientInstallPath,
      targetHomePath,
      activeCollectionId,
      setActiveCollectionId,
    }),
    [activeCollectionId, launchMethod, profileState, steamClientInstallPath, targetHomePath]
  );

  return <ProfileContext.Provider value={value}>{children}</ProfileContext.Provider>;
}

export function useProfileContext(): ProfileContextValue {
  const context = useContext(ProfileContext);

  if (context === null) {
    throw new Error('useProfileContext must be used within a ProfileProvider.');
  }

  return context;
}
