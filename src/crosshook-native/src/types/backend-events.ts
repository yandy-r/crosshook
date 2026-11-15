/**
 * Every event the backend emits to the webview, keyed by its wire name, with
 * the payload the frontend expects. The names mirror the `*_EVENT` constants
 * in `crates/crosshook-app/src/ipc/`; `scripts/check-mock-coverage.sh` fails
 * when a backend event is missing here or has no browser-dev mock emitter.
 */

import type { LaunchLogPayload } from '../utils/log';
import type { LaunchDiagnosticEvent } from './diagnostics';
import type { EnrichedHealthSummary } from './health';
import type { InjectionLogEvent } from './injection';
import type { LaunchCompletePayload } from './launch';
import type { LaunchSessionChangedPayload } from './launch-sessions';
import type { OfflineReadinessScanCompletePayload } from './offline';
import type { PrefixDepCompletePayload, PrefixDepLogPayload } from './prefix-deps';
import type { ProfilesChangedPayload } from './profile-events';
import type { ProtonInstallProgress } from './protonup';
import type { RunExecutableCompletePayload, RunExecutableLogPayload } from './run-executable';
import type { StartupJobFinished } from './startup';
import type { VersionScanComplete } from './version';

/** Launch events always carry the `session_id` that the listener-side types leave optional. */
type WithSessionId<T> = T & { session_id: string };

export interface BackendEventPayloads {
  'startup-job-finished': StartupJobFinished;
  'profiles-changed': ProfilesChangedPayload;
  'profile-health-batch-complete': EnrichedHealthSummary;
  'offline-readiness-scan-complete': OfflineReadinessScanCompletePayload;
  'version-scan-complete': VersionScanComplete;
  'launch-session-changed': LaunchSessionChangedPayload;
  /** The backend always sends `session_id`. */
  'launch-log': Required<LaunchLogPayload>;
  'launch-diagnostic': WithSessionId<LaunchDiagnosticEvent>;
  'launch-complete': WithSessionId<LaunchCompletePayload>;
  'injection-log': InjectionLogEvent;
  'update-log': string;
  'update-complete': number | null;
  'run-executable-log': RunExecutableLogPayload;
  'run-executable-complete': RunExecutableCompletePayload;
  'prefix-dep-log': PrefixDepLogPayload;
  'prefix-dep-complete': PrefixDepCompletePayload;
  'protonup:install:progress': ProtonInstallProgress;
}

export type BackendEventName = keyof BackendEventPayloads;
