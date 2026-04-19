import { HealthBadge } from '../HealthBadge';
import { OfflineStatusBadge } from '../OfflineStatusBadge';
import { PrefixDepsPanel } from '../PrefixDepsPanel';
import ProfileActions from '../ProfileActions';
import ProfileSubTabs from '../ProfileSubTabs';
import { CollapsibleSection } from '../ui/CollapsibleSection';
import { NETWORK_ISOLATION_BADGE, NETWORK_ISOLATION_BADGE_TITLE, VERSION_STATUS_LABELS } from './profiles/constants';
import { ProfilesHealthIssues } from './profiles/ProfilesHealthIssues';
import { ProfilesHero } from './profiles/ProfilesHero';
import { ProfilesOverlays } from './profiles/ProfilesOverlays';
import { useProfilesPageState } from './profiles/useProfilesPageState';

export function ProfilesPage() {
  const state = useProfilesPageState();

  const renderVersionStatusBadge = () => {
    const status = state.selectedVersionStatus;
    if (!status || status === 'untracked' || status === 'unknown' || status === 'matched') {
      return null;
    }

    const isWarning = status === 'game_updated' || status === 'trainer_changed' || status === 'both_changed';

    return (
      <span
        className={`crosshook-status-chip crosshook-version-badge crosshook-version-badge--${isWarning ? 'warning' : 'info'}`}
        title={
          isWarning ? 'Version mismatch detected since last successful launch' : 'Steam is currently updating this game'
        }
      >
        {VERSION_STATUS_LABELS[status] ?? status}
      </span>
    );
  };

  const renderOfflineStatusBadge = () => {
    if (!state.selectedProfile) {
      return null;
    }
    return <OfflineStatusBadge report={state.selectedOfflineReport ?? undefined} />;
  };

  const renderProfileHealthBadge = () => {
    if (!state.selectedProfile) {
      return null;
    }

    if (!state.selectedReport && !state.selectedCachedSnapshot) {
      return null;
    }

    if (state.selectedReport) {
      const issueCount = state.selectedReport.issues.length;
      const issueTooltip =
        issueCount > 0
          ? `${issueCount} issue${issueCount !== 1 ? 's' : ''}: ${state.selectedReport.issues
              .slice(0, 3)
              .map((issue) => `${issue.field} — ${issue.message}`)
              .join('; ')}${issueCount > 3 ? ` (+${issueCount - 3} more)` : ''}`
          : null;

      return (
        <HealthBadge
          report={state.selectedReport}
          metadata={state.selectedReport.metadata}
          trend={state.selectedTrend}
          tooltip={issueTooltip}
          onClick={
            issueCount > 0
              ? () => state.healthIssuesRef.current?.scrollIntoView({ behavior: 'smooth', block: 'start' })
              : undefined
          }
        />
      );
    }

    const cachedSnapshot = state.selectedCachedSnapshot;
    const badgeStatus = cachedSnapshot?.status;
    if (!cachedSnapshot || !badgeStatus) {
      return null;
    }

    const issueCount = cachedSnapshot.issue_count;
    const issueTooltip = issueCount > 0 ? `${issueCount} issue${issueCount !== 1 ? 's' : ''} in cached snapshot` : null;

    return <HealthBadge status={badgeStatus} trend={state.selectedTrend} tooltip={issueTooltip} />;
  };

  return (
    <div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--profiles">
      <div className="crosshook-route-stack crosshook-profiles-page">
        <div className="crosshook-route-stack__body--fill crosshook-profiles-page__body">
          <ProfilesHero
            activeCollectionName={state.activeCollection?.name ?? null}
            filteredProfiles={state.filteredProfiles}
            hasSelectedProfile={state.hasSelectedProfile}
            healthBannerDismissed={state.healthBannerDismissed}
            healthLoading={state.healthLoading}
            selectedProfile={state.selectedProfile}
            summary={state.summary}
            onClearCollectionFilter={() => state.setActiveCollectionId(null)}
            onDismissHealthBanner={state.dismissHealthBanner}
            onOpenEditWizard={() => state.openWizard('edit')}
            onOpenNewWizard={() => state.openWizard('create')}
            onRefreshStatus={state.handleRefreshStatus}
            onSelectProfile={(value) => void state.selectProfile(value)}
            optionBadgeForProfile={(name) => ({
              badge: state.showNetworkIsolationBadge(name) ? NETWORK_ISOLATION_BADGE : undefined,
              badgeTitle: state.showNetworkIsolationBadge(name) ? NETWORK_ISOLATION_BADGE_TITLE : undefined,
            })}
            statusBadges={
              <>
                {renderProfileHealthBadge()}
                {renderOfflineStatusBadge()}
                {state.launchMethod !== 'native' && state.profile.trainer.path.trim().length > 0 ? (
                  <span className="crosshook-status-chip" title="Trainer type catalog id for offline scoring">
                    Trainer type: {state.trainerTypeDisplayName}
                  </span>
                ) : null}
                {renderVersionStatusBadge()}
                {state.showNetworkIsolationBadge(state.selectedProfile) ? (
                  <span
                    className="crosshook-status-chip crosshook-version-badge crosshook-version-badge--warning"
                    title={NETWORK_ISOLATION_BADGE_TITLE}
                  >
                    {NETWORK_ISOLATION_BADGE}
                  </span>
                ) : null}
                {state.summary !== null && state.summary.stale_count + state.summary.broken_count > 0 ? (
                  <span className="crosshook-status-chip">
                    {state.summary.stale_count + state.summary.broken_count} of {state.summary.total_count} profile
                    {state.summary.total_count !== 1 ? 's' : ''} have issues
                  </span>
                ) : null}
                {!state.selectedReport && state.selectedStaleInfo?.isStale ? (
                  <span className="crosshook-status-chip crosshook-status-chip--muted" role="note">
                    Checked {state.selectedStaleInfo.daysAgo}d ago
                  </span>
                ) : null}
              </>
            }
          />

          <ProfilesHealthIssues
            healthIssuesRef={state.healthIssuesRef as React.RefObject<HTMLDivElement>}
            report={state.selectedReport}
          />

          {state.profile.trainer?.required_protontricks && state.profile.trainer.required_protontricks.length > 0 ? (
            <CollapsibleSection title="Prefix Dependencies" className="crosshook-panel">
              <PrefixDepsPanel
                profileName={state.profileName}
                prefixPath={state.profile.runtime?.prefix_path ?? state.profile.steam?.compatdata_path ?? ''}
                requiredPackages={state.profile.trainer.required_protontricks}
              />
            </CollapsibleSection>
          ) : null}

          {state.suggestion !== null && state.suggestion.status === 'missing' && !state.suggestionDismissed ? (
            <div className="crosshook-panel crosshook-protonup-recommendation" role="status">
              <div className="crosshook-protonup-recommendation__content">
                <span className="crosshook-protonup-recommendation__icon" aria-hidden="true">
                  &#9888;
                </span>
                <div className="crosshook-protonup-recommendation__text">
                  <strong>Runtime suggestion</strong>
                  <p className="crosshook-help-text" style={{ margin: '4px 0 0' }}>
                    This community profile recommends <strong>{state.suggestion.community_version}</strong>, which is
                    not currently installed. You can still launch with your current runtime.
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
                  onClick={() => void state.handleInstallSuggestedVersion()}
                  disabled={
                    state.protonUp.installing ||
                    !state.suggestion.recommended_version ||
                    !state.effectiveSteamClientInstallPath
                  }
                >
                  {state.protonUp.installing ? 'Installing…' : 'Install recommended'}
                </button>
                <button
                  type="button"
                  className="crosshook-button crosshook-button--small crosshook-button--ghost"
                  onClick={() => state.setSuggestionDismissed(true)}
                >
                  Dismiss
                </button>
              </div>
              {state.suggestionInstallError ? (
                <p className="crosshook-danger" role="alert" style={{ margin: '8px 0 0' }}>
                  {state.suggestionInstallError}
                </p>
              ) : null}
            </div>
          ) : null}

          <div className="crosshook-profiles-editor-host">
            <div className="crosshook-panel crosshook-subtabs-shell crosshook-profiles-subtabs">
              <ProfileSubTabs
                profile={state.profile}
                profileName={state.profileName}
                profileExists={state.profileExists}
                profiles={state.profiles}
                launchMethod={state.launchMethod}
                protonInstalls={state.protonInstalls}
                protonInstallsError={state.protonInstallsError}
                onUpdateProfile={state.updateProfile}
                onProfileNameChange={state.setProfileName}
                trainerVersion={state.selectedTrainerVersion}
                onVersionSet={() => {
                  if (state.selectedProfile) {
                    void state.revalidateSingle(state.selectedProfile);
                  }
                }}
                steamClientInstallPath={state.effectiveSteamClientInstallPath}
                targetHomePath={state.targetHomePath}
                pendingReExport={state.pendingLauncherReExport}
                onReExportHandled={() => state.setPendingLauncherReExport(false)}
              />
            </div>
          </div>
        </div>

        <div className="crosshook-profiles-page__actions crosshook-route-footer crosshook-panel">
          <ProfileActions
            layoutVariant="footer"
            dirty={state.dirty}
            loading={state.loading}
            saving={state.saving}
            deleting={state.deleting}
            duplicating={state.duplicating}
            renaming={state.renaming}
            error={state.error}
            canSave={state.canSave}
            canDelete={state.canDelete}
            canDuplicate={state.canDuplicate}
            canRename={state.canRename}
            canPreview={state.canPreview}
            previewing={state.previewing}
            canExportCommunity={state.canExportCommunity}
            exportingCommunity={state.exportingCommunity}
            canViewHistory={state.canViewHistory}
            onSave={state.handleSave}
            onDelete={() => state.confirmDelete(state.profileName)}
            onDuplicate={() => state.duplicateProfile(state.profileName)}
            onRename={() => {
              state.setPendingRename(state.selectedProfile);
              state.setRenameValue(state.selectedProfile);
            }}
            onPreview={state.handlePreviewProfile}
            onExportCommunity={state.handleExportCommunityProfile}
            onViewHistory={() => state.setShowHistoryPanel(true)}
          />
          {state.previewError ? (
            <p className="crosshook-danger" role="alert" style={{ marginTop: 12 }}>
              Preview failed: {state.previewError}
            </p>
          ) : null}
          {state.communityExportError ? (
            <p className="crosshook-danger" role="alert" style={{ marginTop: 12 }}>
              Community export failed: {state.communityExportError}
            </p>
          ) : null}
          {state.communityExportSuccess ? (
            <p className="crosshook-help-text" role="status" style={{ marginTop: 12 }}>
              {state.communityExportSuccess}
            </p>
          ) : null}
        </div>
      </div>

      <ProfilesOverlays
        canConfirmRename={state.canConfirmRename}
        pendingDelete={state.pendingDelete}
        pendingRename={state.pendingRename}
        previewContent={state.profilePreviewContent}
        profileName={state.profileName}
        renameError={state.renameError}
        renameInputRef={state.renameInputRef as React.RefObject<HTMLInputElement>}
        renameNameTrimmed={state.renameNameTrimmed}
        renameToast={state.renameToast}
        renameToastDismissed={state.renameToastDismissed}
        renameValue={state.renameValue}
        renaming={state.renaming}
        selectedProfile={state.selectedProfile}
        showHistoryPanel={state.showHistoryPanel}
        showProfilePreview={state.showProfilePreview}
        showWizard={state.showWizard}
        wizardMode={state.wizardMode}
        onAfterRollback={state.handleAfterRollback}
        onCancelDelete={state.cancelDelete}
        onCloseHistory={() => state.setShowHistoryPanel(false)}
        onClosePreview={state.handleCloseProfilePreview}
        onConfirmRename={state.handleRenameConfirm}
        onDismissRenameToast={state.dismissRenameToast}
        onExecuteDelete={state.executeDelete}
        rollbackConfig={state.rollbackConfig}
        onSetPendingRename={state.setPendingRename}
        onSetRenameValue={state.setRenameValue}
        onToggleWizard={state.setShowWizard}
        onUndoRename={state.undoRename}
        fetchConfigDiff={state.fetchConfigDiff}
        fetchConfigHistory={state.fetchConfigHistory}
        markKnownGood={state.markKnownGood}
      />
    </div>
  );
}

export default ProfilesPage;
