import { createContext, useContext, useEffect, useMemo, type ReactNode } from 'react';
import { listen } from '@tauri-apps/api/event';

import { useProfile, type UseProfileResult } from '../hooks/useProfile';
import type { GameProfile, LaunchMethod } from '../types';
import { deriveSteamClientInstallPath, deriveTargetHomePath } from '../utils/steam';

type ResolvedLaunchMethod = Exclude<LaunchMethod, ''>;

export interface ProfileContextValue extends UseProfileResult {
  launchMethod: ResolvedLaunchMethod;
  steamClientInstallPath: string;
  targetHomePath: string;
}

interface ProfileProviderProps {
  children: ReactNode;
}

const ProfileContext = createContext<ProfileContextValue | undefined>(undefined);

function resolveLaunchMethod(profile: GameProfile): ResolvedLaunchMethod {
  const method = profile.launch.method.trim();

  if (method === 'steam_applaunch' || method === 'proton_run' || method === 'native') {
    return method;
  }

  if (profile.steam.enabled) {
    return 'steam_applaunch';
  }

  if (profile.game.executable_path.trim().toLowerCase().endsWith('.exe')) {
    return 'proton_run';
  }

  return 'native';
}

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

  if (context === undefined) {
    throw new Error('useProfileContext must be used within a ProfileProvider.');
  }

  return context;
}
