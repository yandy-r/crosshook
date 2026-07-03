import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { WrapperChainItem } from '@/utils/derivePipelineDetail';
import { WrapperChain } from '../WrapperChain';

const items: WrapperChainItem[] = [
  {
    id: 'optimization-0',
    kind: 'optimization',
    token: 'gamemoderun',
    active: true,
    reason: "Enabled by launch optimization 'Use GameMode'",
  },
  {
    id: 'gamescope-1',
    kind: 'gamescope',
    token: 'gamescope --',
    active: false,
    reason: 'skipped: already inside a gamescope session and nested gamescope is not allowed',
  },
  {
    id: 'runtime-umu',
    kind: 'runtime',
    token: 'umu-run',
    active: true,
    reason: 'using umu-run at /usr/bin/umu-run',
    detail: 'csv coverage: found',
  },
];

describe('WrapperChain', () => {
  it('renders Active and Inactive text badges', () => {
    render(<WrapperChain items={items} emptyMessage="No wrappers" />);

    expect(screen.getAllByText('Active')).toHaveLength(2);
    expect(screen.getAllByText('Inactive')).toHaveLength(1);

    const rows = screen.getAllByRole('listitem');
    expect(rows[0]).toHaveAttribute('data-state', 'active');
    expect(rows[1]).toHaveAttribute('data-state', 'inactive');
  });

  it('renders token, reason, and detail lines', () => {
    render(<WrapperChain items={items} emptyMessage="No wrappers" />);

    expect(screen.getByText('gamemoderun')).toBeInTheDocument();
    expect(screen.getByText("Enabled by launch optimization 'Use GameMode'")).toBeInTheDocument();
    expect(
      screen.getByText('skipped: already inside a gamescope session and nested gamescope is not allowed')
    ).toBeInTheDocument();
    expect(screen.getByText('csv coverage: found')).toBeInTheDocument();
  });

  it('renders a folded annotation when foldedInto is set', () => {
    const folded: WrapperChainItem[] = [
      {
        id: 'optimization-0',
        kind: 'optimization',
        token: 'mangohud',
        active: true,
        reason: "Enabled by launch optimization 'Show MangoHud overlay'",
        foldedInto: 'gamescope --mangoapp',
      },
    ];
    render(<WrapperChain items={folded} emptyMessage="No wrappers" />);

    expect(screen.getByText('folded into gamescope --mangoapp')).toBeInTheDocument();
  });

  it('renders no folded annotation when foldedInto is absent', () => {
    render(<WrapperChain items={items} emptyMessage="No wrappers" />);

    expect(screen.queryByText(/folded into/)).not.toBeInTheDocument();
  });

  it('renders the empty message when items is empty', () => {
    render(<WrapperChain items={[]} emptyMessage="No wrappers for native launches" />);

    expect(screen.getByText('No wrappers for native launches')).toBeInTheDocument();
    expect(screen.queryByRole('list')).not.toBeInTheDocument();
  });
});
