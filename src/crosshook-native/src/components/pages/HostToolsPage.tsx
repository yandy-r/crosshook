import { useMemo, useState } from 'react';
import { callCommand } from '@/lib/ipc';
import { useHostReadinessContext } from '../../context/HostReadinessContext';
import type { HostToolCheckResult } from '../../types/onboarding';
import CapabilityTilesSection from '../host-readiness/CapabilityTilesSection';
import HostDelegationBanner from '../host-readiness/HostDelegationBanner';
import type { HostToolAvailabilityFilter, HostToolCategoryFilter } from '../host-readiness/HostToolFilterBar';
import HostToolInventory from '../host-readiness/HostToolInventory';
import HostToolMetricsHero from '../host-readiness/HostToolMetricsHero';
import HostToolStatusToolbar from '../host-readiness/HostToolStatusToolbar';
import { DashboardPanelSection } from '../layout/DashboardPanelSection';
import { RouteBanner } from '../layout/RouteBanner';

import '../../styles/host-tool-dashboard.css';

function matchesCategory(tool: HostToolCheckResult, filter: HostToolCategoryFilter): boolean {
  if (filter === 'all') return true;
  return tool.category.trim().toLowerCase() === filter;
}

function matchesAvailability(tool: HostToolCheckResult, filter: HostToolAvailabilityFilter): boolean {
  switch (filter) {
    case 'all':
      return true;
    case 'available':
      return tool.is_available;
    case 'missing':
      return !tool.is_available;
    case 'required_missing':
      return tool.is_required && !tool.is_available;
    default:
      return true;
  }
}

function matchesSearch(tool: HostToolCheckResult, normalizedQuery: string): boolean {
  if (normalizedQuery.length === 0) return true;
  const haystack = [
    tool.display_name,
    tool.tool_id,
    tool.category,
    tool.docs_url ?? '',
    tool.tool_version ?? '',
    tool.resolved_path ?? '',
    tool.install_guidance?.command ?? '',
    tool.install_guidance?.alternatives ?? '',
  ];
  return haystack.some((value) => value.toLowerCase().includes(normalizedQuery));
}

