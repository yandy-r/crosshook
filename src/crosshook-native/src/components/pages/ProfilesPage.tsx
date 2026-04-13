import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { callCommand } from '@/lib/ipc';
import { usePreferencesContext } from '../../context/PreferencesContext';
import { useProfileContext } from '../../context/ProfileContext';
import { useProfileHealthContext } from '../../context/ProfileHealthContext';
import { useCollectionMembers } from '../../hooks/useCollectionMembers';
import { useCollections } from '../../hooks/useCollections';
import type { CommunityExportResult } from '../../hooks/useCommunityProfiles';
import { useLaunchPlatformStatus } from '../../hooks/useLaunchPlatformStatus';
import { useOfflineReadiness } from '../../hooks/useOfflineReadiness';
import { useProfileSummaries } from '../../hooks/useProfileSummaries';
import { resolveProtonUpProviderForVersion, useProtonUp } from '../../hooks/useProtonUp';
import { useTrainerTypeCatalog } from '../../hooks/useTrainerTypeCatalog';
import type { ProtonInstallOption } from '../../types/proton';
import type { ProtonUpSuggestion } from '../../types/protonup';
import { chooseSaveFile } from '../../utils/dialog';
import { formatRelativeTime } from '../../utils/format';
import { LAUNCH_PANEL_ACTION_BUTTON_STYLE } from '../../utils/launchPanelActionButtonStyle';
import { deriveTargetHomePath } from '../../utils/steam';
import ConfigHistoryPanel from '../ConfigHistoryPanel';
import { HealthBadge } from '../HealthBadge';
import { RouteBanner } from '../layout/RouteBanner';
import { OfflineStatusBadge } from '../OfflineStatusBadge';
import { OnboardingWizard } from '../OnboardingWizard';
import { PrefixDepsPanel } from '../PrefixDepsPanel';
import ProfileActions from '../ProfileActions';
import ProfilePreviewModal from '../ProfilePreviewModal';
import ProfileSubTabs from '../ProfileSubTabs';
import { CollapsibleSection } from '../ui/CollapsibleSection';
import { ThemedSelect } from '../ui/ThemedSelect';

const NETWORK_ISOLATION_BADGE = 'No network isolation';
const NETWORK_ISOLATION_BADGE_TITLE =
  'This system cannot enforce network isolation (unshare --net). The profile still launches; traffic is not isolated.';

/** Minimal shape of a row returned by `community_list_indexed_profiles`. */
interface CommunityIndexedProfileRow {
  game_name: string | null;
  proton_version: string | null;
}

function suggestedCommunityExportFilename(profileName: string): string {
  const base = profileName
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
  return `${base || 'community-profile'}.json`;
}

interface RenameToast {
  newName: string;
  oldName: string;
}

const RENAME_TOAST_DURATION_MS = 6000;
const HEALTH_BANNER_DISMISSED_SESSION_KEY = 'crosshook.healthBannerDismissed';
const RENAME_TOAST_DISMISSED_SESSION_KEY = 'crosshook.renameToastDismissed';

function sortProtonInstalls(installs: ProtonInstallOption[]): ProtonInstallOption[] {
  return [...installs].sort((left, right) => {
    if (left.is_official !== right.is_official) {
      return left.is_official ? -1 : 1;
    }

    return left.name.localeCompare(right.name) || left.path.localeCompare(right.path);
  });
}

