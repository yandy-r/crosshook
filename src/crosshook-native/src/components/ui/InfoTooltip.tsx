import * as Tooltip from '@radix-ui/react-tooltip';
import { InfoCircleIcon } from '../icons/SidebarIcons';

interface InfoTooltipProps {
  /** Tooltip text content. */
  content: string;
  /** Size of the info icon in px. Defaults to 16. */
  size?: number;
}

/**
 * A small (i) icon that shows a Radix tooltip on hover/focus.
 * Shared across TrainerSection, GamescopeConfigPanel, and anywhere
 * a compact inline hint is needed next to a label or checkbox.
 *
 * Requires a `<Tooltip.Provider>` ancestor (provided at app root in App.tsx).
 */
export function InfoTooltip({ content, size = 16 }: InfoTooltipProps) {
  return (
    <Tooltip.Root>
      <Tooltip.Trigger asChild>
        {/* biome-ignore lint/a11y/useSemanticElements: native <button> is not valid inside <label>; span+role keeps tooltip triggers labelable without nesting interactive controls */}
        <span
          role="button"
          tabIndex={0}
          aria-label="Info"
          onClick={(e) => {
            e.stopPropagation();
            e.preventDefault();
          }}
          onKeyDown={(e) => {
            if (e.key === ' ' || e.key === 'Enter') {
              e.stopPropagation();
              e.preventDefault();
            }
          }}
          style={{
            display: 'inline-flex',
            alignItems: 'center',
            color: 'var(--crosshook-color-text-subtle)',
            cursor: 'help',
            flexShrink: 0,
            background: 'none',
            border: 'none',
            padding: 0,
          }}
        >
          <InfoCircleIcon width={size} height={size} />
        </span>
      </Tooltip.Trigger>
      <Tooltip.Portal>
        <Tooltip.Content
          side="top"
          sideOffset={6}
          style={{
            maxWidth: 320,
            padding: '8px 12px',
            borderRadius: 8,
            fontSize: '0.85rem',
            lineHeight: 1.4,
            color: 'var(--crosshook-color-text)',
            background: 'var(--crosshook-color-surface-raised, #2a2a2e)',
            border: '1px solid var(--crosshook-color-border-strong)',
            boxShadow: '0 4px 16px rgba(0,0,0,0.4)',
            zIndex: 9999,
          }}
        >
          {content}
          <Tooltip.Arrow style={{ fill: 'var(--crosshook-color-surface-raised, #2a2a2e)' }} />
        </Tooltip.Content>
      </Tooltip.Portal>
    </Tooltip.Root>
  );
}
