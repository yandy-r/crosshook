/**
 * Shared launch session state.
 *
 * Wraps useLaunchState in a context so session state and Tauri event
 * listeners persist across route changes instead of resetting on every
 * Launch page mount.
 */
import { createContext, useContext, type ReactNode } from 'react';

import { useLaunchState } from '../hooks/useLaunchState';
import { useProfileContext } from './ProfileContext';
import { buildProfileLaunchRequest } from '../utils/launch';

type LaunchStateContextValue = ReturnType<typeof useLaunchState>;

const LaunchStateContext = createContext<LaunchStateContextValue | null>(null);

export function LaunchStateProvider({ children }: { children: ReactNode }) {
  const profileState = useProfileContext();
  const selectedName = profileState.selectedProfile || '';
  const profileId = profileState.profileName.trim() || selectedName || 'new-profile';
  const request = buildProfileLaunchRequest(
    profileState.profile,
    profileState.launchMethod,
    profileState.steamClientInstallPath,
    selectedName,
  );

  const launchState = useLaunchState({
    profileId,
    profileName: selectedName,
    method: profileState.launchMethod,
    request,
  });

  return (
    <LaunchStateContext.Provider value={launchState}>
      {children}
    </LaunchStateContext.Provider>
  );
}

export function useLaunchStateContext(): LaunchStateContextValue {
  const context = useContext(LaunchStateContext);
  if (context === null) {
    throw new Error('useLaunchStateContext must be used within a LaunchStateProvider.');
  }
  return context;
}
