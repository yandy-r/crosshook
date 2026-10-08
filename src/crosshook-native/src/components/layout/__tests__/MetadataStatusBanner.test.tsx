import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { MetadataStatusBanner } from '@/components/layout/MetadataStatusBanner';
import { METADATA_STATUS_DISMISS_KEY_PREFIX } from '@/hooks/useMetadataStoreStatus';
import { renderWithMocks } from '@/test/render';

vi.mock('@/lib/ipc', async () => {
  const { mockCallCommand } = await import('@/test/render');
  return { callCommand: mockCallCommand };
});

const newerStatus = () => Promise.resolve({ state: 'newer_schema', found: 99, supported: 27 });
const disabledStatus = () => Promise.resolve({ state: 'disabled', reason: 'disk is gone' });

beforeEach(() => {
  sessionStorage.clear();
});

describe('MetadataStatusBanner', () => {
  it('renders nothing when the store is ok', async () => {
    const { container } = renderWithMocks(<MetadataStatusBanner />, {
      handlerOverrides: { metadata_store_status: async () => ({ state: 'ok' }) },
    });
    await waitFor(() => expect(screen.queryByRole('status')).toBeNull());
    expect(container).toBeEmptyDOMElement();
  });

  it('renders the newer-schema warning with versions', async () => {
    renderWithMocks(<MetadataStatusBanner />, {
      handlerOverrides: { metadata_store_status: newerStatus },
    });
    const banner = await screen.findByRole('status');
    expect(banner).toHaveTextContent(
      'This data was written by a newer CrossHook (schema v99). History, health and catalogs are read-only until you update CrossHook.'
    );
  });

  it('renders the disabled warning with the reason', async () => {
    renderWithMocks(<MetadataStatusBanner />, {
      handlerOverrides: { metadata_store_status: disabledStatus },
    });
    const banner = await screen.findByRole('status');
    expect(banner).toHaveTextContent('Metadata storage is unavailable');
    expect(banner).toHaveTextContent('disk is gone');
  });

  it('renders nothing when the command fails', async () => {
    const { container } = renderWithMocks(<MetadataStatusBanner />, {
      handlerOverrides: {
        metadata_store_status: async () => {
          throw new Error('[test-mock] forced failure');
        },
      },
    });
    await waitFor(() => expect(screen.queryByRole('status')).toBeNull());
    expect(container).toBeEmptyDOMElement();
  });

  it('dismiss hides the banner and persists for the session', async () => {
    const user = userEvent.setup();
    renderWithMocks(<MetadataStatusBanner />, {
      handlerOverrides: { metadata_store_status: newerStatus },
    });
    const banner = await screen.findByRole('status');
    expect(sessionStorage.getItem(`${METADATA_STATUS_DISMISS_KEY_PREFIX}newer_schema:99`)).toBeNull();

    await user.click(screen.getByRole('button', { name: 'Dismiss metadata warning' }));
    expect(banner).not.toBeInTheDocument();
    expect(sessionStorage.getItem(`${METADATA_STATUS_DISMISS_KEY_PREFIX}newer_schema:99`)).toBe('1');
  });

  it('stays hidden on remount after session dismissal but shows a different state', async () => {
    sessionStorage.setItem(`${METADATA_STATUS_DISMISS_KEY_PREFIX}newer_schema:99`, '1');
    const first = renderWithMocks(<MetadataStatusBanner />, {
      handlerOverrides: { metadata_store_status: newerStatus },
    });
    await waitFor(() => expect(screen.queryByRole('status')).toBeNull());
    expect(first.container).toBeEmptyDOMElement();
    first.unmount();

    renderWithMocks(<MetadataStatusBanner />, {
      handlerOverrides: { metadata_store_status: disabledStatus },
    });
    await screen.findByRole('status');
  });

  it('re-surfaces when the found schema version changes after dismissal', async () => {
    sessionStorage.setItem(`${METADATA_STATUS_DISMISS_KEY_PREFIX}newer_schema:99`, '1');
    renderWithMocks(<MetadataStatusBanner />, {
      handlerOverrides: {
        metadata_store_status: async () => ({ state: 'newer_schema', found: 100, supported: 27 }),
      },
    });
    const banner = await screen.findByRole('status');
    expect(banner).toHaveTextContent('schema v100');
  });

  it.each([
    ['missing versions', { state: 'newer_schema', found: '99', supported: 27 }],
    ['non-integer versions', { state: 'newer_schema', found: 99.5, supported: 27 }],
    ['negative version', { state: 'newer_schema', found: -1, supported: 27 }],
    ['non-finite version', { state: 'newer_schema', found: Number.POSITIVE_INFINITY, supported: 27 }],
    ['invalid supported', { state: 'newer_schema', found: 99, supported: 27.5 }],
    ['empty reason', { state: 'disabled', reason: '' }],
    ['non-string reason', { state: 'disabled', reason: 42 }],
    ['unknown state', { state: 'mystery' }],
    ['non-object payload', 'ok'],
  ])('renders nothing and logs when the payload is invalid (%s)', async (_label, payload) => {
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const { container } = renderWithMocks(<MetadataStatusBanner />, {
      handlerOverrides: { metadata_store_status: async () => payload },
    });
    await waitFor(() => expect(errorSpy).toHaveBeenCalled());
    expect(container).toBeEmptyDOMElement();
    errorSpy.mockRestore();
  });

  it('clamps a long disabled reason to 200 characters', async () => {
    const longReason = 'x'.repeat(500);
    renderWithMocks(<MetadataStatusBanner />, {
      handlerOverrides: { metadata_store_status: async () => ({ state: 'disabled', reason: longReason }) },
    });
    const banner = await screen.findByRole('status');
    expect(banner).toHaveTextContent(`${'x'.repeat(199)}…`);
    expect(banner).not.toHaveTextContent(longReason);
  });
});
