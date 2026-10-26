import { act, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { emitMockEvent } from '@/lib/events';
import { renderWithMocks } from '@/test/render';
import { ConsoleView } from '../ConsoleView';

describe('ConsoleView', () => {
  it('renders the line of an object launch-log payload carrying a session_id', async () => {
    renderWithMocks(<ConsoleView />);

    await act(async () => {
      await Promise.resolve();
    });

    act(() => {
      emitMockEvent('launch-log', { line: 'game process started', session_id: 's1' });
    });

    expect(screen.getByText('game process started')).toBeInTheDocument();
    expect(screen.queryByText(/\{"line"/)).not.toBeInTheDocument();
    expect(screen.queryByText(/session_id/)).not.toBeInTheDocument();
  });
});
