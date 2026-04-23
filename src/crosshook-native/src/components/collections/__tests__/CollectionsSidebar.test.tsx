import { fireEvent, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { CollectionsProvider } from '@/context/CollectionsContext';
import { renderWithMocks } from '@/test/render';
import { CollectionsSidebar } from '../CollectionsSidebar';

describe('CollectionsSidebar', () => {
  it('renders collection actions and opens the selected collection', async () => {
    const onOpenCollection = vi.fn();

    renderWithMocks(
      <CollectionsProvider>
        <CollectionsSidebar onOpenCollection={onOpenCollection} />
      </CollectionsProvider>
    );

    const collectionButton = await screen.findByRole('button', { name: /Action \/ Adventure/i });
    fireEvent.click(collectionButton);

    expect(onOpenCollection).toHaveBeenCalledWith('mock-collection-1');
    expect(screen.getByRole('button', { name: 'New Collection' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Import Preset' })).toBeInTheDocument();
  });
});
