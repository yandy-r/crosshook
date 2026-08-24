import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { makeProfileDraft } from '@/test/fixtures';
import { LaunchDepGateModal } from '../launch/LaunchDepGateModal';
import type { DepGateState } from '../launch/useLaunchDepGate';

vi.mock('@/context/LaunchStateContext', () => ({
  useLaunchStateContext: () => ({
    launchGame: vi.fn(),
    launchTrainer: vi.fn(),
  }),
}));

function makeDepGate(overrides: Partial<DepGateState> = {}): DepGateState {
  return {
    depGatePackages: null,
    depGateRepair: null,
    depGatePendingAction: null,
    depGateInstalling: false,
    depGateVerifying: false,
    depGateRepairing: false,
    isGamescopeRunning: false,
    setDepGatePackages: vi.fn(),
    setDepGateRepair: vi.fn(),
    setDepGatePendingAction: vi.fn(),
    setDepGateInstalling: vi.fn(),
    setDepGateVerifying: vi.fn(),
    setDepGateRepairing: vi.fn(),
    handleBeforeLaunch: vi.fn().mockResolvedValue(false),
    repairPrefixVersion: vi.fn().mockResolvedValue(undefined),
    installPrefixDependency: vi.fn().mockResolvedValue(undefined),
    ...overrides,
  };
}

describe('LaunchDepGateModal', () => {
  it('requires repair without offering a skip path', async () => {
    const user = userEvent.setup();
    const repairPrefixVersion = vi.fn().mockResolvedValue(undefined);
    const depGate = makeDepGate({
      depGateRepair: { required: true, state: 'failed', last_error: 'restore timed out' },
      depGatePendingAction: 'game',
      repairPrefixVersion,
    });

    render(
      <LaunchDepGateModal
        depGate={depGate}
        profile={makeProfileDraft({ runtime: { prefix_path: '/mock/pfx' } })}
        selectedName="Synthetic Quest"
      />
    );

    expect(screen.getByRole('heading', { name: 'Prefix Repair Required' })).toBeInTheDocument();
    expect(screen.getByText(/restore timed out/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Skip and Launch' })).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Cancel' })).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Repair + Launch' }));
    expect(repairPrefixVersion).toHaveBeenCalledTimes(1);
  });

  it('announces post-install repair verification as a busy state', () => {
    const depGate = makeDepGate({
      depGatePackages: ['dotnet48'],
      depGateInstalling: true,
      depGateVerifying: true,
      depGatePendingAction: 'game',
    });

    render(
      <LaunchDepGateModal
        depGate={depGate}
        profile={makeProfileDraft({ runtime: { prefix_path: '/mock/pfx' } })}
        selectedName="Synthetic Quest"
      />
    );

    expect(screen.getByRole('dialog')).toHaveAttribute('aria-busy', 'true');
    expect(screen.getByRole('status')).toHaveTextContent('Verifying prefix repair');
  });
});
