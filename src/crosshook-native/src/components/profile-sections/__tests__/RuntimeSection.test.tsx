import { fireEvent, render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { makeProfileDraft } from '@/test/fixtures';
import type { GameProfile } from '@/types';
import { RuntimeSection } from '../RuntimeSection';

const usePreferencesContextMock = vi.fn();

vi.mock('@/context/PreferencesContext', () => ({
  usePreferencesContext: () => usePreferencesContextMock(),
}));

function renderRuntimeSection({
  profile = makeProfileDraft({
    runtime: {
      prefix_path: '/prefix',
      proton_path: '/proton',
      working_directory: '',
      umu_store: '',
      umu_codename: '',
    },
    launch: { ...makeProfileDraft().launch, method: 'proton_run' },
  }),
  onUpdateProfile = vi.fn(),
}: {
  profile?: GameProfile;
  onUpdateProfile?: (updater: (current: GameProfile) => GameProfile) => void;
} = {}) {
  render(
    <RuntimeSection
      profile={profile}
      onUpdateProfile={onUpdateProfile}
      launchMethod="proton_run"
      protonInstalls={[]}
      protonInstallsError={null}
    />
  );

  return { profile, onUpdateProfile };
}

describe('RuntimeSection', () => {
  beforeEach(() => {
    usePreferencesContextMock.mockReturnValue({ settings: { umu_preference: 'auto' } });
  });

  it('updates umu store in the proton_run runtime section', () => {
    const onUpdateProfile = vi.fn();
    const { profile } = renderRuntimeSection({ onUpdateProfile });

    fireEvent.change(screen.getByLabelText('umu store'), { target: { value: 'gog' } });

    const updater = onUpdateProfile.mock.calls.at(-1)?.[0] as ((current: GameProfile) => GameProfile) | undefined;
    expect(updater?.(profile).runtime.umu_store).toBe('gog');
  });

  it('updates umu codename in the proton_run runtime section', () => {
    const onUpdateProfile = vi.fn();
    const { profile } = renderRuntimeSection({ onUpdateProfile });

    fireEvent.change(screen.getByLabelText('umu codename'), { target: { value: 'cyberpunk_2077' } });

    const updater = onUpdateProfile.mock.calls.at(-1)?.[0] as ((current: GameProfile) => GameProfile) | undefined;
    expect(updater?.(profile).runtime.umu_codename).toBe('cyberpunk_2077');
  });

  it('explains the Settings dependency for umu store and codename lookup fields', () => {
    renderRuntimeSection();

    expect(screen.getAllByText(/Settings -> umu GAMEID lookup is enabled/)).toHaveLength(2);
  });
});
