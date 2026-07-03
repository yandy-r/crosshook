import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it } from 'vitest';
import { makeLaunchPreview, makeUmuDecisionPreview } from '@/test/fixtures';
import { PipelineDetailPanel } from '../PipelineDetailPanel';

describe('PipelineDetailPanel', () => {
  it('renders a disabled toggle with a dry-run hint when preview is null', () => {
    render(<PipelineDetailPanel preview={null} />);

    expect(screen.getByRole('button', { name: /pipeline details/i })).toBeDisabled();
    expect(screen.getByText('Run Dry-run to inspect the pipeline')).toBeInTheDocument();
    expect(screen.queryByRole('region', { name: 'Launch pipeline details' })).not.toBeInTheDocument();
  });

  it('toggle click flips aria-expanded and mounts the region', async () => {
    const user = userEvent.setup();
    render(<PipelineDetailPanel preview={makeLaunchPreview()} />);

    const toggle = screen.getByRole('button', { name: /pipeline details/i });
    expect(toggle).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByRole('region', { name: 'Launch pipeline details' })).not.toBeInTheDocument();

    await user.click(toggle);

    expect(toggle).toHaveAttribute('aria-expanded', 'true');
    const region = screen.getByRole('region', { name: 'Launch pipeline details' });
    expect(toggle).toHaveAttribute('aria-controls', region.id);

    await user.click(toggle);

    expect(toggle).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByRole('region', { name: 'Launch pipeline details' })).not.toBeInTheDocument();
  });

  it('toggle is keyboard operable via Enter and Space', async () => {
    const user = userEvent.setup();
    render(<PipelineDetailPanel preview={makeLaunchPreview()} />);

    const toggle = screen.getByRole('button', { name: /pipeline details/i });
    toggle.focus();

    await user.keyboard('{Enter}');
    expect(toggle).toHaveAttribute('aria-expanded', 'true');
    expect(screen.getByRole('region', { name: 'Launch pipeline details' })).toBeInTheDocument();

    await user.keyboard(' ');
    expect(toggle).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByRole('region', { name: 'Launch pipeline details' })).not.toBeInTheDocument();
  });

  it('shows the directives error banner and chain fallback when environment is null', async () => {
    const user = userEvent.setup();
    render(
      <PipelineDetailPanel
        preview={makeLaunchPreview({
          environment: null,
          wrappers: null,
          wrapper_details: null,
          directives_error: 'boom',
        })}
      />
    );

    await user.click(screen.getByRole('button', { name: /pipeline details/i }));

    expect(screen.getByRole('alert')).toHaveTextContent('boom');
    expect(screen.getByText('Wrapper details unavailable for this preview.')).toBeInTheDocument();
    expect(screen.getByText('No environment variables.')).toBeInTheDocument();
  });

  it('renders summary meta with env count, wrapper count, and umu summary', () => {
    render(<PipelineDetailPanel preview={makeLaunchPreview({ umu_decision: makeUmuDecisionPreview() })} />);

    expect(screen.getByRole('button', { name: /pipeline details/i })).toHaveTextContent(
      '4 env vars · 1 wrapper · umu: umu-run'
    );
  });

  it('collapses when the preview is cleared and stays collapsed for a new preview', async () => {
    const user = userEvent.setup();
    const { rerender } = render(<PipelineDetailPanel preview={makeLaunchPreview()} />);

    const toggle = screen.getByRole('button', { name: /pipeline details/i });
    await user.click(toggle);
    expect(toggle).toHaveAttribute('aria-expanded', 'true');

    rerender(<PipelineDetailPanel preview={null} />);

    expect(toggle).toHaveAttribute('aria-expanded', 'false');
    expect(toggle).not.toHaveAttribute('aria-controls');
    expect(screen.queryByRole('region', { name: 'Launch pipeline details' })).not.toBeInTheDocument();

    rerender(<PipelineDetailPanel preview={makeLaunchPreview()} />);

    expect(toggle).toHaveAttribute('aria-expanded', 'false');
    expect(screen.queryByRole('region', { name: 'Launch pipeline details' })).not.toBeInTheDocument();
  });

  it('renders wrapper chain and environment groups inside the open panel', async () => {
    const user = userEvent.setup();
    render(<PipelineDetailPanel preview={makeLaunchPreview()} />);

    await user.click(screen.getByRole('button', { name: /pipeline details/i }));

    expect(screen.getByText('Wrapper chain')).toBeInTheDocument();
    expect(screen.getByText('Environment')).toBeInTheDocument();
    expect(screen.getByText('gamemoderun')).toBeInTheDocument();
    expect(screen.getByText('Cleared before launch')).toBeInTheDocument();
    expect(screen.getByText('WINEPREFIX')).toBeInTheDocument();
  });
});
