import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { makeLibraryCardData } from '@/test/fixtures';
import { Inspector } from '../Inspector';
import { ROUTE_METADATA } from '../routeMetadata';

describe('Inspector', () => {
  it('renders data-testid on root aside', () => {
    render(<Inspector route="profiles" width={320} />);
    expect(screen.getByTestId('inspector')).toBeInTheDocument();
  });

  it('shows empty route copy when route has no inspector body', () => {
    render(<Inspector route="profiles" width={320} />);
    expect(screen.getByText('No inspector content for this route')).toBeInTheDocument();
  });

  it('renders error boundary fallback when inspector body throws', () => {
    const prev = ROUTE_METADATA.library.inspectorComponent;
    function Throwing(): never {
      throw new Error('boom');
    }
    ROUTE_METADATA.library.inspectorComponent = Throwing;
    try {
      render(<Inspector route="library" width={320} selection={makeLibraryCardData()} />);
      expect(screen.getByText('Inspector unavailable.')).toBeInTheDocument();
    } finally {
      ROUTE_METADATA.library.inspectorComponent = prev;
    }
  });
});
