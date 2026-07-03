import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { CatalogFacets, CatalogQuery } from '@/types/discovery';
import { DiscoveryFacetBar } from '../DiscoveryFacetBar';

function buildFacets(overrides: Partial<CatalogFacets> = {}): CatalogFacets {
  return {
    gameTitles: [
      { value: 'Elden Ring', count: 2 },
      { value: 'Sekiro', count: 1 },
      { value: 'Empty Game', count: 0 },
    ],
    loadingModes: [{ value: 'source_directory', count: 3 }],
    compatibilityBands: [
      { value: 'working', count: 2 },
      { value: 'platinum', count: 1 },
    ],
    taps: [
      { value: 'https://example.com/awesome-taps.git', count: 2 },
      { value: 'https://example.com/other-taps.git', count: 1 },
    ],
    ...overrides,
  };
}

function renderBar(query: CatalogQuery = {}, facets: CatalogFacets = buildFacets()) {
  const onSetFacet = vi.fn();
  const onClearFilters = vi.fn();
  render(<DiscoveryFacetBar facets={facets} query={query} onSetFacet={onSetFacet} onClearFilters={onClearFilters} />);
  return { onSetFacet, onClearFilters };
}

async function openSelect(name: string) {
  const user = userEvent.setup();
  await user.click(screen.getByRole('combobox', { name }));
  return user;
}

describe('DiscoveryFacetBar', () => {
  it('renders counts inside option labels', async () => {
    renderBar();

    await openSelect('Game title');

    expect(await screen.findByRole('option', { name: 'Elden Ring (2)' })).toBeInTheDocument();
    expect(screen.getByRole('option', { name: 'Sekiro (1)' })).toBeInTheDocument();
    expect(screen.getByRole('option', { name: 'All games' })).toBeInTheDocument();
  });

  it('selecting a value calls onSetFacet with the dimension key', async () => {
    const { onSetFacet } = renderBar();

    const user = await openSelect('Loading mode');
    await user.click(await screen.findByRole('option', { name: 'Run from current directory (3)' }));

    expect(onSetFacet).toHaveBeenCalledWith('loadingMode', 'source_directory');
  });

  it('selecting the head option clears the dimension', async () => {
    const { onSetFacet } = renderBar({ gameTitles: ['Elden Ring'] });

    const user = await openSelect('Game title');
    await user.click(await screen.findByRole('option', { name: 'All games' }));

    expect(onSetFacet).toHaveBeenCalledWith('gameTitle', undefined);
  });

  it('zero count options are disabled', async () => {
    renderBar();

    await openSelect('Game title');

    const empty = await screen.findByRole('option', { name: 'Empty Game (0)' });
    expect(empty).toHaveAttribute('aria-disabled', 'true');
    expect(screen.getByRole('option', { name: 'Elden Ring (2)' })).not.toHaveAttribute('aria-disabled', 'true');
  });

  it('fixed loading mode and rating menus include absent values at zero', async () => {
    renderBar();

    await openSelect('Loading mode');
    expect(await screen.findByRole('option', { name: 'Run from current directory (3)' })).toBeInTheDocument();
    const copyToPrefix = screen.getByRole('option', { name: 'Copy into prefix (0)' });
    const unknownMode = screen.getByRole('option', { name: 'Unknown (0)' });
    expect(copyToPrefix).toHaveAttribute('aria-disabled', 'true');
    expect(unknownMode).toHaveAttribute('aria-disabled', 'true');

    // Close and open the compatibility menu.
    await userEvent.keyboard('{Escape}');
    await openSelect('Compatibility');
    expect(await screen.findByRole('option', { name: 'Working (2)' })).toBeInTheDocument();
    expect(screen.getByRole('option', { name: 'Platinum (1)' })).toBeInTheDocument();
    for (const absent of ['Partial (0)', 'Broken (0)', 'Unknown (0)']) {
      expect(screen.getByRole('option', { name: absent })).toHaveAttribute('aria-disabled', 'true');
    }
  });

  it('active chips expose remove buttons with aria labels', async () => {
    const { onSetFacet } = renderBar({
      gameTitles: ['Elden Ring'],
      tapUrls: ['https://example.com/awesome-taps.git'],
    });

    expect(screen.getByText('Game: Elden Ring')).toBeInTheDocument();
    expect(screen.getByText('Tap: awesome-taps')).toBeInTheDocument();

    await userEvent.click(screen.getByRole('button', { name: 'Remove filter Game Elden Ring' }));
    expect(onSetFacet).toHaveBeenCalledWith('gameTitle', undefined);

    await userEvent.click(screen.getByRole('button', { name: 'Remove filter Tap awesome-taps' }));
    expect(onSetFacet).toHaveBeenCalledWith('tap', undefined);
  });

  it('clear all appears only with two or more active filters', async () => {
    const single = renderBar({ gameTitles: ['Elden Ring'] });
    expect(screen.queryByRole('button', { name: 'Clear all' })).not.toBeInTheDocument();
    expect(single.onClearFilters).not.toHaveBeenCalled();

    // Re-render with two active dimensions.
    document.body.innerHTML = '';
    const double = renderBar({ gameTitles: ['Elden Ring'], compatibilityBands: ['working'] });

    const clearAll = screen.getByRole('button', { name: 'Clear all' });
    await userEvent.click(clearAll);
    expect(double.onClearFilters).toHaveBeenCalledTimes(1);
  });
});
