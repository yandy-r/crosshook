/**
 * Shared profile health state.
 *
 * Wraps useProfileHealth in a context so health data persists across
 * route changes instead of re-fetching on every page mount.
 */
import { createContext, useContext, type ReactNode } from 'react';

import { useProfileHealth } from '../hooks/useProfileHealth';

type ProfileHealthContextValue = ReturnType<typeof useProfileHealth>;

const ProfileHealthContext = createContext<ProfileHealthContextValue | null>(null);

export function ProfileHealthProvider({ children }: { children: ReactNode }) {
  const health = useProfileHealth();
  return <ProfileHealthContext.Provider value={health}>{children}</ProfileHealthContext.Provider>;
}

export function useProfileHealthContext(): ProfileHealthContextValue {
  const context = useContext(ProfileHealthContext);
  if (context === null) {
    throw new Error('useProfileHealthContext must be used within a ProfileHealthProvider.');
  }
  return context;
}
