import type { MutableRefObject } from 'react';
import { useDeferredValue, useEffect, useMemo, useRef, useState } from 'react';
import { useOfflineReadiness } from '../../../hooks/useOfflineReadiness';
import type { CachedHealthSnapshot, EnrichedHealthSummary } from '../../../types';
import type { EnrichedProfileHealthReport } from '../../../types/health';
import type { CardTrend, SortDirection, SortField, StatusFilter } from './constants';
import { STATUS_RANK, VERSION_STATUS_RANK } from './constants';
import { categorizeIssue, offlineSortScore } from './utils';

export interface HealthDashboardStateReturn {
  sortField: SortField;
  sortDirection: SortDirection;
  statusFilter: StatusFilter;
  searchQuery: string;
  setStatusFilter: (f: StatusFilter) => void;
  setSearchQuery: (q: string) => void;
  allProfiles: EnrichedProfileHealthReport[];
  missingProtonCount: number;
  filteredProfiles: EnrichedProfileHealthReport[];
  hasUnknownSentinel: boolean;
  cachedSnapshotList: CachedHealthSnapshot[];
  recentFailures: EnrichedProfileHealthReport[];
  cardTrends: { healthy: CardTrend; stale: CardTrend; broken: CardTrend };
  ariaAnnouncement: string;
  recheckPendingRef: MutableRefObject<boolean>;
  handleSortClick: (field: SortField) => void;
  getAriaSortAttr: (field: SortField) => 'ascending' | 'descending' | 'none';
}

interface UseHealthDashboardStateParams {
  summary: EnrichedHealthSummary | null;
  loading: boolean;
  error: string | null;
  batchValidate: () => Promise<void>;
  cachedSnapshots: Record<string, CachedHealthSnapshot>;
}

