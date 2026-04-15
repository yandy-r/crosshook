/**
 * Shared launch session state.
 *
 * Wraps useLaunchState in a context so session state and Tauri event
 * listeners persist across route changes instead of resetting on every
 * Launch page mount.
 */
import { createContext, type ReactNode, useContext } from 'react';

import { useLaunchState } from '../hooks/useLaunchState';
import { buildProfileLaunchRequest } from '../utils/launch';
import { usePreferencesContext } from './PreferencesContext';
import { useProfileContext } from './ProfileContext';

type LaunchStateContextValue = ReturnType<typeof useLaunchState>;

const LaunchStateContext = createContext<LaunchStateContextValue | null>(null);

export function LaunchStateProvider({ children }: { children: ReactNode }) {
  const profileState = useProfileContext();
  const { defaultSteamClientInstallPath, settings } = usePreferencesContext();
  const selectedName = profileState.selectedProfile || '';
  const profileId = profileState.profileName.trim() || selectedName || 'new-profile';
  const effectiveSteamClientInstallPath = defaultSteamClientInstallPath || profileState.steamClientInstallPath;
  const request = buildProfileLaunchRequest(
    profileState.profile,
    profileState.launchMethod,
    effectiveSteamClientInstallPath,
    selectedName,
    settings.umu_preference
  );

  const launchState = useLaunchState({
    profileId,
    profileName: selectedName,
    method: profileState.launchMethod,
    request,
  });

  return <LaunchStateContext.Provider value={launchState}>{children}</LaunchStateContext.Provider>;
}

export function useLaunchStateContext(): LaunchStateContextValue {
  const context = useContext(LaunchStateContext);
  if (context === null) {
    throw new Error('useLaunchStateContext must be used within a LaunchStateProvider.');
  }
  return context;
}
