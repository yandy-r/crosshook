import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { subscribeEvent } from '@/lib/events';
import { callCommand } from '@/lib/ipc';
import { useLaunchPrefixDependencyGate } from '../../hooks/useLaunchPrefixDependencyGate';
import { useLaunchPlatformStatus } from '../../hooks/useLaunchPlatformStatus';

import LaunchPanel from '../LaunchPanel';
import { RouteBanner } from '../layout/RouteBanner';
import { LaunchSubTabs } from '../LaunchSubTabs';
import { ThemedSelect } from '../ui/ThemedSelect';
import { useCollectionMembers } from '../../hooks/useCollectionMembers';
import { useCollections } from '../../hooks/useCollections';
import { useProfileContext } from '../../context/ProfileContext';
import { useProfileHealthContext } from '../../context/ProfileHealthContext';
import { usePreferencesContext } from '../../context/PreferencesContext';
import { useLaunchStateContext } from '../../context/LaunchStateContext';
import type { AcceptSuggestionRequest, ProtonDbRecommendationGroup } from '../../types/protondb';
import { DEFAULT_GAMESCOPE_CONFIG, DEFAULT_MANGOHUD_CONFIG } from '../../types/profile';
import { useProtonDbSuggestions } from '../../hooks/useProtonDbSuggestions';
import { resolveArtAppId } from '../../utils/art';
import { buildProfileLaunchRequest } from '../../utils/launch';
import {
  applyProtonDbGroupToProfile,
  mergeProtonDbEnvVarGroup,
  type PendingProtonDbOverwrite,
} from '../../utils/protondb';
import type { ProfileSummary } from '../../types/library';

const FLATPAK_NET_BADGE = 'No network isolation';
const FLATPAK_NET_BADGE_TITLE =
  'Flatpak cannot enforce network isolation (unshare) on this system. The profile still launches; traffic is not isolated.';