export function useHealthDashboardState({
  summary,
  loading,
  error,
  batchValidate,
  cachedSnapshots,
}: UseHealthDashboardStateParams): HealthDashboardStateReturn {
  const offlineReadiness = useOfflineReadiness();

  const [sortField, setSortField] = useState<SortField>('status');
  const [sortDirection, setSortDirection] = useState<SortDirection>('desc');
  const [statusFilter, setStatusFilter] = useState<StatusFilter>('all');
  const [searchQuery, setSearchQuery] = useState('');
  const [ariaAnnouncement, setAriaAnnouncement] = useState('');

  const recheckPendingRef = useRef(false);
  const yButtonPrevRef = useRef(false);

  const deferredSearch = useDeferredValue(searchQuery);

  function handleSortClick(field: SortField) {
    if (field === sortField) {
      setSortDirection((d) => (d === 'asc' ? 'desc' : 'asc'));
    } else {
      setSortField(field);
      setSortDirection('asc');
    }
  }

  function getAriaSortAttr(field: SortField): 'ascending' | 'descending' | 'none' {
    if (sortField !== field) return 'none';
    return sortDirection === 'asc' ? 'ascending' : 'descending';
  }

  const allProfiles = useMemo(() => {
    return (summary?.profiles ?? []).filter((r) => r.name !== '<unknown>');
  }, [summary?.profiles]);

  const missingProtonCount = useMemo(() => {
    return allProfiles.filter((r) => r.issues.some((issue) => categorizeIssue(issue) === 'missing_proton')).length;
  }, [allProfiles]);

  const filteredProfiles = useMemo(() => {
    const term = deferredSearch.toLowerCase().trim();

    let result = allProfiles;

    if (statusFilter !== 'all') {
      result = result.filter((r) => r.status === statusFilter);
    }

    if (term.length > 0) {
      result = result.filter((r) => r.name.toLowerCase().includes(term));
    }

    result = result.slice().sort((a, b) => {
      // Favorites always pin to top regardless of sort
      const aFav = a.metadata?.is_favorite ?? false;
      const bFav = b.metadata?.is_favorite ?? false;
      if (aFav !== bFav) return aFav ? -1 : 1;

      let cmp = 0;
      switch (sortField) {
        case 'name':
          cmp = a.name.localeCompare(b.name);
          break;
        case 'status':
          cmp = (STATUS_RANK[a.status] ?? 0) - (STATUS_RANK[b.status] ?? 0);
          if (cmp === 0) cmp = a.name.localeCompare(b.name);
          break;
        case 'issues':
          cmp = a.issues.length - b.issues.length;
          break;
        case 'last_success': {
          const aSuccess = a.metadata?.last_success ?? '';
          const bSuccess = b.metadata?.last_success ?? '';
          cmp = aSuccess.localeCompare(bSuccess);
          break;
        }
        case 'launch_method':
          cmp = a.launch_method.localeCompare(b.launch_method);
          break;
        case 'failures':
          cmp = (a.metadata?.failure_count_30d ?? 0) - (b.metadata?.failure_count_30d ?? 0);
          break;
        case 'favorite': {
          const aFavSort = a.metadata?.is_favorite ? 1 : 0;
          const bFavSort = b.metadata?.is_favorite ? 1 : 0;
          cmp = aFavSort - bFavSort;
          break;
        }
        case 'version_status': {
          const aRank = VERSION_STATUS_RANK[a.metadata?.version_status ?? 'unknown'] ?? -1;
          const bRank = VERSION_STATUS_RANK[b.metadata?.version_status ?? 'unknown'] ?? -1;
          cmp = aRank - bRank;
          break;
        }
        case 'offline_score': {
          cmp =
            offlineSortScore(a, offlineReadiness.reportForProfile(a.name)) -
            offlineSortScore(b, offlineReadiness.reportForProfile(b.name));
          if (cmp === 0) {
            cmp = a.name.localeCompare(b.name);
          }
          break;
        }
        default:
          cmp = 0;
      }

      return sortDirection === 'asc' ? cmp : -cmp;
    });

    return result;
  }, [allProfiles, sortField, sortDirection, statusFilter, deferredSearch, offlineReadiness]);

  const hasUnknownSentinel = (summary?.profiles ?? []).some((r) => r.name === '<unknown>');

  const cachedSnapshotList = useMemo(() => {
    return Object.values(cachedSnapshots)
      .slice()
      .sort((a, b) => {
        const rankDiff = (STATUS_RANK[b.status] ?? 0) - (STATUS_RANK[a.status] ?? 0);
        if (rankDiff !== 0) return rankDiff;
        return a.profile_name.localeCompare(b.profile_name);
      });
  }, [cachedSnapshots]);

  const recentFailures = useMemo(() => {
    return (summary?.profiles ?? [])
      .filter((r) => (r.metadata?.failure_count_30d ?? 0) > 0)
      .slice()
      .sort((a, b) => (b.metadata?.failure_count_30d ?? 0) - (a.metadata?.failure_count_30d ?? 0));
  }, [summary?.profiles]);

  const cardTrends = useMemo<{ healthy: CardTrend; stale: CardTrend; broken: CardTrend }>(() => {
    const snaps = Object.values(cachedSnapshots);
    if (snaps.length === 0 || !summary) {
      return { healthy: null, stale: null, broken: null };
    }
    const cachedHealthy = snaps.filter((s) => s.status === 'healthy').length;
    const cachedStale = snaps.filter((s) => s.status === 'stale').length;
    const cachedBroken = snaps.filter((s) => s.status === 'broken').length;
    const healthyDiff = summary.healthy_count - cachedHealthy;
    const staleDiff = summary.stale_count - cachedStale;
    const brokenDiff = summary.broken_count - cachedBroken;
    return {
      healthy: healthyDiff > 0 ? 'up' : healthyDiff < 0 ? 'down' : null,
      stale: staleDiff > 0 ? 'up' : staleDiff < 0 ? 'down' : null,
      broken: brokenDiff > 0 ? 'up' : brokenDiff < 0 ? 'down' : null,
    };
  }, [cachedSnapshots, summary]);

  useEffect(() => {
    if (typeof window === 'undefined') return;

    let rafId = 0;

    const poll = () => {
      const gamepad = navigator.getGamepads?.()[0];
      if (gamepad) {
        const yPressed = Boolean(gamepad.buttons[3]?.pressed);
        const wasPressed = yButtonPrevRef.current;
        if (yPressed && !wasPressed && !loading) {
          recheckPendingRef.current = true;
          void batchValidate();
        }
        yButtonPrevRef.current = yPressed;
      }
      rafId = window.requestAnimationFrame(poll);
    };

    rafId = window.requestAnimationFrame(poll);

    return () => {
      window.cancelAnimationFrame(rafId);
    };
  }, [loading, batchValidate]);

  useEffect(() => {
    if (loading) {
      if (recheckPendingRef.current) {
        setAriaAnnouncement('Checking all profiles...');
      }
      return;
    }
    if (!recheckPendingRef.current) {
      return;
    }
    recheckPendingRef.current = false;
    if (error) {
      setAriaAnnouncement('');
    } else if (summary) {
      setAriaAnnouncement(
        `Validation complete. ${summary.broken_count} broken, ${summary.stale_count} stale, ${summary.healthy_count} healthy.`
      );
    } else {
      setAriaAnnouncement('Validation complete.');
    }
  }, [loading, error, summary]);

  return {
    sortField,
    sortDirection,
    statusFilter,
    searchQuery,
    setStatusFilter,
    setSearchQuery,
    allProfiles,
    missingProtonCount,
    filteredProfiles,
    hasUnknownSentinel,
    cachedSnapshotList,
    recentFailures,
    cardTrends,
    ariaAnnouncement,
    recheckPendingRef,
    handleSortClick,
    getAriaSortAttr,
  };
}
