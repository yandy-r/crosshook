import { fireEvent, render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { makeProfileDraft } from '@/test/fixtures';
import type { GameProfile } from '@/types';
import { TrainerSection } from '../TrainerSection';

vi.mock('@/hooks/useTrainerTypeCatalog', () => ({
  useTrainerTypeCatalog: () => ({
    catalog: [],
    error: null,
    selectOptions: [{ value: 'unknown', label: 'Unknown' }],
  }),
}));

vi.mock('@/components/ui/InfoTooltip', () => ({
  InfoTooltip: () => null,
}));

function renderTrainerSection(requiredProtontricks: string[] = []) {
  const profile = makeProfileDraft();
  profile.trainer.required_protontricks = requiredProtontricks;
  const onUpdateProfile = vi.fn();

  render(
    <TrainerSection
      profile={profile}
      onUpdateProfile={onUpdateProfile}
      launchMethod="proton_run"
      profileName="Ghost of Tsushima (Proton)"
    />
  );

  return { profile, onUpdateProfile };
}

function latestUpdatedProfile(
  profile: GameProfile,
  onUpdateProfile: ReturnType<typeof vi.fn>
): GameProfile | undefined {
  const updater = onUpdateProfile.mock.calls.at(-1)?.[0] as ((current: GameProfile) => GameProfile) | undefined;
  return updater?.(profile);
}

describe('TrainerSection prefix dependencies', () => {
  it('adds a normalized dependency verb', () => {
    const { profile, onUpdateProfile } = renderTrainerSection();

    fireEvent.change(screen.getByLabelText('Add prefix dependency'), { target: { value: '  dotnet48  ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add dependency' }));

    expect(latestUpdatedProfile(profile, onUpdateProfile)?.trainer.required_protontricks).toEqual(['dotnet48']);
  });

  it('removes an existing dependency verb', () => {
    const { profile, onUpdateProfile } = renderTrainerSection(['dotnet48', 'corefonts']);

    fireEvent.click(screen.getByRole('button', { name: 'Remove dotnet48 dependency' }));

    expect(latestUpdatedProfile(profile, onUpdateProfile)?.trainer.required_protontricks).toEqual(['corefonts']);
  });

  it('rejects flag-like dependency input', () => {
    const { onUpdateProfile } = renderTrainerSection();

    fireEvent.change(screen.getByLabelText('Add prefix dependency'), { target: { value: '--help' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add dependency' }));

    expect(screen.getByRole('alert')).toHaveTextContent('lowercase letters, numbers, underscores, or hyphens');
    expect(onUpdateProfile).not.toHaveBeenCalled();
  });

  it('does not add a duplicate dependency', () => {
    const { onUpdateProfile } = renderTrainerSection(['dotnet48']);

    fireEvent.change(screen.getByLabelText('Add prefix dependency'), { target: { value: 'dotnet48' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add dependency' }));

    expect(screen.getByRole('alert')).toHaveTextContent('already declared');
    expect(onUpdateProfile).not.toHaveBeenCalled();
  });
});
