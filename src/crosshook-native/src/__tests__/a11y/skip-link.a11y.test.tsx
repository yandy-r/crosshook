import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AppShell } from '@/components/layout/AppShell';
import { MAIN_CONTENT_ID } from '@/components/layout/SkipToContentLink';
import { CollectionsProvider } from '@/context/CollectionsContext';
import { HostReadinessProvider } from '@/context/HostReadinessContext';
import { InspectorSelectionProvider } from '@/context/InspectorSelectionContext';
import { ProfileProvider } from '@/context/ProfileContext';
import { ProfileHealthProvider } from '@/context/ProfileHealthContext';
import { renderWithMocks } from '@/test/render';
import { axe } from '@/test/setup';

vi.mock('@/lib/ipc', async () => {
  const { mockCallCommand } = await import('@/test/render');
  return { callCommand: mockCallCommand };
});

function AppShellInAppProviders() {
  return (
    <ProfileProvider>
      <ProfileHealthProvider>
        <HostReadinessProvider>
          <CollectionsProvider>
            <InspectorSelectionProvider>
              <AppShell controllerMode={false} />
            </InspectorSelectionProvider>
          </CollectionsProvider>
        </HostReadinessProvider>
      </ProfileHealthProvider>
    </ProfileProvider>
  );
}

async function renderAppShell() {
  renderWithMocks(<AppShellInAppProviders />);
  await waitFor(() => {
    expect(screen.getByTestId('sidebar')).toBeInTheDocument();
  });
}

describe('SkipToContentLink (AppShell integration)', () => {
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

  it('skip link is the first tab stop', async () => {
    const user = userEvent.setup();
    await renderAppShell();

    await user.tab();

    expect(document.activeElement).toBeInstanceOf(HTMLElement);
    expect((document.activeElement as HTMLElement).classList.contains('crosshook-skip-link')).toBe(true);
  });

  it('skip link has the hidden-until-focus class contract', async () => {
    await renderAppShell();

    const link = screen.getByRole('link', { name: 'Skip to main content' });
    expect(link.classList.contains('crosshook-skip-link')).toBe(true);
    expect(link.getAttribute('href')).toBe(`#${MAIN_CONTENT_ID}`);
  });

  it('activating the skip link moves focus to the main content region', async () => {
    const user = userEvent.setup();
    await renderAppShell();

    await user.click(screen.getByRole('link', { name: 'Skip to main content' }));

    expect(document.activeElement?.id).toBe(MAIN_CONTENT_ID);
    expect(document.activeElement?.getAttribute('tabindex')).toBe('-1');
  });

  it('AppShell chrome passes axe', async () => {
    renderWithMocks(<AppShellInAppProviders />);
    await waitFor(() => {
      expect(screen.getByTestId('sidebar')).toBeInTheDocument();
    });

    const layout = document.querySelector('.crosshook-app-layout');
    expect(layout).not.toBeNull();
    const results = await axe(layout as HTMLElement);
    expect(results).toHaveNoViolations();
  });
});
