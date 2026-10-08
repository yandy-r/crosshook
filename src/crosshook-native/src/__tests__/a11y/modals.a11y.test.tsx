/**
 * Accessibility tests for modal / dialog surfaces.
 *
 * Modals render via createPortal into a dynamically-created host div, so axe
 * is run against `document.body` (not the RTL render container) to capture
 * the portal's content. `screen.getByRole('dialog')` traverses the full
 * document and resolves portals correctly.
 */
import { screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { LauncherPreviewModal } from '@/components/LauncherPreviewModal';
import { MetadataStatusBanner } from '@/components/layout/MetadataStatusBanner';
import { OnboardingWizard } from '@/components/OnboardingWizard';
import { ProfileReviewModal } from '@/components/ProfileReviewModal';
import { PreferencesProvider } from '@/context/PreferencesContext';
import { ProfileProvider } from '@/context/ProfileContext';
import { renderWithMocks } from '@/test/render';
import { axe } from '@/test/setup';

vi.mock('@/lib/ipc', async () => {
  const { mockCallCommand } = await import('@/test/render');
  return { callCommand: mockCallCommand };
});

const noop = () => {};

function OnboardingProviders({ children }: { children: ReactNode }) {
  return (
    <ProfileProvider>
      <PreferencesProvider>{children}</PreferencesProvider>
    </ProfileProvider>
  );
}

// ---------------------------------------------------------------------------
// ProfileReviewModal
// ---------------------------------------------------------------------------

describe('ProfileReviewModal accessibility', () => {
  it('has no axe violations when open', async () => {
    renderWithMocks(
      <ProfileReviewModal
        open={true}
        title="Review: Test Game"
        statusLabel="Ready"
        profileName="Test Game Alpha"
        executablePath="/home/user/games/testgame.exe"
        prefixPath="/home/user/.wine/prefixes/testgame"
        helperLogPath="/home/user/.local/share/crosshook/logs/testgame.log"
        onClose={noop}
      >
        <p>Profile body content</p>
      </ProfileReviewModal>
    );

    // Portal is created in a useEffect — wait for the dialog to appear.
    const dialog = await screen.findByRole('dialog');

    const results = await axe(document.body);
    expect(results).toHaveNoViolations();

    // aria-modal must be set on the dialog element.
    expect(dialog).toHaveAttribute('aria-modal', 'true');

    // aria-labelledby must point to an element that exists in the DOM.
    const labelId = dialog.getAttribute('aria-labelledby');
    expect(labelId).toBeTruthy();
    if (labelId) {
      expect(document.getElementById(labelId)).not.toBeNull();
    }
  });
});

// ---------------------------------------------------------------------------
// MetadataStatusBanner + modal coexistence
// ---------------------------------------------------------------------------

describe('MetadataStatusBanner with modal accessibility', () => {
  it('has no axe violations when the degraded banner is visible alongside an open modal', async () => {
    sessionStorage.clear();
    renderWithMocks(
      <>
        <MetadataStatusBanner />
        <ProfileReviewModal
          open={true}
          title="Review: Test Game"
          statusLabel="Ready"
          profileName="Test Game Alpha"
          executablePath="/home/user/games/testgame.exe"
          prefixPath="/home/user/.wine/prefixes/testgame"
          helperLogPath="/home/user/.local/share/crosshook/logs/testgame.log"
          onClose={noop}
        >
          <p>Profile body content</p>
        </ProfileReviewModal>
      </>,
      {
        handlerOverrides: {
          metadata_store_status: async () => ({ state: 'newer_schema', found: 99, supported: 27 }),
        },
      }
    );

    const dialog = await screen.findByRole('dialog');

    // The modal marks its siblings aria-hidden + inert for focus containment, so the
    // banner is in the DOM but not exposed to the a11y tree while the modal is open.
    await waitFor(() => expect(document.querySelector('.crosshook-metadata-status-banner')).not.toBeNull());

    const results = await axe(document.body);
    expect(results).toHaveNoViolations();

    // aria-modal must be set on the dialog element.
    expect(dialog).toHaveAttribute('aria-modal', 'true');
  });
});

// ---------------------------------------------------------------------------
// LauncherPreviewModal
// ---------------------------------------------------------------------------

describe('LauncherPreviewModal accessibility', () => {
  it('has no axe violations when open', async () => {
    renderWithMocks(
      <LauncherPreviewModal
        scriptContent={'#!/usr/bin/env bash\nexec proton run game.exe'}
        desktopContent={'[Desktop Entry]\nName=Test Game\nExec=/path/to/launcher'}
        displayName="Test Game Alpha"
        onClose={noop}
      />
    );

    // Portal is created in a useEffect — wait for the dialog to appear.
    const dialog = await screen.findByRole('dialog');

    const results = await axe(document.body);
    expect(results).toHaveNoViolations();

    // aria-modal must be set on the dialog element.
    expect(dialog).toHaveAttribute('aria-modal', 'true');

    // aria-labelledby must point to an element that exists in the DOM.
    const labelId = dialog.getAttribute('aria-labelledby');
    expect(labelId).toBeTruthy();
    if (labelId) {
      expect(document.getElementById(labelId)).not.toBeNull();
    }
  });
});

// ---------------------------------------------------------------------------
// OnboardingWizard
// ---------------------------------------------------------------------------

describe('OnboardingWizard accessibility', () => {
  it('has no axe violations when open on the identity_game stage', async () => {
    renderWithMocks(
      <OnboardingProviders>
        <OnboardingWizard open={true} mode="create" onComplete={noop} onDismiss={noop} />
      </OnboardingProviders>
    );

    // Portal is created in a useEffect — wait for the wizard dialog to appear.
    const dialog = await screen.findByRole('dialog');

    await waitFor(async () => {
      const results = await axe(document.body);
      expect(results).toHaveNoViolations();
    });

    // aria-modal must be set on the dialog element.
    expect(dialog).toHaveAttribute('aria-modal', 'true');

    // aria-labelledby must point to an element that exists in the DOM.
    const labelId = dialog.getAttribute('aria-labelledby');
    expect(labelId).toBeTruthy();
    if (labelId) {
      expect(document.getElementById(labelId)).not.toBeNull();
    }
  });
});
