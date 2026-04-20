// Profile config history handlers: history, diff, rollback, mark known good.
// See `lib/mocks/README.md`.
// All error messages MUST start with `[dev-mock]` to participate in the
// `.github/workflows/release.yml` "Verify no mock code in production bundle"
// sentinel.

import type { ConfigDiffResult, ConfigRevisionSummary, ConfigRollbackResult } from '../../../types/profile-history';
import { emitMockEvent } from '../eventBus';
import { getStore } from '../store';
import { withProfileFixtureGate } from './profile-utils';
import type { Handler } from './types';

// Lightweight config revision history (profile name → ordered summaries, newest first)
export const profileConfigHistory = new Map<string, ConfigRevisionSummary[]>();

let nextRevisionId = 1;

export function appendRevision(profileName: string, source: ConfigRevisionSummary['source']): ConfigRevisionSummary {
  const revision: ConfigRevisionSummary = {
    id: nextRevisionId++,
    profile_name_at_write: profileName,
    source,
    content_hash: `mock-hash-${nextRevisionId}`,
    source_revision_id: null,
    is_last_known_working: false,
    created_at: new Date().toISOString(),
  };
  const existing = profileConfigHistory.get(profileName) ?? [];
  profileConfigHistory.set(profileName, [revision, ...existing]);
  return revision;
}

export function registerProfileHistory(map: Map<string, Handler>): void {
  map.set(
    'profile_config_history',
    withProfileFixtureGate('profile_config_history', async (args): Promise<ConfigRevisionSummary[]> => {
      const { name, limit } = args as { name: string; limit?: number };
      const trimmed = name.trim();
      const rows = profileConfigHistory.get(trimmed) ?? [];
      const capped = typeof limit === 'number' ? rows.slice(0, limit) : rows;
      return structuredClone(capped);
    })
  );

  map.set(
    'profile_config_diff',
    withProfileFixtureGate('profile_config_diff', async (args): Promise<ConfigDiffResult> => {
      const { name, revisionId, rightRevisionId } = args as {
        name: string;
        revisionId: number;
        rightRevisionId?: number;
      };
      const trimmed = name.trim();
      const rows = profileConfigHistory.get(trimmed) ?? [];
      const left = rows.find((r) => r.id === revisionId);
      if (!left) {
        throw new Error(`[dev-mock] profile_config_diff: revision ${revisionId} not found for profile "${trimmed}"`);
      }
      // In the mock, diff is always empty (no real TOML serialization)
      const rightLabel = rightRevisionId !== undefined ? `revision/${rightRevisionId}` : 'current';
      return {
        revision_id: revisionId,
        revision_source: left.source,
        revision_created_at: left.created_at,
        diff_text: `--- revision/${revisionId}\n+++ ${rightLabel}\n@@ -1,1 +1,1 @@\n [mock: no diff available in browser-dev mode]\n`,
        added_lines: 0,
        removed_lines: 0,
        truncated: false,
      };
    })
  );

  map.set(
    'profile_config_rollback',
    withProfileFixtureGate('profile_config_rollback', async (args): Promise<ConfigRollbackResult> => {
      const { name, revisionId } = args as { name: string; revisionId: number };
      const trimmed = name.trim();
      const store = getStore();
      const existing = store.profiles.get(trimmed);
      if (!existing) {
        throw new Error(`[dev-mock] profile_config_rollback: profile not found: ${trimmed}`);
      }
      const rows = profileConfigHistory.get(trimmed) ?? [];
      const target = rows.find((r) => r.id === revisionId);
      if (!target) {
        throw new Error(
          `[dev-mock] profile_config_rollback: revision ${revisionId} not found for profile "${trimmed}"`
        );
      }
      // In the mock, rollback restores the current profile unchanged (no real TOML snapshots)
      const restored = structuredClone(existing);
      const newRevision = appendRevision(trimmed, 'rollback_apply');
      newRevision.source_revision_id = revisionId;
      emitMockEvent('profiles-changed', { name: trimmed, action: 'rollback' });
      return {
        restored_revision_id: revisionId,
        new_revision_id: newRevision.id,
        profile: restored,
      };
    })
  );

  map.set(
    'profile_mark_known_good',
    withProfileFixtureGate('profile_mark_known_good', async (args) => {
      const { name, revisionId } = args as { name: string; revisionId: number };
      const trimmed = name.trim();
      const rows = profileConfigHistory.get(trimmed) ?? [];
      const target = rows.find((r) => r.id === revisionId);
      if (!target) {
        throw new Error(
          `[dev-mock] profile_mark_known_good: revision ${revisionId} not found for profile "${trimmed}"`
        );
      }
      // Clear known-good from all, then set on target
      for (const row of rows) {
        row.is_last_known_working = row.id === revisionId;
      }
      return null;
    })
  );
}

export function resetProfileHistoryState(): void {
  profileConfigHistory.clear();
  nextRevisionId = 1;
}
