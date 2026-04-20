import type { ConfigDiffResult, ConfigRevisionSummary } from '../../types/profile-history';

export interface ConfigHistoryPanelProps {
  profileName: string;
  onClose: () => void;
  fetchConfigHistory: (profileName: string, limit?: number) => Promise<ConfigRevisionSummary[]>;
  fetchConfigDiff: (profileName: string, revisionId: number, rightRevisionId?: number) => Promise<ConfigDiffResult>;
  rollbackConfig: (profileName: string, revisionId: number) => Promise<unknown>;
  markKnownGood: (profileName: string, revisionId: number) => Promise<void>;
  /** Called after a successful rollback so the caller can refresh health data. */
  onAfterRollback?: (profileName: string) => void;
}
