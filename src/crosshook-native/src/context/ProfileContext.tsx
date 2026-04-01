/**
 * Active profile state and selection.
 *
 * ProfileContext owns profile CRUD, selection, and derived values (launch method,
 * Steam paths). App settings and recent files are handled by PreferencesContext.
 *
 * Listens for `auto-load-profile` events emitted by the Tauri backend at startup.
 */
import { createContext, useContext, useEffect, useMemo, type ReactNode } from 'react';
import { listen } from '@tauri-apps/api/event';

import { useProfile, type UseProfileResult } from '../hooks/useProfile';
import { deriveSteamClientInstallPath, deriveTargetHomePath } from '../utils/steam';
import { resolveLaunchMethod, type ResolvedLaunchMethod } from '../utils/launch';

export interface ProfileContextValue extends UseProfileResult {
  launchMethod: ResolvedLaunchMethod;
  steamClientInstallPath: string;
  targetHomePath: string;
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

  useEffect(() => {
    let active = true;
    const unlistenPromise = listen<string>('auto-load-profile', (event) => {
      if (!active) {
        return;
      }

      void profileState.selectProfile(event.payload);
    });

    return () => {
      active = false;
      void unlistenPromise.then((unlisten) => unlisten());
    };
  }, [profileState.selectProfile]);

  const value = useMemo<ProfileContextValue>(
    () => ({
      ...profileState,
      launchMethod,
      steamClientInstallPath,
      targetHomePath,
    }),
    [launchMethod, profileState, steamClientInstallPath, targetHomePath]
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