export function LaunchPage() {
  const profileState = useProfileContext();
  const launchPlatform = useLaunchPlatformStatus();
  const [profileNetworkIsolation, setProfileNetworkIsolation] = useState<Record<string, boolean>>({});
  const { activeCollectionId, setActiveCollectionId } = profileState;
  const { collections } = useCollections();
  const { memberNames, membersForCollectionId, loading: membersLoading } = useCollectionMembers(activeCollectionId);
  const activeCollection = useMemo(
    () =>
      activeCollectionId === null ? null : (collections.find((c) => c.collection_id === activeCollectionId) ?? null),
    [collections, activeCollectionId]
  );
  const filteredProfiles = useMemo(() => {
    if (activeCollectionId === null) {
      return profileState.profiles;
    }
    if (membersForCollectionId !== activeCollectionId || membersLoading) {
      return [];
    }
    if (memberNames.length === 0) {
      return [];
    }
    const set = new Set(memberNames);
    return profileState.profiles.filter((name) => set.has(name));
  }, [profileState.profiles, activeCollectionId, memberNames, membersForCollectionId, membersLoading]);

  useEffect(() => {
    if (activeCollectionId === null) {
      return;
    }
    if (membersLoading || membersForCollectionId !== activeCollectionId) {
      return;
    }
    const sel = profileState.selectedProfile.trim();
    if (filteredProfiles.length === 0) {
      if (sel !== '') {
        void profileState.selectProfile('');
      }
      return;
    }
    if (sel !== '' && !filteredProfiles.includes(sel)) {
      void profileState.selectProfile(filteredProfiles[0], {
        collectionId: activeCollectionId ?? undefined,
      });
    }
  }, [
    activeCollectionId,
    membersLoading,
    membersForCollectionId,
    filteredProfiles,
    profileState.selectedProfile,
    profileState.selectProfile,
  ]);

  useEffect(() => {
    let active = true;
    void callCommand<ProfileSummary[]>('profile_list_summaries')
      .then((rows) => {
        if (!active) {
          return;
        }
        const next: Record<string, boolean> = {};
        for (const row of rows) {
          next[row.name] = row.networkIsolation;
        }
        setProfileNetworkIsolation(next);
      })
      .catch(() => {
        if (active) {
          setProfileNetworkIsolation({});
        }
      });
    return () => {
      active = false;
    };
  }, [profileState.profiles]);

  const showFlatpakNetworkIsolationBadge = useCallback(
    (profileName: string) => {
      if (
        !launchPlatform?.isFlatpak ||
        launchPlatform.unshareNetAvailable ||
        !profileName.trim()
      ) {
        return false;
      }
      return profileNetworkIsolation[profileName] === true;
    },
    [launchPlatform, profileNetworkIsolation]
  );

  const { healthByName } = useProfileHealthContext();
  const { settings, defaultSteamClientInstallPath } = usePreferencesContext();
  const { launchGame, launchTrainer } = useLaunchStateContext();
  const { getDependencyStatus, installPrefixDependency, isGamescopeRunning } = useLaunchPrefixDependencyGate();
  const profile = profileState.profile;
  const selectedName = profileState.selectedProfile || '';
  const effectiveSteamClientInstallPath = useMemo(
    () => defaultSteamClientInstallPath || profileState.steamClientInstallPath,
    [defaultSteamClientInstallPath, profileState.steamClientInstallPath]
  );
  const launchRequest = buildProfileLaunchRequest(
    profile,
    profileState.launchMethod,
    effectiveSteamClientInstallPath,
    selectedName
  );
  const profileId = profileState.profileName.trim() || selectedName || 'new-profile';

  const pinnedSet = useMemo(() => new Set(profileState.favoriteProfiles), [profileState.favoriteProfiles]);
  const handleTogglePin = useCallback(
    (value: string) => {
      void profileState.toggleFavorite(value, !pinnedSet.has(value));
    },
    [pinnedSet, profileState.toggleFavorite]
  );

  const optimizationPresetNames = useMemo(
    () => Object.keys(profile.launch.presets ?? {}).sort((a, b) => a.localeCompare(b)),
    [profile.launch.presets]
  );
  const hasSavedSelectedProfile = useMemo(() => {
    const trimmedProfileName = profileState.profileName.trim();
    const trimmedSelectedProfile = profileState.selectedProfile.trim();
    return (
      trimmedProfileName.length > 0 &&
      trimmedSelectedProfile.length > 0 &&
      trimmedProfileName === trimmedSelectedProfile &&
      profileState.profiles.includes(trimmedProfileName)
    );
  }, [profileState.profileName, profileState.profiles, profileState.selectedProfile]);

  const selectedTrainerVersion =
    profileState.selectedProfile.trim().length > 0
      ? (healthByName[profileState.selectedProfile]?.metadata?.trainer_version ?? null)
      : null;

  const showProtonDbLookup =
    profileState.launchMethod === 'steam_applaunch' || profileState.launchMethod === 'proton_run';

  const [pendingProtonDbOverwrite, setPendingProtonDbOverwrite] = useState<PendingProtonDbOverwrite | null>(null);
  const [applyingProtonDbGroupId, setApplyingProtonDbGroupId] = useState<string | null>(null);
  const [protonDbStatusMessage, setProtonDbStatusMessage] = useState<string | null>(null);
  const suggestions = useProtonDbSuggestions(resolveArtAppId(profile), selectedName);

  const handleAcceptSuggestion = useCallback(
    async (request: AcceptSuggestionRequest): Promise<void> => {
      const result = await suggestions.acceptSuggestion(request);
      if (result.appliedKeys.length > 0 || result.toggledOptionIds.length > 0) {
        // LaunchPage: reload reflects active collection context if any.
        void profileState.selectProfile(selectedName, {
          collectionId: activeCollectionId ?? undefined,
        });
      }
    },
    [suggestions.acceptSuggestion, profileState.selectProfile, selectedName, activeCollectionId]
  );

  // Dep gate modal state
  const [depGatePackages, setDepGatePackages] = useState<string[] | null>(null);
  const [depGatePendingAction, setDepGatePendingAction] = useState<'game' | 'trainer' | null>(null);
  const [depGateInstalling, setDepGateInstalling] = useState(false);

  const environmentAutosaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const persistProfileDraftRef = useRef(profileState.persistProfileDraft);
  const latestProfileRef = useRef(profileState.profile);
  const latestProfileNameRef = useRef(profileState.profileName);
  const latestNextEnvVarsRef = useRef<Readonly<Record<string, string>>>({});

  useEffect(() => {
    persistProfileDraftRef.current = profileState.persistProfileDraft;
    latestProfileRef.current = profileState.profile;
    latestProfileNameRef.current = profileState.profileName;
  }, [profileState.persistProfileDraft, profileState.profile, profileState.profileName]);

  useEffect(() => {
    return () => {
      if (environmentAutosaveTimerRef.current !== null) {
        clearTimeout(environmentAutosaveTimerRef.current);
        environmentAutosaveTimerRef.current = null;
      }
    };
  }, []);

  const resolvedSteamAppId = resolveArtAppId(profile);
  useEffect(() => {
    setPendingProtonDbOverwrite(null);
    setApplyingProtonDbGroupId(null);
    setProtonDbStatusMessage(null);
  }, [profileState.profileName, resolvedSteamAppId, profileState.launchMethod]);

  // Dep gate: listen for prefix-dep-complete while installing
  useEffect(() => {
    if (!depGateInstalling) return;

    const prefixPath = profile.runtime?.prefix_path ?? profile.steam?.compatdata_path ?? '';

    const unlistenPromise = subscribeEvent<{
      profile_name: string;
      prefix_path: string;
      succeeded: boolean;
    }>('prefix-dep-complete', (event) => {
      if (event.payload.profile_name !== selectedName || event.payload.prefix_path !== prefixPath) return;

      setDepGateInstalling(false);
      if (event.payload.succeeded) {
        const action = depGatePendingAction;
        setDepGatePackages(null);
        setDepGatePendingAction(null);
        if (action === 'game') {
          launchGame();
        } else if (action === 'trainer') {
          launchTrainer();
        }
      } else {
        setDepGatePackages(null);
        setDepGatePendingAction(null);
      }
    });

    return () => {
      void unlistenPromise.then((fn) => fn());
    };
  }, [depGateInstalling, selectedName, profile, depGatePendingAction, launchGame, launchTrainer]);

  const handleBeforeLaunch = useCallback(
    async (action: 'game' | 'trainer'): Promise<boolean> => {
      const requiredPackages = profile.trainer?.required_protontricks;
      if (!requiredPackages || requiredPackages.length === 0) return true;

      const prefixPath = profile.runtime?.prefix_path ?? profile.steam?.compatdata_path ?? '';
      if (!prefixPath) return true;

      try {
        const statuses = await getDependencyStatus(selectedName, prefixPath);

        const missing = requiredPackages.filter((pkg) => {
          const status = statuses.find((s) => s.package_name === pkg);
          return (
            !status || status.state === 'missing' || status.state === 'install_failed' || status.state === 'unknown'
          );
        });

        if (missing.length === 0) return true;

        if (settings.auto_install_prefix_deps) {
          // Auto-install: invoke and wait for the prefix-dep-complete event
          setDepGatePackages(missing);
          setDepGatePendingAction(action);
          setDepGateInstalling(true);
          try {
            await installPrefixDependency(selectedName, prefixPath, missing);
          } catch {
            setDepGateInstalling(false);
            setDepGatePackages(null);
            setDepGatePendingAction(null);
          }
          return false;
        }

        // Show gate modal
        setDepGatePackages(missing);
        setDepGatePendingAction(action);
        return false;
      } catch {
        // Cannot check — allow launch
        return true;
      }
    },
    [profile, selectedName, settings.auto_install_prefix_deps, getDependencyStatus, installPrefixDependency]
  );

  const applyProtonDbGroup = useCallback(
    (group: ProtonDbRecommendationGroup, overwriteKeys: readonly string[]) => {
      const result = { appliedKeys: [] as string[], unchangedKeys: [] as string[], toggledOptionIds: [] as string[] };
      profileState.updateProfile((current) => {
        const applyResult = applyProtonDbGroupToProfile(current, group, overwriteKeys, profileState.catalog);
        result.appliedKeys = applyResult.appliedKeys;
        result.unchangedKeys = applyResult.unchangedKeys;
        result.toggledOptionIds = applyResult.toggledOptionIds;
        return applyResult.nextProfile;
      });
      setApplyingProtonDbGroupId(null);
      setPendingProtonDbOverwrite(null);

      const appliedCount = result.appliedKeys.length;
      const unchangedCount = result.unchangedKeys.length;
      const toggledCount = result.toggledOptionIds.length;
      if (appliedCount > 0 || toggledCount > 0) {
        const parts: string[] = [];
        if (toggledCount > 0) parts.push(`${toggledCount} optimization${toggledCount === 1 ? '' : 's'}`);
        if (appliedCount - toggledCount > 0) {
          const envCount = appliedCount - toggledCount;
          parts.push(`${envCount} env var${envCount === 1 ? '' : 's'}`);
        }
        setProtonDbStatusMessage(
          `Applied ${parts.join(' and ')}${
            unchangedCount > 0
              ? ` and left ${unchangedCount} existing match${unchangedCount === 1 ? '' : 'es'} unchanged`
              : ''
          }.`
        );
        return;
      }

      if (unchangedCount > 0) {
        setProtonDbStatusMessage('All suggested ProtonDB environment variables already match the current profile.');
        return;
      }

      setProtonDbStatusMessage('No ProtonDB environment-variable changes were applied.');
    },
    [profileState.updateProfile, profileState.catalog]
  );

  const handleApplyProtonDbEnvVars = useCallback(
    (group: ProtonDbRecommendationGroup) => {
      const envVars = group.env_vars ?? [];
      if (envVars.length === 0) {
        return;
      }

      setApplyingProtonDbGroupId(group.group_id?.trim() || group.title?.trim() || null);
      const merge = mergeProtonDbEnvVarGroup(profile.launch.custom_env_vars, group);
      if (merge.conflicts.length === 0) {
        applyProtonDbGroup(group, []);
        return;
      }

      setApplyingProtonDbGroupId(null);
      setPendingProtonDbOverwrite({
        group,
        conflicts: merge.conflicts,
        resolutions: Object.fromEntries(merge.conflicts.map((conflict) => [conflict.key, 'keep_current' as const])),
      });
      setProtonDbStatusMessage(null);
    },
    [applyProtonDbGroup, profile.launch.custom_env_vars]
  );

  const handleEnvironmentBlurAutoSave = useCallback(
    (
      trigger: 'key' | 'value',
      row: Readonly<{ key: string; value: string }>,
      nextEnvVars: Readonly<Record<string, string>>
    ) => {
      if (!hasSavedSelectedProfile) {
        return;
      }
      if (trigger === 'value' && row.key.trim().length === 0) {
        return;
      }
      latestNextEnvVarsRef.current = { ...nextEnvVars };
      if (environmentAutosaveTimerRef.current !== null) {
        clearTimeout(environmentAutosaveTimerRef.current);
      }
      environmentAutosaveTimerRef.current = setTimeout(() => {
        const latestProfile = latestProfileRef.current;
        const latestProfileName = latestProfileNameRef.current;
        void persistProfileDraftRef.current(latestProfileName, {
          ...latestProfile,
          launch: {
            ...latestProfile.launch,
            custom_env_vars: { ...latestNextEnvVarsRef.current },
          },
        });
      }, 400);
    },
    [hasSavedSelectedProfile]
  );

  return (
    <div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--launch">
      <div className="crosshook-route-stack crosshook-launch-page__grid">
        <RouteBanner route="launch" />
        <LaunchPanel
          profileId={profileId}
          method={profileState.launchMethod}
          request={launchRequest}
          profile={profile}
          profileSelectSlot={
            <>
              {activeCollection !== null && (
                <div className="crosshook-launch-collection-filter">
                  Filtering by: <strong>{activeCollection.name}</strong>
                  <button
                    type="button"
                    className="crosshook-button crosshook-button--ghost crosshook-button--small"
                    onClick={() => setActiveCollectionId(null)}
                    aria-label="Clear collection filter"
                  >
                    ×
                  </button>
                </div>
              )}
              <ThemedSelect
                id="launch-profile-selector"
                value={profileState.selectedProfile}
                onValueChange={(name) =>
                  // LaunchPage threads `activeCollectionId` so Rust merges the
                  // collection's launch defaults via `effective_profile_with`.
                  // Editor safety: `ProfilesPage` MUST NOT pass collectionId.
                  void profileState.selectProfile(name, {
                    collectionId: activeCollectionId ?? undefined,
                  })
                }
                placeholder="Select a profile"
                pinnedValues={pinnedSet}
                onTogglePin={handleTogglePin}
                ariaLabelledby="launch-active-profile-label"
                options={filteredProfiles.map((name) => ({
                  value: name,
                  label: name,
                  badge: showFlatpakNetworkIsolationBadge(name) ? FLATPAK_NET_BADGE : undefined,
                  badgeTitle: showFlatpakNetworkIsolationBadge(name) ? FLATPAK_NET_BADGE_TITLE : undefined,
                }))}
              />
            </>
          }
          tabsSlot={
            <LaunchSubTabs
              launchMethod={profileState.launchMethod}
              steamAppId={resolveArtAppId(profile)}
              customCoverArtPath={profile.game.custom_cover_art_path}
              gamescopeConfig={profile.launch.gamescope ?? DEFAULT_GAMESCOPE_CONFIG}
              onGamescopeChange={(gamescope) => {
                profileState.updateLaunchSetting((current) => ({
                  ...current,
                  launch: { ...current.launch, gamescope },
                }));
              }}
              isInsideGamescopeSession={isGamescopeRunning}
              mangoHudConfig={profile.launch.mangohud ?? DEFAULT_MANGOHUD_CONFIG}
              onMangoHudChange={(mangohud) => {
                profileState.updateLaunchSetting((current) => ({
                  ...current,
                  launch: { ...current.launch, mangohud },
                }));
              }}
              showMangoHudOverlayEnabled={profile.launch.optimizations.enabled_option_ids.includes(
                'show_mangohud_overlay'
              )}
              enabledOptionIds={profile.launch.optimizations.enabled_option_ids}
              onToggleOption={profileState.toggleLaunchOptimization}
              launchOptimizationsStatus={profileState.launchOptimizationsStatus}
              optimizationPresetNames={optimizationPresetNames}
              activeOptimizationPreset={profile.launch.active_preset ?? ''}
              onSelectOptimizationPreset={(name) => {
                void profileState.switchLaunchOptimizationPreset(name);
              }}
              bundledOptimizationPresets={profileState.bundledOptimizationPresets}
              onApplyBundledPreset={(presetId) => {
                void profileState.applyBundledOptimizationPreset(presetId);
              }}
              optimizationPresetActionBusy={profileState.optimizationPresetActionBusy}
              onSaveManualPreset={profileState.saveManualOptimizationPreset}
              catalog={profileState.catalog}
              customEnvVars={profile.launch.custom_env_vars}
              profileName={profileState.profileName}
              onUpdateProfile={profileState.updateLaunchSetting}
              onEnvironmentBlurAutoSave={handleEnvironmentBlurAutoSave}
              showProtonDbLookup={showProtonDbLookup}
              trainerVersion={selectedTrainerVersion}
              onApplyProtonDbEnvVars={handleApplyProtonDbEnvVars}
              applyingProtonDbGroupId={applyingProtonDbGroupId}
              protonDbStatusMessage={protonDbStatusMessage}
              pendingProtonDbOverwrite={pendingProtonDbOverwrite}
              onConfirmProtonDbOverwrite={(overwriteKeys) => {
                if (pendingProtonDbOverwrite) {
                  applyProtonDbGroup(pendingProtonDbOverwrite.group, overwriteKeys);
                }
              }}
              onCancelProtonDbOverwrite={() => setPendingProtonDbOverwrite(null)}
              onUpdateProtonDbResolution={(key, resolution) =>
                setPendingProtonDbOverwrite((current) =>
                  current == null ? current : { ...current, resolutions: { ...current.resolutions, [key]: resolution } }
                )
              }
              suggestionSet={suggestions.suggestionSet}
              onAcceptSuggestion={handleAcceptSuggestion}
              onDismissSuggestion={suggestions.dismissSuggestion}
              gamescopeAutoSaveStatus={profileState.gamescopeAutoSaveStatus}
              mangoHudAutoSaveStatus={profileState.mangoHudAutoSaveStatus}
            />
          }
          onBeforeLaunch={handleBeforeLaunch}
        />
      </div>

      {depGatePackages !== null && (
        <div className="crosshook-modal-overlay" role="dialog" aria-modal="true" aria-labelledby="dep-gate-title">
          <div className="crosshook-modal crosshook-prefix-deps__confirm">
            <h3 id="dep-gate-title">Missing Prefix Dependencies</h3>
            <p>
              This profile requires WINE prefix dependencies that are not installed. You can install them now or skip
              and launch anyway.
            </p>
            <ul>
              {depGatePackages.map((pkg) => (
                <li key={pkg}>
                  <code>{pkg}</code>
                </li>
              ))}
            </ul>
            {depGateInstalling ? <p className="crosshook-muted">Installing dependencies...</p> : null}
            <div className="crosshook-modal__actions">
              <button
                type="button"
                className="crosshook-button"
                disabled={depGateInstalling}
                onClick={() => {
                  void (async () => {
                    const prefixPath = profile.runtime?.prefix_path ?? profile.steam?.compatdata_path ?? '';
                    setDepGateInstalling(true);
                    try {
                      await installPrefixDependency(selectedName, prefixPath, depGatePackages);
                    } catch {
                      setDepGateInstalling(false);
                    }
                  })();
                }}
              >
                Install + Launch
              </button>
              <button
                type="button"
                className="crosshook-button crosshook-button--secondary"
                disabled={depGateInstalling}
                onClick={() => {
                  const action = depGatePendingAction;
                  setDepGatePackages(null);
                  setDepGatePendingAction(null);
                  if (action === 'game') {
                    launchGame();
                  } else if (action === 'trainer') {
                    launchTrainer();
                  }
                }}
              >
                Skip and Launch
              </button>
              <button
                type="button"
                className="crosshook-button crosshook-button--secondary"
                disabled={depGateInstalling}
                onClick={() => {
                  setDepGatePackages(null);
                  setDepGatePendingAction(null);
                  setDepGateInstalling(false);
                }}
              >
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default LaunchPage;