export function HostToolsPage() {
  const { snapshot, capabilities, isStale, lastCheckedAt, isRefreshing, error, refresh, probeTool } =
    useHostReadinessContext();

  const [categoryFilter, setCategoryFilter] = useState<HostToolCategoryFilter>('all');
  const [availabilityFilter, setAvailabilityFilter] = useState<HostToolAvailabilityFilter>('all');
  const [searchQuery, setSearchQuery] = useState('');
  const [probingToolId, setProbingToolId] = useState<string | null>(null);
  const [dismissError, setDismissError] = useState<string | null>(null);

  const toolChecks = snapshot?.tool_checks ?? [];
  const normalizedSearchQuery = searchQuery.trim().toLowerCase();

  const filteredTools = useMemo(
    () =>
      toolChecks.filter(
        (tool) =>
          matchesCategory(tool, categoryFilter) &&
          matchesAvailability(tool, availabilityFilter) &&
          matchesSearch(tool, normalizedSearchQuery)
      ),
    [availabilityFilter, categoryFilter, normalizedSearchQuery, toolChecks]
  );

  const [requiredTools, optionalTools] = useMemo(() => {
    const required: HostToolCheckResult[] = [];
    const optional: HostToolCheckResult[] = [];
    for (const tool of filteredTools) {
      if (tool.is_required) {
        required.push(tool);
      } else {
        optional.push(tool);
      }
    }
    return [required, optional] as const;
  }, [filteredTools]);

  const hasSnapshot = snapshot != null;
  const hasToolChecks = toolChecks.length > 0;
  const hasFilteredTools = filteredTools.length > 0;
  const initialLoading = !hasSnapshot && isRefreshing && error == null;
  const initialError = !hasSnapshot && error != null;
  const shouldShowCapabilitiesSection = initialLoading || capabilities.length > 0;

  const handleRefresh = () => {
    setDismissError(null);
    void refresh().catch(() => {
      // useHostReadiness exposes the latest refresh error on its `error` field.
    });
  };

  const handleProbeDetails = async (toolId: string) => {
    setProbingToolId(toolId);
    try {
      await probeTool(toolId);
    } finally {
      setProbingToolId((current) => (current === toolId ? null : current));
    }
  };

  const handleDismissReadinessNag = async (toolId: string) => {
    setDismissError(null);
    try {
      await callCommand<void>('dismiss_readiness_nag', { toolId });
      await refresh();
    } catch (nextError) {
      setDismissError(nextError instanceof Error ? nextError.message : String(nextError));
    }
  };

  return (
    <div
      className="crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--host-tools"
      aria-busy={isRefreshing}
    >
      <div className="crosshook-route-stack" data-crosshook-focus-zone="content">
        <div className="crosshook-route-stack__body--scroll">
          <RouteBanner route="host-tools" />
          <div className="crosshook-host-tool-dashboard">
            {error != null && hasSnapshot ? (
              <div role="alert" className="crosshook-host-tool-dashboard-error crosshook-panel">
                <p>Host readiness refresh failed: {error}</p>
                <button type="button" className="crosshook-button" onClick={handleRefresh}>
                  Retry
                </button>
              </div>
            ) : null}

            {dismissError != null ? (
              <div role="alert" className="crosshook-host-tool-dashboard-error crosshook-panel">
                <p>{dismissError}</p>
              </div>
            ) : null}

            <DashboardPanelSection
              eyebrow="Host readiness"
              title="Check runtime coverage before you launch"
              description="CrossHook inspects required tools and optional capabilities on the host so you can spot missing launch dependencies early."
              className="crosshook-host-tool-dashboard-panel crosshook-host-tool-dashboard-panel--hero"
              bodyClassName="crosshook-host-tool-dashboard-panel__body--hero"
            >
              <HostDelegationBanner />
              <HostToolMetricsHero toolChecks={toolChecks} capabilities={capabilities} loading={initialLoading} />
            </DashboardPanelSection>

            {shouldShowCapabilitiesSection ? (
              <DashboardPanelSection
                className="crosshook-host-tool-dashboard-panel"
                eyebrow="Capability coverage"
                title="Optional integrations"
                description="These capability probes summarize what extra launch and tooling features are currently available on the host."
              >
                <CapabilityTilesSection capabilities={capabilities} loading={initialLoading} />
              </DashboardPanelSection>
            ) : null}

            <DashboardPanelSection
              className="crosshook-host-tool-dashboard-panel"
              eyebrow="Tool inventory"
              title="Required and optional host tools"
              description="Refresh detection, filter the current snapshot, and inspect individual tools without changing the underlying readiness flow."
            >
              {initialError ? (
                <section className="crosshook-host-tool-dashboard__empty-state crosshook-panel" role="alert">
                  <h2 className="crosshook-host-tool-dashboard__card-title">Unable to load host readiness</h2>
                  <p className="crosshook-host-tool-dashboard__card-summary">{error}</p>
                  <button
                    type="button"
                    className="crosshook-button crosshook-button--secondary"
                    onClick={handleRefresh}
                  >
                    Try again
                  </button>
                </section>
              ) : (
                <>
                  <HostToolStatusToolbar
                    lastCheckedAt={lastCheckedAt}
                    isStale={isStale}
                    isRefreshing={isRefreshing}
                    shownCount={filteredTools.length}
                    totalCount={toolChecks.length}
                    detectedDistroFamily={snapshot?.detected_distro_family ?? ''}
                    onRefresh={handleRefresh}
                    categoryFilter={categoryFilter}
                    availabilityFilter={availabilityFilter}
                    searchQuery={searchQuery}
                    onCategoryFilterChange={setCategoryFilter}
                    onAvailabilityFilterChange={setAvailabilityFilter}
                    onSearchQueryChange={setSearchQuery}
                    filtersDisabled={isRefreshing && !hasToolChecks}
                  />

                  {initialLoading ? (
                    <section className="crosshook-host-tool-dashboard__empty-state" aria-live="polite">
                      <h2 className="crosshook-host-tool-dashboard__card-title">Checking host tools…</h2>
                      <p className="crosshook-host-tool-dashboard__card-summary">
                        CrossHook is probing the host system for required and optional tool availability.
                      </p>
                    </section>
                  ) : null}

                  {!initialLoading && !hasToolChecks ? (
                    <section className="crosshook-host-tool-dashboard__empty-state">
                      <h2 className="crosshook-host-tool-dashboard__card-title">No host tool data available</h2>
                      <p className="crosshook-host-tool-dashboard__card-summary">
                        CrossHook did not return any host tool checks for this snapshot yet. Try refreshing.
                      </p>
                    </section>
                  ) : null}

                  {hasToolChecks && !hasFilteredTools ? (
                    <section className="crosshook-host-tool-dashboard__empty-state">
                      <h2 className="crosshook-host-tool-dashboard__card-title">No tools match these filters</h2>
                      <p className="crosshook-host-tool-dashboard__card-summary">
                        Clear or broaden the category, availability, or search filters to see more host tools.
                      </p>
                    </section>
                  ) : null}

                  {hasFilteredTools ? (
                    <HostToolInventory
                      requiredTools={requiredTools}
                      optionalTools={optionalTools}
                      probingToolId={probingToolId}
                      onProbeDetails={handleProbeDetails}
                      onDismissReadinessNag={(toolId) => void handleDismissReadinessNag(toolId)}
                    />
                  ) : null}
                </>
              )}
            </DashboardPanelSection>
          </div>
        </div>
      </div>
    </div>
  );
}

export default HostToolsPage;
