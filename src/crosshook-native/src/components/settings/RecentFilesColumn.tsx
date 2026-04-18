import { CollapsibleSection } from '../ui/CollapsibleSection';
import { RecentFilesSection } from './RecentFilesSection';
import type { RecentFilesState } from './types';

interface RecentFilesColumnProps {
  recentFiles: RecentFilesState;
  recentFilesLimit: number;
}

/** Right-hand column composing the three RecentFilesSection lists inside a CollapsibleSection. */
export function RecentFilesColumn({ recentFiles, recentFilesLimit }: RecentFilesColumnProps) {
  return (
    <section className="crosshook-settings-recent-column" aria-label="Recent files">
      <CollapsibleSection
        title="Recent Files"
        defaultOpen={false}
        className="crosshook-panel crosshook-settings-section"
        meta={<span className="crosshook-muted">Most recent paths used by the app</span>}
      >
        <p className="crosshook-muted crosshook-settings-help">
          These lists are intended to come from the backend recent-files store. Non-existent entries should be removed
          before the data is passed into this component.
        </p>

        <RecentFilesSection label="Games" paths={recentFiles.gamePaths} limit={recentFilesLimit} />
        <RecentFilesSection label="Trainers" paths={recentFiles.trainerPaths} limit={recentFilesLimit} />
        <RecentFilesSection label="DLLs" paths={recentFiles.dllPaths} limit={recentFilesLimit} />
      </CollapsibleSection>
    </section>
  );
}
