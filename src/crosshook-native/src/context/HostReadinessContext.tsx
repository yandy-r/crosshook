/**
 * Shared host readiness state.
 *
 * Wraps useHostReadiness in a context so the single set of IPC calls
 * (get_cached_host_readiness_snapshot + get_capabilities) is issued once
 * regardless of how many components consume the data. Without this, each
 * useCapabilityGate caller creates an independent hook instance that fires
 * its own IPC burst on mount (see F002).
 */
import { createContext, type ReactNode, useContext } from 'react';

import { useHostReadiness } from '../hooks/useHostReadiness';

type HostReadinessContextValue = ReturnType<typeof useHostReadiness>;

const HostReadinessContext = createContext<HostReadinessContextValue | null>(null);

export function HostReadinessProvider({ children }: { children: ReactNode }) {
  const readiness = useHostReadiness();
  return <HostReadinessContext.Provider value={readiness}>{children}</HostReadinessContext.Provider>;
}

export function useHostReadinessContext(): HostReadinessContextValue {
  const context = useContext(HostReadinessContext);
  if (context === null) {
    throw new Error('useHostReadinessContext must be used within a HostReadinessProvider.');
  }
  return context;
}