export function ProfilesPage() {
  const { defaultSteamClientInstallPath } = usePreferencesContext();
  const {
    deleting,
    dirty,
    duplicateProfile,
    duplicating,
    error,
    executeDelete,
    favoriteProfiles,
    loading,
    pendingDelete,
    profile,
    profileExists,
    profileName,
    profiles,
    refreshProfiles,
    renameProfile,
    renaming,
    saveProfile,
    saving,
    selectProfile,
    selectedProfile,
    setProfileName,
    cancelDelete,
    confirmDelete,
    toggleFavorite,
    updateProfile,
    launchMethod,
    steamClientInstallPath,
    fetchConfigHistory,
    fetchConfigDiff,
    rollbackConfig,
    markKnownGood,
    activeCollectionId,
    setActiveCollectionId,
  } = useProfileContext();
  const { collections } = useCollections();
  const { memberNames, membersForCollectionId, loading: membersLoading } = useCollectionMembers(activeCollectionId);
  const activeCollection = useMemo(
    () =>
      activeCollectionId === null ? null : (collections.find((c) => c.collection_id === activeCollectionId) ?? null),
    [collections, activeCollectionId]
  );
  const filteredProfiles = useMemo(() => {
    if (activeCollectionId === null) {
      return profiles;
    }
    if (membersLoading || membersForCollectionId !== activeCollectionId) {
      return [];
    }
    if (memberNames.length === 0) {
      return [];
    }
    const set = new Set(memberNames);
    return profiles.filter((name) => set.has(name));
  }, [profiles, activeCollectionId, memberNames, membersLoading, membersForCollectionId]);

  const launchPlatform = useLaunchPlatformStatus();
  const { summaries: profileSummaries } = useProfileSummaries(profiles, activeCollectionId);
  const profileNetworkIsolation = useMemo(() => {
    const next: Record<string, boolean> = {};
    for (const row of profileSummaries) {
      next[row.name] = row.networkIsolation;
    }
    return next;
  }, [profileSummaries]);

  useEffect(() => {
    if (activeCollectionId === null) {
      return;
    }
    if (membersLoading || membersForCollectionId !== activeCollectionId) {
      return;
    }
    const sel = selectedProfile.trim();
    if (filteredProfiles.length === 0) {
      if (sel !== '') {
        void selectProfile('');
      }
      return;
    }
    if (sel !== '' && !filteredProfiles.includes(sel)) {
      void selectProfile(filteredProfiles[0]);
    }
  }, [activeCollectionId, membersLoading, membersForCollectionId, filteredProfiles, selectedProfile, selectProfile]);

  const showNetworkIsolationBadge = useCallback(
    (candidateProfileName: string) => {
      if (!launchPlatform || launchPlatform.unshareNetAvailable || !candidateProfileName.trim()) {
        return false;
      }

      if (candidateProfileName.trim() === selectedProfile.trim()) {
        return profile.launch.network_isolation ?? true;
      }

      return profileNetworkIsolation[candidateProfileName] === true;
    },
    [launchPlatform, profile.launch.network_isolation, profileNetworkIsolation, selectedProfile]
  );

  const [protonInstalls, setProtonInstalls] = useState<ProtonInstallOption[]>([]);
  const [protonInstallsError, setProtonInstallsError] = useState<string | null>(null);
  const [pendingRename, setPendingRename] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState('');
  const renameInputRef = useRef<HTMLInputElement>(null);
  const [renameToast, setRenameToast] = useState<RenameToast | null>(null);
  const [healthBannerDismissed, setHealthBannerDismissed] = useState(() => {
    try {
      return sessionStorage.getItem(HEALTH_BANNER_DISMISSED_SESSION_KEY) === '1';
    } catch {
      return false;
    }
  });
  const [renameToastDismissed, setRenameToastDismissed] = useState(() => {
    try {
      return sessionStorage.getItem(RENAME_TOAST_DISMISSED_SESSION_KEY) === '1';
    } catch {
      return false;
    }
  });
  const renameToastTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [pendingLauncherReExport, setPendingLauncherReExport] = useState(false);
  const [showWizard, setShowWizard] = useState(false);
  const [wizardMode, setWizardMode] = useState<'create' | 'edit'>('create');
  const [showProfilePreview, setShowProfilePreview] = useState(false);
  const [profilePreviewContent, setProfilePreviewContent] = useState('');
  const [previewing, setPreviewing] = useState(false);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [exportingCommunity, setExportingCommunity] = useState(false);
  const [communityExportError, setCommunityExportError] = useState<string | null>(null);
  const [communityExportSuccess, setCommunityExportSuccess] = useState<string | null>(null);
  const [showHistoryPanel, setShowHistoryPanel] = useState(false);
  const healthIssuesRef = useRef<HTMLDivElement>(null);

  const {
    batchValidate,
    revalidateSingle,
    healthByName,
    summary,
    loading: healthLoading,
    staleInfoByName,
    cachedSnapshots,
    trendByName,
  } = useProfileHealthContext();
  const offlineReadiness = useOfflineReadiness();
  const { labels: trainerTypeLabels } = useTrainerTypeCatalog();

  const effectiveSteamClientInstallPath = useMemo(
    () => defaultSteamClientInstallPath || steamClientInstallPath,
    [defaultSteamClientInstallPath, steamClientInstallPath]
  );
  const targetHomePath = useMemo(
    () => deriveTargetHomePath(effectiveSteamClientInstallPath),
    [effectiveSteamClientInstallPath]
  );

  const protonUp = useProtonUp({
    steamClientInstallPath: effectiveSteamClientInstallPath,
  });
  const [suggestion, setSuggestion] = useState<ProtonUpSuggestion | null>(null);
  const [suggestionDismissed, setSuggestionDismissed] = useState(false);
  const [suggestionInstallError, setSuggestionInstallError] = useState<string | null>(null);
  const canSave =
    profileName.trim().length > 0 && profile.game.executable_path.trim().length > 0 && !saving && !deleting && !loading;
  const canDelete = profileExists && !saving && !deleting && !loading && !duplicating && !renaming;
  const canDuplicate = profileExists && !saving && !deleting && !loading && !duplicating && !renaming;
  const canRename = profileExists && !saving && !deleting && !loading && !duplicating && !renaming;
  const canPreview = profileName.trim().length > 0 && !loading;
  const canExportCommunity =
    profileExists && !saving && !deleting && !loading && !duplicating && !renaming && !exportingCommunity;
  const canViewHistory =
    Boolean(selectedProfile.trim()) &&
    profiles.includes(selectedProfile.trim()) &&
    !saving &&
    !deleting &&
    !loading &&
    !duplicating &&
    !renaming &&
    !exportingCommunity;
  useEffect(() => {
    let active = true;

    async function loadProtonInstalls() {
      try {
        const installs = await callCommand<ProtonInstallOption[]>('list_proton_installs', {
          steamClientInstallPath:
            effectiveSteamClientInstallPath.trim().length > 0 ? effectiveSteamClientInstallPath : undefined,
        });

        if (!active) {
          return;
        }

        setProtonInstalls(sortProtonInstalls(installs));
        setProtonInstallsError(null);
      } catch (loadError) {
        if (!active) {
          return;
        }

        setProtonInstalls([]);
        setProtonInstallsError(loadError instanceof Error ? loadError.message : String(loadError));
      }
    }

    void loadProtonInstalls();

    return () => {
      active = false;
    };
  }, [effectiveSteamClientInstallPath]);

  useEffect(() => {
    setSuggestionDismissed(false);
    setSuggestion(null);
    setSuggestionInstallError(null);

    const gameName = profile.game.name.trim();
    if (!gameName || !selectedProfile) {
      return;
    }

    let active = true;

    async function fetchSuggestion() {
      try {
        const rows = await callCommand<CommunityIndexedProfileRow[]>('community_list_indexed_profiles');
        if (!active) {
          return;
        }

        const normalizedGame = gameName.toLowerCase();
        const match = rows.find(
          (row) =>
            typeof row.game_name === 'string' &&
            row.game_name.trim().toLowerCase() === normalizedGame &&
            typeof row.proton_version === 'string' &&
            row.proton_version.trim().length > 0
        );

        if (!match?.proton_version) {
          return;
        }

        const result = await protonUp.getSuggestion(match.proton_version);
        if (active) {
          setSuggestion(result);
        }
      } catch {
        // Advisory-only: silently ignore errors; no suggestion shown on failure
      }
    }

    void fetchSuggestion();

    return () => {
      active = false;
    };
  }, [selectedProfile, profile.game.name, protonUp.getSuggestion]);

  useEffect(() => {
    if (pendingRename !== null) {
      renameInputRef.current?.select();
    }
  }, [pendingRename]);

  useEffect(() => {
    setCommunityExportError(null);
    setCommunityExportSuccess(null);
  }, []);

  // F2 keyboard shortcut: open rename dialog when a profile is selected and no modal is open
  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if (event.key !== 'F2') {
        return;
      }

      // Skip if focus is inside an editable element
      const target = event.target;
      if (
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        (target instanceof HTMLElement && target.isContentEditable)
      ) {
        return;
      }

      // Skip if a modal is already open
      if (pendingRename !== null || pendingDelete !== null) {
        return;
      }

      // Only open if a saved profile is selected and rename is allowed
      if (!canRename || !selectedProfile) {
        return;
      }

      event.preventDefault();
      setPendingRename(selectedProfile);
      setRenameValue(selectedProfile);
    }

    document.addEventListener('keydown', handleKeyDown);
    return () => document.removeEventListener('keydown', handleKeyDown);
  }, [canRename, pendingDelete, pendingRename, selectedProfile]);

  // Clean up toast timer on unmount
  useEffect(() => {
    return () => {
      if (renameToastTimerRef.current !== null) {
        clearTimeout(renameToastTimerRef.current);
      }
    };
  }, []);

  const showRenameToast = useCallback((oldName: string, newName: string) => {
    if (renameToastTimerRef.current !== null) {
      clearTimeout(renameToastTimerRef.current);
    }

    setRenameToastDismissed(false);
    try {
      sessionStorage.removeItem(RENAME_TOAST_DISMISSED_SESSION_KEY);
    } catch {
      // Ignore storage errors in restricted environments.
    }

    setRenameToast({ oldName, newName });
    renameToastTimerRef.current = setTimeout(() => {
      setRenameToast(null);
      renameToastTimerRef.current = null;
    }, RENAME_TOAST_DURATION_MS);
  }, []);

  const dismissRenameToast = useCallback(() => {
    if (renameToastTimerRef.current !== null) {
      clearTimeout(renameToastTimerRef.current);
      renameToastTimerRef.current = null;
    }

    setRenameToast(null);
    setRenameToastDismissed(true);
    try {
      sessionStorage.setItem(RENAME_TOAST_DISMISSED_SESSION_KEY, '1');
    } catch {
      // Ignore storage errors in restricted environments.
    }
  }, []);

  const dismissHealthBanner = useCallback(() => {
    setHealthBannerDismissed(true);
    try {
      sessionStorage.setItem(HEALTH_BANNER_DISMISSED_SESSION_KEY, '1');
    } catch {
      // Ignore storage errors in restricted environments.
    }
  }, []);

  const handleSave = useCallback(async () => {
    await saveProfile();
    if (profileName.trim()) {
      void revalidateSingle(profileName.trim());
    }
  }, [saveProfile, profileName, revalidateSingle]);

  const handleAfterRollback = useCallback(
    (name: string) => {
      void revalidateSingle(name);
    },
    [revalidateSingle]
  );

  const undoRename = useCallback(() => {
    if (!renameToast) {
      return;
    }

    const { oldName, newName } = renameToast;
    dismissRenameToast();
    void renameProfile(newName, oldName).then(({ ok, hadLauncher }) => {
      if (!ok) {
        return;
      }

      if (hadLauncher) {
        setPendingLauncherReExport(true);
      }
    });
  }, [dismissRenameToast, renameProfile, renameToast]);

  const handleRenameConfirm = useCallback(
    (oldName: string, newName: string) => {
      setPendingRename(null);
      void renameProfile(oldName, newName).then(({ ok, hadLauncher }) => {
        if (!ok) {
          return;
        }

        showRenameToast(oldName, newName);
        if (hadLauncher) {
          setPendingLauncherReExport(true);
        }
      });
    },
    [renameProfile, showRenameToast]
  );

  const handleExportCommunityProfile = useCallback(async () => {
    const nameOnDisk = selectedProfile.trim();
    if (!nameOnDisk || !profiles.includes(nameOnDisk)) {
      setCommunityExportError('Save the profile before exporting as a community manifest.');
      setCommunityExportSuccess(null);
      return;
    }

    setCommunityExportError(null);
    setCommunityExportSuccess(null);

    const outputPath = await chooseSaveFile('Export community profile', {
      defaultPath: suggestedCommunityExportFilename(nameOnDisk),
      filters: [{ name: 'JSON', extensions: ['json'] }],
    });

    if (outputPath === null) {
      return;
    }

    setExportingCommunity(true);
    try {
      const result = await callCommand<CommunityExportResult>('community_export_profile', {
        profile_name: nameOnDisk,
        output_path: outputPath,
      });
      setCommunityExportSuccess(`Community profile saved to ${result.output_path}`);
      setCommunityExportError(null);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      console.error('Community profile export failed:', err);
      setCommunityExportError(message);
      setCommunityExportSuccess(null);
    } finally {
      setExportingCommunity(false);
    }
  }, [profiles, selectedProfile]);

  async function handlePreviewProfile() {
    setPreviewing(true);
    setPreviewError(null);
    try {
      const toml = await callCommand<string>('profile_export_toml', {
        name: profileName,
        data: profile,
      });
      setProfilePreviewContent(toml);
      setPreviewError(null);
      setShowProfilePreview(true);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      console.error('Profile preview failed:', err);
      setPreviewError(message);
    } finally {
      setPreviewing(false);
    }
  }

  function handleCloseProfilePreview() {
    setShowProfilePreview(false);
    setPreviewError(null);
  }

  const renameNameTrimmed = renameValue.trim();
  const renameIsEmpty = renameNameTrimmed.length === 0;
  const renameIsUnchanged = pendingRename !== null && renameNameTrimmed === pendingRename;
  const renameHasConflict =
    !renameIsEmpty &&
    !renameIsUnchanged &&
    profiles.some((name) => name.toLowerCase() === renameNameTrimmed.toLowerCase());
  const renameError = renameIsEmpty
    ? 'Profile name cannot be empty.'
    : renameHasConflict
      ? `A profile named '${renameNameTrimmed}' already exists.`
      : null;
  const canConfirmRename = !renameIsEmpty && !renameIsUnchanged && !renameHasConflict && !renaming;
  const selectedReport = selectedProfile ? healthByName[selectedProfile] : undefined;
  const selectedCachedSnapshot = selectedProfile ? cachedSnapshots[selectedProfile] : undefined;
  const selectedStaleInfo = selectedProfile ? staleInfoByName[selectedProfile] : undefined;
  const selectedTrend = selectedProfile ? (trendByName[selectedProfile] ?? null) : null;
  const selectedVersionStatus = selectedReport?.metadata?.version_status ?? null;
  const selectedTrainerVersion = selectedReport?.metadata?.trainer_version ?? null;
  const hasSelectedProfile = selectedProfile.trim().length > 0;

  const VERSION_STATUS_LABELS: Record<string, string> = {
    game_updated: 'Game updated',
    trainer_changed: 'Trainer changed',
    both_changed: 'Both changed',
    update_in_progress: 'Update in progress',
  };

  const renderVersionStatusBadge = () => {
    if (
      !selectedVersionStatus ||
      selectedVersionStatus === 'untracked' ||
      selectedVersionStatus === 'unknown' ||
      selectedVersionStatus === 'matched'
    ) {
      return null;
    }

    const isWarning =
      selectedVersionStatus === 'game_updated' ||
      selectedVersionStatus === 'trainer_changed' ||
      selectedVersionStatus === 'both_changed';

    return (
      <span
        className={`crosshook-status-chip crosshook-version-badge crosshook-version-badge--${isWarning ? 'warning' : 'info'}`}
        title={
          isWarning ? 'Version mismatch detected since last successful launch' : 'Steam is currently updating this game'
        }
      >
        {VERSION_STATUS_LABELS[selectedVersionStatus] ?? selectedVersionStatus}
      </span>
    );
  };

  const trainerTypeDisplayName = useMemo(() => {
    const id = profile.trainer.trainer_type?.trim() || 'unknown';
    return trainerTypeLabels[id] ?? id;
  }, [profile.trainer.trainer_type, trainerTypeLabels]);

  const renderOfflineStatusBadge = () => {
    if (!selectedProfile) {
      return null;
    }
    const report = offlineReadiness.reportForProfile(selectedProfile);
    return <OfflineStatusBadge report={report ?? undefined} />;
  };

  const renderProfileHealthBadge = () => {
    if (!selectedProfile) {
      return null;
    }

    if (!selectedReport && !selectedCachedSnapshot) {
      return null;
    }

    if (selectedReport) {
      const issueCount = selectedReport.issues.length;
      const issueTooltip =
        issueCount > 0
          ? `${issueCount} issue${issueCount !== 1 ? 's' : ''}: ${selectedReport.issues
              .slice(0, 3)
              .map((i) => `${i.field} \u2014 ${i.message}`)
              .join('; ')}${issueCount > 3 ? ` (+${issueCount - 3} more)` : ''}`
          : null;

      return (
        <HealthBadge
          report={selectedReport}
          metadata={selectedReport.metadata}
          trend={selectedTrend}
          tooltip={issueTooltip}
          onClick={
            issueCount > 0
              ? () => healthIssuesRef.current?.scrollIntoView({ behavior: 'smooth', block: 'start' })
              : undefined
          }
        />
      );
    }

    const badgeStatus = selectedCachedSnapshot?.status;
    if (!badgeStatus) {
      return null;
    }

    const issueCount = selectedCachedSnapshot.issue_count;
    const issueTooltip = issueCount > 0 ? `${issueCount} issue${issueCount !== 1 ? 's' : ''} in cached snapshot` : null;

    return (
      <HealthBadge
        status={badgeStatus}
        trend={selectedTrend}
        tooltip={issueTooltip}
        onClick={
          selectedReport && issueCount > 0
            ? () => healthIssuesRef.current?.scrollIntoView({ behavior: 'smooth', block: 'start' })
            : undefined
        }
      />
    );
  };

  return (
    <div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--profiles">
      {summary !== null && !healthLoading && summary.broken_count > 0 && !healthBannerDismissed ? (
        <div className="crosshook-rename-toast" role="status" aria-live="polite">
          <span>
            {summary.broken_count} profile{summary.broken_count !== 1 ? 's' : ''} have issues that may prevent launching
          </span>
          <button
            type="button"
            className="crosshook-rename-toast-dismiss"
            onClick={dismissHealthBanner}
            aria-label="Dismiss"
          >
            &times;
          </button>
        </div>
      ) : null}

      <div className="crosshook-route-stack crosshook-profiles-page">
        <div className="crosshook-route-stack__body--fill crosshook-profiles-page__body">
          <RouteBanner route="profiles" />
          <div className="crosshook-panel crosshook-profiles-hero-outer">
            <section className="crosshook-launch-panel crosshook-route-hero-launch-panel">
              <div className="crosshook-launch-panel__profile-row">
                <label
                  className="crosshook-label"
                  htmlFor="profile-selector-top"
                  style={{ margin: 0, whiteSpace: 'nowrap' }}
                >
                  Active Profile
                </label>
                <div className="crosshook-launch-panel__profile-row-select">
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
                    id="profile-selector-top"
                    value={selectedProfile}
                    onValueChange={(val) => void selectProfile(val)}
                    placeholder="Create New"
                    options={[
                      { value: '', label: 'Create New' },
                      ...filteredProfiles.map((name) => ({
                        value: name,
                        label: name,
                        badge: showNetworkIsolationBadge(name) ? NETWORK_ISOLATION_BADGE : undefined,
                        badgeTitle: showNetworkIsolationBadge(name) ? NETWORK_ISOLATION_BADGE_TITLE : undefined,
                      })),
                    ]}
                  />
                </div>
                <div className="crosshook-launch-panel__profile-row-actions">
                  <button
                    type="button"
                    className="crosshook-button crosshook-launch-panel__action"
                    style={LAUNCH_PANEL_ACTION_BUTTON_STYLE}
                    onClick={() => {
                      setWizardMode('create');
                      setShowWizard(true);
                    }}
                  >
                    New Profile
                  </button>
                  {hasSelectedProfile ? (
                    <button
                      type="button"
                      className="crosshook-button crosshook-button--secondary crosshook-launch-panel__action crosshook-launch-panel__action--secondary"
                      style={LAUNCH_PANEL_ACTION_BUTTON_STYLE}
                      onClick={() => {
                        setWizardMode('edit');
                        setShowWizard(true);
                      }}
                    >
                      Edit in Wizard
                    </button>
                  ) : null}
                </div>
              </div>

              <div className="crosshook-profiles-hero-status">
                {renderProfileHealthBadge()}
                {renderOfflineStatusBadge()}
                {launchMethod !== 'native' && profile.trainer.path.trim().length > 0 ? (
                  <span className="crosshook-status-chip" title="Trainer type catalog id for offline scoring">
                    Trainer type: {trainerTypeDisplayName}
                  </span>
                ) : null}
                {renderVersionStatusBadge()}
                {showNetworkIsolationBadge(selectedProfile) ? (
                  <span
                    className="crosshook-status-chip crosshook-version-badge crosshook-version-badge--warning"
                    title={NETWORK_ISOLATION_BADGE_TITLE}
                  >
                    {NETWORK_ISOLATION_BADGE}
                  </span>
                ) : null}
                {summary !== null && summary.stale_count + summary.broken_count > 0 ? (
                  <span className="crosshook-status-chip">
                    {summary.stale_count + summary.broken_count} of {summary.total_count} profile
                    {summary.total_count !== 1 ? 's' : ''} have issues
                  </span>
                ) : null}
                {!selectedReport && selectedStaleInfo?.isStale ? (
                  <span className="crosshook-status-chip crosshook-status-chip--muted" role="note">
                    Checked {selectedStaleInfo.daysAgo}d ago
                  </span>
                ) : null}
                <div className="crosshook-profiles-hero-status__action">
                  <button
                    type="button"
                    className="crosshook-button crosshook-button--secondary crosshook-launch-panel__action crosshook-launch-panel__action--secondary"
                    style={LAUNCH_PANEL_ACTION_BUTTON_STYLE}
                    onClick={async (event) => {
                      event.preventDefault();
                      await refreshProfiles();
                      await batchValidate();
                    }}
                  >
                    {summary !== null && summary.stale_count + summary.broken_count > 0
                      ? healthLoading
                        ? 'Checking...'
                        : 'Re-check'
                      : 'Refresh'}
                  </button>
                </div>
              </div>
            </section>
          </div>

          {/* Health Issues card — shown when selected profile has broken/stale health */}
          {(() => {
            const report = selectedReport;
            if (!report || (report.status !== 'broken' && report.status !== 'stale') || report.issues.length === 0) {
              return null;
            }

            const metadata = report.metadata ?? null;

            const driftMessage: Record<string, string> = {
              missing: 'Exported launcher not found — re-export recommended',
              moved: 'Exported launcher has moved — re-export recommended',
              stale: 'Exported launcher may be outdated — re-export recommended',
            };
            const driftWarning =
              metadata !== null && metadata.launcher_drift_state !== null
                ? (driftMessage[metadata.launcher_drift_state] ?? null)
                : null;

            return (
              <div ref={healthIssuesRef}>
                <CollapsibleSection title="Health Issues" className="crosshook-panel">
                  {metadata !== null ? (
                    <div style={{ marginBottom: 10, display: 'grid', gap: 4 }}>
                      {metadata.last_success !== null ? (
                        <p className="crosshook-help-text" style={{ margin: 0 }}>
                          Last worked: {formatRelativeTime(metadata.last_success)}
                        </p>
                      ) : null}
                      {metadata.total_launches > 0 ? (
                        <p className="crosshook-help-text" style={{ margin: 0 }}>
                          Launched {metadata.total_launches} time{metadata.total_launches !== 1 ? 's' : ''} &bull;{' '}
                          {metadata.failure_count_30d} failure{metadata.failure_count_30d !== 1 ? 's' : ''} in last 30
                          days
                        </p>
                      ) : null}
                      {driftWarning !== null ? (
                        <p className="crosshook-danger" style={{ margin: 0 }} role="alert">
                          {driftWarning}
                        </p>
                      ) : null}
                      {metadata.is_community_import && (report.status === 'broken' || report.status === 'stale') ? (
                        <p className="crosshook-help-text" style={{ margin: 0 }}>
                          This profile was imported from a community tap — paths may need adjustment for your system.
                        </p>
                      ) : null}
                    </div>
                  ) : null}
                  <ul style={{ margin: 0, padding: 0, listStyle: 'none', display: 'grid', gap: 8 }}>
                    {report.issues.map((issue, index) => (
                      <li
                        key={index}
                        style={{ borderLeft: '3px solid var(--crosshook-danger, #ef4444)', paddingLeft: 10 }}
                      >
                        <strong>{issue.field}</strong>
                        {issue.path ? <span className="crosshook-muted"> — {issue.path}</span> : null}
                        <p style={{ margin: '2px 0' }}>{issue.message}</p>
                        {issue.remediation ? (
                          <p className="crosshook-help-text" style={{ margin: '2px 0' }}>
                            {issue.remediation}
                          </p>
                        ) : null}
                      </li>
                    ))}
                  </ul>
                </CollapsibleSection>
              </div>
            );
          })()}

          {/* Prefix Dependencies — shown when the selected profile has required_protontricks */}
          {profile.trainer?.required_protontricks && profile.trainer.required_protontricks.length > 0 ? (
            <CollapsibleSection title="Prefix Dependencies" className="crosshook-panel">
              <PrefixDepsPanel
                profileName={profileName}
                prefixPath={profile.runtime?.prefix_path ?? profile.steam?.compatdata_path ?? ''}
                requiredPackages={profile.trainer.required_protontricks}
              />
            </CollapsibleSection>
          ) : null}

          {/* ProtonUp recommendation — advisory only, never blocks launch */}
          {suggestion !== null && suggestion.status === 'missing' && !suggestionDismissed ? (
            <div className="crosshook-panel crosshook-protonup-recommendation" role="status">
              <div className="crosshook-protonup-recommendation__content">
                <span className="crosshook-protonup-recommendation__icon" aria-hidden="true">
                  &#9888;
                </span>
                <div className="crosshook-protonup-recommendation__text">
                  <strong>Runtime suggestion</strong>
                  <p className="crosshook-help-text" style={{ margin: '4px 0 0' }}>
                    This community profile recommends <strong>{suggestion.community_version}</strong>, which is not
                    currently installed. You can still launch with your current runtime.
                  </p>
                </div>
              </div>
              <div
                className="crosshook-protonup-recommendation__actions"
                style={{ display: 'flex', gap: 8, flexWrap: 'wrap', marginTop: 10 }}
              >
                <button
                  type="button"
                  className="crosshook-button crosshook-button--small crosshook-button--primary"
                  onClick={() => {
                    if (!suggestion.recommended_version) {
                      return;
                    }
                    const targetRoot = effectiveSteamClientInstallPath
                      ? `${effectiveSteamClientInstallPath}/compatibilitytools.d`
                      : '';
                    setSuggestionInstallError(null);
                    void (async () => {
                      const provider = await resolveProtonUpProviderForVersion(suggestion.recommended_version!);
                      return protonUp.installVersion({
                        provider,
                        version: suggestion.recommended_version!,
                        target_root: targetRoot,
                      });
                    })().then((result) => {
                      if (!result.success) {
                        setSuggestionInstallError(result.error_message ?? result.error_kind ?? 'Install failed');
                        return;
                      }
                      void callCommand<ProtonInstallOption[]>('list_proton_installs', {
                        steamClientInstallPath:
                          effectiveSteamClientInstallPath.trim().length > 0
                            ? effectiveSteamClientInstallPath
                            : undefined,
                      }).then((installs) => {
                        setProtonInstalls(sortProtonInstalls(installs));
                      });
                      setSuggestionDismissed(true);
                    });
                  }}
                  disabled={protonUp.installing || !suggestion.recommended_version || !effectiveSteamClientInstallPath}
                >
                  {protonUp.installing ? 'Installing\u2026' : 'Install recommended'}
                </button>
                <button
                  type="button"
                  className="crosshook-button crosshook-button--small crosshook-button--ghost"
                  onClick={() => setSuggestionDismissed(true)}
                >
                  Dismiss
                </button>
              </div>
              {suggestionInstallError ? (
                <p className="crosshook-danger" role="alert" style={{ margin: '8px 0 0' }}>
                  {suggestionInstallError}
                </p>
              ) : null}
            </div>
          ) : null}

          {/* Profile sub-tabs — stable height; scroll inside active tab */}
          <div className="crosshook-profiles-editor-host">
            <div className="crosshook-panel crosshook-subtabs-shell crosshook-profiles-subtabs">
              <ProfileSubTabs
                profile={profile}
                profileName={profileName}
                profileExists={profileExists}
                profiles={profiles}
                launchMethod={launchMethod}
                protonInstalls={protonInstalls}
                protonInstallsError={protonInstallsError}
                onUpdateProfile={updateProfile}
                onProfileNameChange={setProfileName}
                trainerVersion={selectedTrainerVersion}
                onVersionSet={() => {
                  if (selectedProfile) void revalidateSingle(selectedProfile);
                }}
                steamClientInstallPath={effectiveSteamClientInstallPath}
                targetHomePath={targetHomePath}
                pendingReExport={pendingLauncherReExport}
                onReExportHandled={() => setPendingLauncherReExport(false)}
              />
            </div>
          </div>
        </div>

        <div className="crosshook-profiles-page__actions crosshook-route-footer crosshook-panel">
          <ProfileActions
            layoutVariant="footer"
            dirty={dirty}
            loading={loading}
            saving={saving}
            deleting={deleting}
            duplicating={duplicating}
            renaming={renaming}
            error={error}
            canSave={canSave}
            canDelete={canDelete}
            canDuplicate={canDuplicate}
            canRename={canRename}
            canPreview={canPreview}
            previewing={previewing}
            canExportCommunity={canExportCommunity}
            exportingCommunity={exportingCommunity}
            canViewHistory={canViewHistory}
            onSave={handleSave}
            onDelete={() => confirmDelete(profileName)}
            onDuplicate={() => duplicateProfile(profileName)}
            onRename={() => {
              setPendingRename(selectedProfile);
              setRenameValue(selectedProfile);
            }}
            onPreview={handlePreviewProfile}
            onExportCommunity={handleExportCommunityProfile}
            onViewHistory={() => setShowHistoryPanel(true)}
          />
          {previewError ? (
            <p className="crosshook-danger" role="alert" style={{ marginTop: 12 }}>
              Preview failed: {previewError}
            </p>
          ) : null}
          {communityExportError ? (
            <p className="crosshook-danger" role="alert" style={{ marginTop: 12 }}>
              Community export failed: {communityExportError}
            </p>
          ) : null}
          {communityExportSuccess ? (
            <p className="crosshook-help-text" role="status" style={{ marginTop: 12 }}>
              {communityExportSuccess}
            </p>
          ) : null}
        </div>
      </div>

      {pendingDelete ? (
        <div className="crosshook-profile-editor-delete-overlay" data-crosshook-focus-root="modal">
          <div className="crosshook-profile-editor-delete-dialog">
            <h3 style={{ margin: '0 0 12px' }}>Delete Profile</h3>
            <p>
              Delete profile <strong>{pendingDelete.name}</strong>?
            </p>
            {pendingDelete.launcherInfo ? (
              <div className="crosshook-profile-editor-delete-warning">
                <p style={{ margin: '0 0 6px', fontWeight: 600 }}>Launcher files will also be removed:</p>
                <p style={{ margin: '2px 0', color: '#d1d5db', wordBreak: 'break-all' }}>
                  {pendingDelete.launcherInfo.script_path}
                </p>
                <p style={{ margin: '2px 0', color: '#d1d5db', wordBreak: 'break-all' }}>
                  {pendingDelete.launcherInfo.desktop_entry_path}
                </p>
              </div>
            ) : null}
            <div className="crosshook-profile-editor-delete-actions">
              <button
                type="button"
                className="crosshook-button crosshook-button--secondary"
                onClick={cancelDelete}
                data-crosshook-modal-close
              >
                Cancel
              </button>
              <button
                type="button"
                className="crosshook-profile-editor-delete-confirm"
                onClick={() => void executeDelete()}
              >
                {pendingDelete.launcherInfo ? 'Delete Profile and Launcher' : 'Delete Profile'}
              </button>
            </div>
          </div>
        </div>
      ) : null}

      {pendingRename !== null ? (
        <div className="crosshook-profile-editor-delete-overlay" data-crosshook-focus-root="modal">
          <div
            className="crosshook-profile-editor-delete-dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="rename-dialog-heading"
            style={{ marginBottom: 'auto', marginTop: '12vh' }}
          >
            <h3 id="rename-dialog-heading" style={{ margin: '0 0 12px' }}>
              Rename Profile
            </h3>
            <div className="crosshook-field">
              <label className="crosshook-label" htmlFor="rename-profile-input">
                New Name
              </label>
              <input
                id="rename-profile-input"
                ref={renameInputRef}
                className="crosshook-input"
                value={renameValue}
                onChange={(event) => setRenameValue(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter' && canConfirmRename) {
                    const oldName = pendingRename;
                    const newName = renameNameTrimmed;
                    handleRenameConfirm(oldName, newName);
                  }

                  if (event.key === 'Escape') {
                    setPendingRename(null);
                  }
                }}
              />
              {renameError ? (
                <p className="crosshook-danger" role="alert">
                  {renameError}
                </p>
              ) : null}
            </div>
            <div className="crosshook-profile-editor-delete-actions">
              <button
                type="button"
                className="crosshook-button crosshook-button--secondary"
                onClick={() => setPendingRename(null)}
                data-crosshook-modal-close
              >
                Cancel
              </button>
              <button
                type="button"
                className="crosshook-button"
                disabled={!canConfirmRename}
                onClick={() => {
                  const oldName = pendingRename;
                  const newName = renameNameTrimmed;
                  handleRenameConfirm(oldName, newName);
                }}
              >
                {renaming ? 'Renaming...' : 'Rename'}
              </button>
            </div>
          </div>
        </div>
      ) : null}

      {renameToast && !renameToastDismissed ? (
        <div className="crosshook-rename-toast" role="status" aria-live="polite">
          <span>Renamed to &lsquo;{renameToast.newName}&rsquo;</span>
          <button type="button" className="crosshook-button crosshook-button--ghost" onClick={undoRename}>
            Undo
          </button>
          <button
            type="button"
            className="crosshook-rename-toast-dismiss"
            onClick={dismissRenameToast}
            aria-label="Dismiss"
          >
            &times;
          </button>
        </div>
      ) : null}

      {showProfilePreview ? (
        <ProfilePreviewModal
          tomlContent={profilePreviewContent}
          profileName={profileName}
          onClose={handleCloseProfilePreview}
        />
      ) : null}

      {showHistoryPanel && selectedProfile ? (
        <ConfigHistoryPanel
          profileName={selectedProfile}
          onClose={() => setShowHistoryPanel(false)}
          fetchConfigHistory={fetchConfigHistory}
          fetchConfigDiff={fetchConfigDiff}
          rollbackConfig={rollbackConfig}
          markKnownGood={markKnownGood}
          onAfterRollback={handleAfterRollback}
        />
      ) : null}

      {showWizard ? (
        <OnboardingWizard
          open={showWizard}
          mode={wizardMode}
          onComplete={() => setShowWizard(false)}
          onDismiss={() => setShowWizard(false)}
        />
      ) : null}
    </div>
  );
}

export default ProfilesPage;
