import { screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AppShell } from '@/components/layout/AppShell';
import { CollectionsProvider } from '@/context/CollectionsContext';
import { HostReadinessProvider } from '@/context/HostReadinessContext';
import { ProfileProvider } from '@/context/ProfileContext';
import { ProfileHealthProvider } from '@/context/ProfileHealthContext';
import { renderWithMocks } from '@/test/render';

function AppShellInAppProviders() {
  return (
    <ProfileProvider>
      <ProfileHealthProvider>
        <HostReadinessProvider>
          <CollectionsProvider>
            <AppShell controllerMode={false} />
          </CollectionsProvider>
        </HostReadinessProvider>
      </ProfileHealthProvider>
    </ProfileProvider>
  );
}

describe('AppShell (integration)', () => {
  beforeEach(() => {
    const memory = new Map<string, string>();
    vi.stubGlobal('localStorage', {
      get length() {
        return memory.size;
      },
      clear: (): void => {
        memory.clear();
      },
      getItem: (k: string): string | null => memory.get(k) ?? null,
      key: (i: number): string | null => Array.from(memory.keys())[i] ?? null,
      removeItem: (k: string): void => {
        memory.delete(k);
      },
      setItem: (k: string, v: string): void => {
        memory.set(k, v);
      },
    } as Storage);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('renders the resizable layout shell and primary navigation (parity with App wrapper)', () => {
    renderWithMocks(<AppShellInAppProviders />);

    const layout = document.querySelector('.crosshook-app-layout');
    expect(layout).not.toBeNull();
    expect(layout).toBeInTheDocument();
    expect(screen.getByLabelText('CrossHook navigation')).toBeInTheDocument();
  });
});
