import { screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AppShell } from '@/components/layout/AppShell';
import { CollectionsProvider } from '@/context/CollectionsContext';
import { HostReadinessProvider } from '@/context/HostReadinessContext';
import { InspectorSelectionProvider } from '@/context/InspectorSelectionContext';
import { ProfileProvider } from '@/context/ProfileContext';
import { ProfileHealthProvider } from '@/context/ProfileHealthContext';
import { renderWithMocks } from '@/test/render';

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

function setInnerWidth(w: number): void {
  Object.defineProperty(window, 'innerWidth', { value: w, configurable: true, writable: true });
}

function setInnerHeight(h: number): void {
  Object.defineProperty(window, 'innerHeight', { value: h, configurable: true, writable: true });
}

function shellRect(width: number, height: number): DOMRect {
  return {
    width,
    height,
    x: 0,
    y: 0,
    top: 0,
    left: 0,
    right: width,
    bottom: height,
    toJSON: () => ({}),
  } as DOMRect;
}

function mockAppShellRect(width: number, height: number) {
  const original = HTMLDivElement.prototype.getBoundingClientRect;
  return vi.spyOn(HTMLDivElement.prototype, 'getBoundingClientRect').mockImplementation(function (
    this: HTMLDivElement
  ) {
    if (this.classList.contains('crosshook-app-layout')) {
      return shellRect(width, height);
    }
    return original.call(this);
  });
}

describe('AppShell (integration)', () => {
  let prevWidth: number;
  let prevHeight: number;

  beforeEach(() => {
    prevWidth = window.innerWidth;
    prevHeight = window.innerHeight;
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
    setInnerWidth(prevWidth);
    setInnerHeight(prevHeight);
    vi.unstubAllGlobals();
  });

  it('renders the resizable layout shell and primary navigation (parity with App wrapper)', () => {
    renderWithMocks(<AppShellInAppProviders />);

    const layout = document.querySelector('.crosshook-app-layout');
    expect(layout).not.toBeNull();
    expect(layout).toBeInTheDocument();
    expect(screen.getByLabelText('CrossHook navigation')).toBeInTheDocument();
  });

  it('uses the rail sidebar variant for deck-sized shells', async () => {
    setInnerWidth(1280);
    setInnerHeight(800);
    const rectSpy = mockAppShellRect(1280, 800);

    renderWithMocks(<AppShellInAppProviders />);

    await waitFor(() => {
      const nav = screen.getByLabelText('CrossHook navigation');
      expect(nav).toHaveAttribute('data-sidebar-variant', 'rail');
      expect(nav).toHaveAttribute('data-sidebar-width', '56');
      expect(nav).toHaveAttribute('data-collapsed', 'true');
    });
    rectSpy.mockRestore();
  });

  it('uses the full sidebar variant for desktop-sized shells and keeps Collections in declared order', async () => {
    setInnerWidth(1920);
    setInnerHeight(1080);
    const rectSpy = mockAppShellRect(1920, 1080);

    renderWithMocks(<AppShellInAppProviders />);

    await waitFor(() => {
      const nav = screen.getByLabelText('CrossHook navigation');
      expect(nav).toHaveAttribute('data-sidebar-variant', 'full');
      expect(nav).toHaveAttribute('data-sidebar-width', '240');
      expect(nav).toHaveAttribute('data-collapsed', 'false');
    });

    const sectionLabels = Array.from(document.querySelectorAll('.crosshook-sidebar__section-label')).map((node) =>
      node.textContent?.trim()
    );
    expect(sectionLabels).toEqual(['Game', 'Collections', 'Setup', 'Dashboards', 'Community']);
    rectSpy.mockRestore();
  });

  it('exposes sidebar and inspector test ids at desk width', async () => {
    setInnerWidth(1920);
    setInnerHeight(1080);
    const rectSpy = mockAppShellRect(1920, 1080);
    try {
      renderWithMocks(<AppShellInAppProviders />);

      await waitFor(() => {
        expect(screen.getByTestId('sidebar')).toBeInTheDocument();
        expect(screen.getByTestId('inspector')).toBeInTheDocument();
      });
    } finally {
      rectSpy.mockRestore();
    }
  });

  it('hides the inspector rail at deck width', async () => {
    setInnerWidth(1024);
    setInnerHeight(800);
    const rectSpy = mockAppShellRect(1024, 800);
    try {
      renderWithMocks(<AppShellInAppProviders />);

      await waitFor(() => {
        expect(screen.getByTestId('sidebar')).toBeInTheDocument();
        expect(screen.queryByTestId('inspector')).not.toBeInTheDocument();
      });
    } finally {
      rectSpy.mockRestore();
    }
  });
});
