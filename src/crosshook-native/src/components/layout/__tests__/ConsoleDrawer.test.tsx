import { act, screen, waitFor } from '@testing-library/react';
import type { PanelImperativeHandle } from 'react-resizable-panels';
import { describe, expect, it, vi } from 'vitest';
import { HostReadinessProvider } from '@/context/HostReadinessContext';
import { emitMockEvent } from '@/lib/events';
import { renderWithMocks } from '@/test/render';
import { ConsoleDrawer } from '../ConsoleDrawer';

function createPanelRef() {
  return {
    current: {
      collapse: vi.fn(),
      expand: vi.fn(),
    } as unknown as PanelImperativeHandle,
  };
}

describe('ConsoleDrawer', () => {
  it('renders the compact status bar on narrow layouts', async () => {
    const panelRef = createPanelRef();

    renderWithMocks(
      <HostReadinessProvider>
        <ConsoleDrawer panelRef={panelRef} mode="status" />
      </HostReadinessProvider>
    );

    expect(screen.getByTestId('console-status-bar')).toBeInTheDocument();
    expect(screen.getByText('Runtime console')).toBeInTheDocument();
    expect(screen.getByText('⌘K commands')).toBeInTheDocument();

    await waitFor(() => {
      expect(screen.getByText(/required/i)).toBeInTheDocument();
      expect(screen.getByText(/capabilities/i)).toBeInTheDocument();
    });
  });

  it('keeps the wide drawer collapsed when log events arrive', async () => {
    const panelRef = createPanelRef();

    renderWithMocks(
      <HostReadinessProvider>
        <ConsoleDrawer panelRef={panelRef} mode="drawer" />
      </HostReadinessProvider>
    );

    const toggle = screen.getByRole('button', { name: /runtime console/i });
    expect(toggle).toHaveAttribute('aria-expanded', 'false');

    await act(async () => {
      await Promise.resolve();
    });

    act(() => {
      emitMockEvent('launch-log', 'first line\nsecond line');
    });

    expect(toggle).toHaveAttribute('aria-expanded', 'false');
    expect(screen.getByText('2 lines')).toBeInTheDocument();
    expect(panelRef.current.expand).not.toHaveBeenCalled();
  });
});
