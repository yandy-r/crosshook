import { useMemo } from 'react';
import { useProfileContext } from '@/context/ProfileContext';
import type { LibraryCardData } from '@/types/library';
import type { GameProfile, InjectionSection, LoadedDllHook } from '@/types/profile';
import { DashboardPanelSection } from '../layout/DashboardPanelSection';
import { InjectionConfigPanel } from './trainer/InjectionConfigPanel';
import { InjectionLogTail } from './trainer/InjectionLogTail';
import { LoadedDllHookListPanel } from './trainer/LoadedDllHookListPanel';
import { useHeroTrainerAutosave } from './trainer/useHeroTrainerAutosave';

export interface HeroDetailTrainerTabProps {
  summary: LibraryCardData;
  displayProfileName?: string;
}

function syncLegacyInjectionMirrors(injection: InjectionSection): InjectionSection {
  return {
    ...injection,
    dll_paths: injection.loaded_hooks.map((hook) => hook.path),
    inject_on_launch: injection.loaded_hooks.map((hook) => hook.enabled),
  };
}

export function HeroDetailTrainerTab({ summary, displayProfileName }: HeroDetailTrainerTabProps) {
  const { profile, profileName, selectedProfile, profiles, updateProfile, persistProfileDraft } = useProfileContext();

  const selectedTrimmed = selectedProfile.trim();
  const profileNameTrimmed = profileName.trim();
  const resolvedProfileName = displayProfileName?.trim() || selectedTrimmed || profileNameTrimmed || summary.name;

  // `hasSavedSelectedProfile` gates autosave to the ProfileContext-selected
  // profile. Mirrors HeroDetailLaunchTab so editor writes never target an
  // unsaved or stale singleton profile.
  const hasSavedSelectedProfile =
    selectedTrimmed.length > 0 && profiles.includes(selectedTrimmed) && profileNameTrimmed === selectedTrimmed;

  // The Trainer tab writes through ProfileContext's selected profile. If the
  // displayed profile differs from selectedProfile, writes would target the
  // wrong profile, so keep the editor visible but non-editable.
  const profileMismatch = useMemo(() => {
    if (selectedTrimmed.length === 0) {
      return false;
    }
    const displayedName = displayProfileName?.trim() ?? '';
    if (displayedName.length === 0) {
      return false;
    }
    return displayedName !== selectedTrimmed;
  }, [selectedTrimmed, displayProfileName]);

  const { trainerAutoSaveStatus, scheduleTrainerAutosave } = useHeroTrainerAutosave({
    hasSavedSelectedProfile,
    profile,
    profileName,
    persistProfileDraft,
  });

  function updateInjection(nextInjection: InjectionSection) {
    const normalizedInjection = syncLegacyInjectionMirrors(nextInjection);
    const nextProfile: GameProfile = {
      ...profile,
      injection: normalizedInjection,
    };
    updateProfile(() => nextProfile);
    scheduleTrainerAutosave(nextProfile);
  }

  function updateLoadedHooks(hooks: LoadedDllHook[]) {
    updateInjection(
      syncLegacyInjectionMirrors({
        ...profile.injection,
        loaded_hooks: hooks,
      })
    );
  }

  return (
    <div className="crosshook-hero-detail__trainer-tab">
      <DashboardPanelSection
        title="Loaded DLL hooks"
        titleAs="h3"
        className="crosshook-hero-detail__section"
        actions={
          <span
            className={`crosshook-hero-detail__trainer-save crosshook-hero-detail__trainer-save--${trainerAutoSaveStatus.tone}`}
          >
            {trainerAutoSaveStatus.label}
          </span>
        }
      >
        <div className="crosshook-hero-detail__hooks-stack">
          {profileMismatch ? (
            <p className="crosshook-hero-detail__muted" role="status">
              Trainer settings apply to the selected profile ({selectedProfile || 'none'}). Select{' '}
              {resolvedProfileName || 'this game'} to edit its DLL hook declarations here.
            </p>
          ) : (
            <>
              {trainerAutoSaveStatus.detail ? (
                <p className="crosshook-hero-detail__warn" role="status">
                  {trainerAutoSaveStatus.detail}
                </p>
              ) : null}
              <LoadedDllHookListPanel hooks={profile.injection.loaded_hooks} onUpdate={updateLoadedHooks} />
            </>
          )}
        </div>
      </DashboardPanelSection>

      <DashboardPanelSection title="Injection configuration" titleAs="h3" className="crosshook-hero-detail__section">
        {profileMismatch ? (
          <p className="crosshook-hero-detail__muted" role="status">
            Select {resolvedProfileName || 'this game'} to edit stored injection configuration.
          </p>
        ) : (
          <InjectionConfigPanel injection={profile.injection} onUpdate={updateInjection} />
        )}
      </DashboardPanelSection>

      <DashboardPanelSection title="Recent injection log" titleAs="h3" className="crosshook-hero-detail__section">
        <InjectionLogTail profileName={resolvedProfileName} />
      </DashboardPanelSection>
    </div>
  );
}

export default HeroDetailTrainerTab;
