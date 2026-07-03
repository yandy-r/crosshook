import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { DetectedModCandidate, ProfileModRecord } from '@/types/mods';
import { HeroModsSection } from '../mods/HeroModsSection';

const callCommandMock = vi.fn();

vi.mock('@/lib/ipc', () => ({
  callCommand: (name: string, args?: unknown) => callCommandMock(name, args),
}));

function buildMod(overrides: Partial<ProfileModRecord> = {}): ProfileModRecord {
  return {
    mod_id: 'mod-1',
    profile_id: 'profile-1',
    name: 'ReShade',
    category: 'overlay_injection',
    paths: ['dxgi.dll', 'ReShade.ini'],
    enabled: true,
    provenance: 'detected',
    source_url: 'https://example.com/reshade',
    created_at: '2026-01-01T00:00:00+00:00',
    updated_at: '2026-01-01T00:00:00+00:00',
    ...overrides,
  };
}

type Handler = (args: unknown) => unknown;

function installHandlers(overrides: Record<string, Handler> = {}) {
  const defaults: Record<string, Handler> = {
    list_profile_mods: () => ({ available: true, mods: [] }),
    analyze_mod_coexistence: () => [],
  };
  callCommandMock.mockImplementation((name: string, args: unknown) => {
    const handler = overrides[name] ?? defaults[name];
    if (!handler) {
      return Promise.reject(new Error(`unexpected command: ${name}`));
    }
    try {
      return Promise.resolve(handler(args));
    } catch (error) {
      return Promise.reject(error);
    }
  });
}

function renderSection(
  overrides: Partial<{
    profileName: string | undefined;
    hasTrainerConfigured: boolean;
    gameExecutablePath: string;
  }> = {}
) {
  return render(
    <HeroModsSection
      profileName={overrides.profileName ?? 'Elden Ring'}
      hasTrainerConfigured={overrides.hasTrainerConfigured ?? true}
      gameExecutablePath={overrides.gameExecutablePath ?? '/games/elden-ring/eldenring.exe'}
    />
  );
}

describe('HeroModsSection', () => {
  beforeEach(() => {
    callCommandMock.mockReset();
  });

  it('renders empty state CTAs', async () => {
    installHandlers();
    renderSection();

    expect(await screen.findByText('No mods registered for this game yet.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Add mod' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Scan game directory' })).toBeEnabled();
  });

  it('renders rows with chip badge paths and link', async () => {
    installHandlers({
      list_profile_mods: () => ({ available: true, mods: [buildMod()] }),
    });
    renderSection();

    expect(await screen.findByText('ReShade')).toBeInTheDocument();
    expect(screen.getByText('Overlay / injection')).toBeInTheDocument();
    expect(screen.getByText('Detected')).toBeInTheDocument();
    expect(screen.getByText('dxgi.dll +1 more')).toBeInTheDocument();
    const link = screen.getByRole('link', { name: 'example.com' });
    expect(link).toHaveAttribute('href', 'https://example.com/reshade');
    expect(link).toHaveAttribute('rel', 'noreferrer');
    expect(screen.getByRole('switch', { name: 'Enable ReShade' })).toHaveAttribute('aria-checked', 'true');
    expect(
      await screen.findByText('No coexistence conflicts detected for the current trainer loading mode.')
    ).toBeInTheDocument();
  });

  it('scan button disabled without game path and hint linked', async () => {
    installHandlers();
    renderSection({ gameExecutablePath: '   ' });

    const button = await screen.findByRole('button', { name: 'Scan game directory' });
    expect(button).toBeDisabled();
    const hint = screen.getByText('Set the game executable path to enable detection.');
    expect(button).toHaveAttribute('aria-describedby', hint.id);
  });

  it('unavailable notice replaces controls', async () => {
    installHandlers({
      list_profile_mods: () => ({ available: false, mods: [] }),
    });
    renderSection();

    const notice = await screen.findByRole('status');
    expect(notice).toHaveTextContent(
      'Mod registry is unavailable — the metadata database could not be opened. Launches will proceed without coexistence advisories.'
    );
    expect(screen.queryByRole('button', { name: 'Add mod' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Scan game directory' })).toBeNull();
  });

  it('announces registration in live region', async () => {
    const registered: ProfileModRecord[] = [];
    installHandlers({
      list_profile_mods: () => ({ available: true, mods: [...registered] }),
      add_profile_mod: () => {
        const record = buildMod({ name: 'SKSE64', category: 'script_extender', provenance: 'manual' });
        registered.push(record);
        return record;
      },
    });
    const { container } = renderSection();

    fireEvent.click(await screen.findByRole('button', { name: 'Add mod' }));
    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'SKSE64' } });
    fireEvent.submit(container.querySelector('form') as HTMLFormElement);

    await waitFor(() => {
      expect(container.querySelector('[aria-live="polite"]')).toHaveTextContent('SKSE64 registered.');
    });
    expect(await screen.findByText('SKSE64')).toBeInTheDocument();
  });

  it('advisory error renders inline muted line', async () => {
    installHandlers({
      list_profile_mods: () => ({ available: true, mods: [buildMod()] }),
      analyze_mod_coexistence: () => {
        throw 'boom';
      },
    });
    renderSection();

    expect(await screen.findByText('Advisories could not be computed: boom')).toBeInTheDocument();
    expect(screen.getByText('ReShade')).toBeInTheDocument();
  });

  it('registers detected candidates sequentially with a single advisory refresh', async () => {
    const candidates: DetectedModCandidate[] = [
      {
        detector_id: 'reshade',
        suggested_name: 'ReShade',
        category: 'overlay_injection',
        matched_paths: ['ReShade.ini'],
        already_registered: false,
      },
      {
        detector_id: 'script_extender:skse64',
        suggested_name: 'SKSE64',
        category: 'script_extender',
        matched_paths: ['skse64_loader.exe'],
        already_registered: false,
      },
    ];
    let added = 0;
    installHandlers({
      detect_profile_mods: () => ({
        scanned_root: '/games/elden-ring',
        candidates,
        entries_scanned: 2,
        truncated: false,
      }),
      add_profile_mod: (args) => {
        added += 1;
        const { input } = args as { input: { name: string } };
        return buildMod({ mod_id: `mod-${added}`, name: input.name });
      },
    });
    renderSection();

    fireEvent.click(await screen.findByRole('button', { name: 'Scan game directory' }));
    fireEvent.click(await screen.findByRole('button', { name: 'Register 2 selected' }));

    await waitFor(() =>
      expect(callCommandMock.mock.calls.filter(([name]) => name === 'add_profile_mod')).toHaveLength(2)
    );
    expect(await screen.findByText('SKSE64')).toBeInTheDocument();
    expect(screen.getByText('ReShade')).toBeInTheDocument();
    await waitFor(() =>
      expect(callCommandMock.mock.calls.filter(([name]) => name === 'analyze_mod_coexistence')).toHaveLength(1)
    );
  });

  it('no-trainer note shown when hasTrainerConfigured false', async () => {
    installHandlers({
      list_profile_mods: () => ({ available: true, mods: [buildMod()] }),
    });
    renderSection({ hasTrainerConfigured: false });

    expect(
      await screen.findByText('No trainer loading mode configured — advisories apply once a trainer is set.')
    ).toBeInTheDocument();
  });
});
