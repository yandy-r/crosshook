import { render, screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { LaunchPreview } from '@/types/launch';
import { HighlightedCommandBlock } from '../HighlightedCommandBlock';

function makePreview(overrides: Partial<LaunchPreview> = {}): LaunchPreview {
  return {
    resolved_method: 'proton_run',
    validation: { issues: [] },
    environment: [
      { key: 'DXVK_HUD', value: 'fps', source: 'profile_custom' },
      { key: 'MALICIOUS', value: '<script>alert("x")</script> $()\nnext', source: 'profile_custom' },
      { key: 'HOME', value: '/home/devuser', source: 'host' },
    ],
    cleared_variables: [],
    wrappers: ['gamescope', 'mangohud'],
    effective_command: 'gamescope -- mangohud /usr/bin/proton run /games/synthetic quest/game.exe --flag',
    directives_error: null,
    steam_launch_options: null,
    proton_setup: {
      wine_prefix_path: '/prefixes/synthetic',
      compat_data_path: '/steam/compatdata/9999001',
      steam_client_install_path: '/steam/root',
      proton_executable: '/usr/bin/proton',
      umu_run_path: null,
    },
    working_directory: '/games/synthetic quest',
    game_executable: '/games/synthetic quest/game.exe',
    game_executable_name: 'game.exe',
    trainer: null,
    generated_at: '2026-04-23T12:00:00.000Z',
    display_text: '',
    umu_decision: null,
    ...overrides,
  };
}

describe('HighlightedCommandBlock', () => {
  it('renders display tokens with stable tone classes', () => {
    render(<HighlightedCommandBlock preview={makePreview()} profileName="Synthetic Quest" />);

    const block = screen.getByLabelText('Synthetic Quest launch command preview');
    expect(within(block).getByText(/proton_run launch preview/)).toHaveClass(
      'crosshook-hero-detail__command-token--comment'
    );
    expect(within(block).getByText('DXVK_HUD')).toHaveClass('crosshook-hero-detail__command-token--env-key');
    expect(within(block).getByText(JSON.stringify('fps'))).toHaveClass('crosshook-hero-detail__command-token--value');
    expect(within(block).getByText('gamescope')).toHaveClass('crosshook-hero-detail__command-token--binary');
    expect(within(block).getByText('/games/synthetic quest/game.exe')).toHaveClass(
      'crosshook-hero-detail__command-token--binary'
    );
    expect(within(block).getByText('--flag')).toHaveClass('crosshook-hero-detail__command-token--flag');
  });

  it('renders unsafe values as text without injecting script elements', () => {
    render(<HighlightedCommandBlock preview={makePreview()} />);

    expect(screen.getByText(JSON.stringify('<script>alert("x")</script> $()\nnext'))).toBeInTheDocument();
    expect(document.querySelector('script')).toBeNull();
  });

  it('does not promote bare -- or non-flag hyphen tokens from effective_command', () => {
    render(
      <HighlightedCommandBlock
        preview={makePreview({
          effective_command: 'gamescope -- mangohud /usr/bin/proton run /games/synthetic quest/game.exe --flag -=bad',
        })}
      />
    );

    const block = screen.getByLabelText('Launch command preview');
    expect(within(block).getByText('--flag')).toHaveClass('crosshook-hero-detail__command-token--flag');
    expect(within(block).queryByText('--', { exact: true })).toBeNull();
    expect(within(block).queryByText('-=bad')).toBeNull();
  });

  it('shows ellipsis when effective_command flags exceed the display cap', () => {
    const manyFlags = Array.from({ length: 10 }, (_, index) => `--opt${index}`).join(' ');
    render(
      <HighlightedCommandBlock
        preview={makePreview({
          wrappers: [],
          effective_command: `/games/game.exe ${manyFlags}`,
        })}
      />
    );

    const block = screen.getByLabelText('Launch command preview');
    expect(within(block).getByText('--opt0')).toHaveClass('crosshook-hero-detail__command-token--flag');
    expect(within(block).getByText('--opt7')).toHaveClass('crosshook-hero-detail__command-token--flag');
    expect(within(block).queryByText('--opt8')).toBeNull();
    expect(within(block).getByText('…')).toHaveClass('crosshook-hero-detail__command-token--comment');
  });

  it('keeps wrapper binaries separate when effective_command overlaps wrapper names', () => {
    render(
      <HighlightedCommandBlock
        preview={makePreview({
          wrappers: ['gamescope', 'mangohud'],
          effective_command: 'gamescope -- mangohud /usr/bin/proton run /games/synthetic quest/game.exe --flag',
        })}
      />
    );

    const block = screen.getByLabelText('Launch command preview');
    const gamescopeTokens = within(block).getAllByText('gamescope');
    const mangohudTokens = within(block).getAllByText('mangohud');
    expect(gamescopeTokens).toHaveLength(1);
    expect(mangohudTokens).toHaveLength(1);
    expect(gamescopeTokens[0]).toHaveClass('crosshook-hero-detail__command-token--binary');
    expect(within(block).queryByText('--', { exact: true })).toBeNull();
  });
});
