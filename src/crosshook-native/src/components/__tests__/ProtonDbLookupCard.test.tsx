import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { UseProtonDbLookupResult } from '../../hooks/useProtonDbLookup';
import type { ProtonDbSuggestionSet } from '../../types/protondb';
import { ProtonDbLookupCard } from '../ProtonDbLookupCard';

const SNAPSHOT = { app_id: '12345', tier: 'gold', recommendation_groups: [] };

vi.mock('../../hooks/useProtonDbLookup', () => ({
  useProtonDbLookup: (): UseProtonDbLookupResult => ({
    appId: '12345',
    state: 'ready',
    loading: false,
    lookup: { app_id: '12345', state: 'ready', cache: null, snapshot: SNAPSHOT },
    snapshot: SNAPSHOT,
    cache: null,
    recommendationGroups: [],
    fromCache: false,
    isStale: false,
    isUnavailable: false,
    refresh: async () => {},
  }),
}));

const SUGGESTION_SET: ProtonDbSuggestionSet = {
  catalogSuggestions: [
    {
      catalogEntryId: 'dxvk_async',
      label: 'DXVK async',
      description: 'Async shader compilation',
      envPairs: [['DXVK_ASYNC', '1']],
      status: 'new',
      supportingReportCount: 3,
    },
  ],
  envVarSuggestions: [],
  launchOptionSuggestions: [],
  tier: 'gold',
  totalReports: 10,
  isStale: false,
};

describe('ProtonDbLookupCard suggestion accept', () => {
  it('surfaces a failed accept as an alert instead of swallowing it', async () => {
    const onAcceptSuggestion = vi.fn().mockRejectedValue(new Error('accept failed'));
    render(<ProtonDbLookupCard appId="12345" suggestionSet={SUGGESTION_SET} onAcceptSuggestion={onAcceptSuggestion} />);

    await userEvent.click(screen.getByRole('button', { name: 'Enable' }));

    expect(onAcceptSuggestion).toHaveBeenCalledWith({ kind: 'catalog', catalogEntryId: 'dxvk_async' });
    expect(await screen.findByRole('alert')).toHaveTextContent('accept failed');
  });

  it('clears the accept error when the suggestion set changes', async () => {
    const onAcceptSuggestion = vi.fn().mockRejectedValue(new Error('accept failed'));
    const { rerender } = render(
      <ProtonDbLookupCard appId="12345" suggestionSet={SUGGESTION_SET} onAcceptSuggestion={onAcceptSuggestion} />
    );

    await userEvent.click(screen.getByRole('button', { name: 'Enable' }));
    expect(await screen.findByRole('alert')).toHaveTextContent('accept failed');

    rerender(
      <ProtonDbLookupCard
        appId="12345"
        suggestionSet={{ ...SUGGESTION_SET, totalReports: 11 }}
        onAcceptSuggestion={onAcceptSuggestion}
      />
    );

    expect(screen.queryByRole('alert')).not.toBeInTheDocument();
  });
});
