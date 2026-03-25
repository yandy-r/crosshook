import { useId, useState } from 'react';
import type { LaunchMethod } from '../types';
import {
  LAUNCH_OPTIMIZATION_CATEGORIES,
  LAUNCH_OPTIMIZATION_CATEGORY_LABELS,
  findLaunchOptimizationConflicts,
  getConflictingLaunchOptimizationIds,
  LAUNCH_OPTIMIZATION_OPTIONS,
  LAUNCH_OPTIMIZATION_OPTIONS_BY_ID,
  type LaunchOptimizationCategory,
  type LaunchOptimizationConflict,
  type LaunchOptimizationId,
  type LaunchOptimizationOption,
} from '../types/launch-optimizations';

type LaunchOptimizationsPanelStatusTone = 'idle' | 'saving' | 'success' | 'warning' | 'error';

export interface LaunchOptimizationsPanelStatus {
  tone: LaunchOptimizationsPanelStatusTone;
  label: string;
  detail?: string;
}

export interface LaunchOptimizationsPanelProps {
  method: LaunchMethod;
  enabledOptionIds: readonly LaunchOptimizationId[];
  onToggleOption: (optionId: LaunchOptimizationId, nextEnabled: boolean) => void;
  status?: LaunchOptimizationsPanelStatus;
  className?: string;
}

interface GroupedOptions {
  category: LaunchOptimizationCategory;
  options: LaunchOptimizationOption[];
}

const DEFAULT_STATUS: Record<LaunchMethod, LaunchOptimizationsPanelStatus> = {
  '': {
    tone: 'warning',
    label: 'Profile method is not set',
    detail: 'Launch optimizations are only available for Proton-backed profiles.',
  },
  native: {
    tone: 'warning',
    label: 'Unavailable for native launches',
    detail: 'Switch the launch method to proton_run to edit these Proton-specific toggles.',
  },
  proton_run: {
    tone: 'idle',
    label: 'Ready for Proton-backed launches',
    detail: 'These settings stay profile-scoped and only apply when the method is proton_run.',
  },
  steam_applaunch: {
    tone: 'warning',
    label: 'Unavailable for Steam launches',
    detail: 'Switch the launch method to proton_run to edit these Proton-specific toggles.',
  },
};

function joinClasses(...values: Array<string | false | null | undefined>): string {
  return values.filter(Boolean).join(' ');
}

function formatCountLabel(count: number, singular: string, plural: string): string {
  return `${count} ${count === 1 ? singular : plural}`;
}

function groupOptions(options: readonly LaunchOptimizationOption[]): GroupedOptions[] {
  return LAUNCH_OPTIMIZATION_CATEGORIES.map((category) => ({
    category,
    options: options.filter((option) => option.category === category),
  })).filter((group) => group.options.length > 0);
}

function getConflictLabels(option: LaunchOptimizationOption): string[] {
  return (option.conflictsWith ?? [])
    .map((conflictId) => LAUNCH_OPTIMIZATION_OPTIONS_BY_ID[conflictId])
    .map((conflictOption) => conflictOption?.label)
    .filter((label): label is string => Boolean(label));
}

function getMainCaveat(option: LaunchOptimizationOption, conflictLabels: string[]): string {
  if (conflictLabels.length > 0) {
    return `Do not combine this with ${conflictLabels.join(', ')}.`;
  }

  if (option.community) {
    return 'This is community-documented or hardware-specific and can behave differently across Proton builds or drivers.';
  }

  return 'Use this only when the matching launch issue is present; otherwise leave it off.';
}

function formatConflictLabels(conflictLabels: readonly string[]): string {
  if (conflictLabels.length <= 1) {
    return conflictLabels[0] ?? '';
  }

  if (conflictLabels.length === 2) {
    return `${conflictLabels[0]} or ${conflictLabels[1]}`;
  }

  return `${conflictLabels.slice(0, -1).join(', ')}, or ${conflictLabels[conflictLabels.length - 1]}`;
}

function getStatusToneClass(tone: LaunchOptimizationsPanelStatusTone): string {
  return `crosshook-launch-optimizations__status-chip--${tone}`;
}

function formatConflictSummary(conflict: LaunchOptimizationConflict): string {
  return `${LAUNCH_OPTIMIZATION_OPTIONS_BY_ID[conflict.optionId].label} conflicts with ${LAUNCH_OPTIMIZATION_OPTIONS_BY_ID[conflict.conflictsWith].label}.`;
}

function OptionGroup(props: {
  group: GroupedOptions;
  enabledIds: Set<LaunchOptimizationId>;
  selectedConflicts: readonly LaunchOptimizationConflict[];
  isMethodSupported: boolean;
  onToggleOption: (optionId: LaunchOptimizationId, nextEnabled: boolean) => void;
  tooltipIdPrefix: string;
  tooltipId: LaunchOptimizationId | null;
  setTooltipId: (optionId: LaunchOptimizationId | null) => void;
  sectionTone: 'default' | 'advanced';
}) {
  const {
    group,
    enabledIds,
    selectedConflicts,
    isMethodSupported,
    onToggleOption,
    tooltipIdPrefix,
    tooltipId,
    setTooltipId,
    sectionTone,
  } = props;
  const groupOptionIds = group.options.map((option) => option.id);
  const groupConflicts = selectedConflicts.filter((conflict) => {
    return (
      groupOptionIds.includes(conflict.optionId) ||
      groupOptionIds.includes(conflict.conflictsWith)
    );
  });

  return (
    <fieldset className={joinClasses('crosshook-launch-optimizations__group', `crosshook-launch-optimizations__group--${sectionTone}`)}>
      <legend className="crosshook-launch-optimizations__group-title">{LAUNCH_OPTIMIZATION_CATEGORY_LABELS[group.category]}</legend>
      {groupConflicts.length > 0 ? (
        <div className="crosshook-warning-banner crosshook-launch-optimizations__group-warning">
          {groupConflicts.map(formatConflictSummary).join(' ')}
        </div>
      ) : null}
      <div className="crosshook-launch-optimizations__option-list">
        {group.options.map((option) => {
          const isEnabled = enabledIds.has(option.id);
          const isTooltipOpen = tooltipId === option.id;
          const conflictingIds = getConflictingLaunchOptimizationIds(
            option.id,
            [...enabledIds].filter((enabledOptionId) => enabledOptionId !== option.id)
          );
          const blockedByLabels = conflictingIds.map(
            (conflictingId) => LAUNCH_OPTIMIZATION_OPTIONS_BY_ID[conflictingId].label
          );
          const isBlockedByConflict = !isEnabled && blockedByLabels.length > 0;
          const isSupported =
            isMethodSupported &&
            option.applicableMethods.includes('proton_run') &&
            !isBlockedByConflict;
          const checkboxId = `${tooltipIdPrefix}-${option.id}`;
          const tooltipIdValue = `${tooltipIdPrefix}-${option.id}-tooltip`;
          const conflictLabels = getConflictLabels(option);

          return (
            <div
              key={option.id}
              className={joinClasses(
                'crosshook-launch-optimizations__option',
                isEnabled && 'crosshook-launch-optimizations__option--enabled',
                !isSupported && 'crosshook-launch-optimizations__option--disabled'
              )}
            >
              <div className="crosshook-launch-optimizations__option-body">
                <input
                  id={checkboxId}
                  className="crosshook-launch-optimizations__checkbox"
                  type="checkbox"
                  checked={isEnabled}
                  disabled={!isSupported}
                  onChange={(event) => onToggleOption(option.id, event.target.checked)}
                />

                <div className="crosshook-launch-optimizations__option-copy">
                  <label className="crosshook-launch-optimizations__option-label" htmlFor={checkboxId}>
                    {option.label}
                  </label>
                  <p className="crosshook-launch-optimizations__option-description">{option.description}</p>
                  <div className="crosshook-launch-optimizations__option-meta">
                    <span
                      className={joinClasses(
                        'crosshook-launch-optimizations__option-pill',
                        option.advanced && 'crosshook-launch-optimizations__option-pill--advanced',
                        !option.advanced && 'crosshook-launch-optimizations__option-pill--recommended'
                      )}
                    >
                      {option.advanced ? 'Advanced' : 'Recommended'}
                    </span>
                    {option.community ? (
                      <span className="crosshook-launch-optimizations__option-pill crosshook-launch-optimizations__option-pill--community">
                        Community
                      </span>
                    ) : null}
                    {conflictLabels.length > 0 ? (
                      <span className="crosshook-launch-optimizations__option-pill crosshook-launch-optimizations__option-pill--warning">
                        Conflicts with {conflictLabels.join(', ')}
                      </span>
                    ) : null}
                    {blockedByLabels.length > 0 ? (
                      <span className="crosshook-launch-optimizations__option-pill crosshook-launch-optimizations__option-pill--warning">
                        Blocked by {formatConflictLabels(blockedByLabels)}
                      </span>
                    ) : null}
                    {!isSupported ? (
                      <span className="crosshook-launch-optimizations__option-pill crosshook-launch-optimizations__option-pill--disabled">
                        {isBlockedByConflict ? 'Resolve conflict first' : 'Proton only'}
                      </span>
                    ) : null}
                  </div>
                </div>
              </div>

              <button
                type="button"
                className="crosshook-launch-optimizations__info-button"
                aria-label={`More information about ${option.label}`}
                aria-expanded={isTooltipOpen}
                aria-describedby={isTooltipOpen ? tooltipIdValue : undefined}
                onFocus={() => setTooltipId(option.id)}
                onBlur={() => setTooltipId(null)}
                onMouseEnter={() => setTooltipId(option.id)}
                onMouseLeave={() => setTooltipId(null)}
                onKeyDown={(event) => {
                  if (event.key === 'Escape') {
                    setTooltipId(null);
                  }
                }}
              >
                i
              </button>

              <div
                id={tooltipIdValue}
                role="tooltip"
                aria-hidden={!isTooltipOpen}
                className={joinClasses(
                  'crosshook-launch-optimizations__tooltip',
                  isTooltipOpen && 'crosshook-launch-optimizations__tooltip--open'
                )}
              >
                <p className="crosshook-launch-optimizations__tooltip-title">{option.label}</p>
                <p className="crosshook-launch-optimizations__tooltip-kicker">What it does</p>
                <p className="crosshook-launch-optimizations__tooltip-copy">{option.description}</p>
                <p className="crosshook-launch-optimizations__tooltip-kicker">When it helps</p>
                <p className="crosshook-launch-optimizations__tooltip-copy">{option.helpText}</p>
                <p className="crosshook-launch-optimizations__tooltip-kicker">Main caveat</p>
                <p className="crosshook-launch-optimizations__tooltip-copy">
                  {getMainCaveat(option, conflictLabels)}
                </p>
                {blockedByLabels.length > 0 ? (
                  <p className="crosshook-launch-optimizations__tooltip-copy">
                    Selection is currently blocked by {formatConflictLabels(blockedByLabels)}.
                  </p>
                ) : null}
                {conflictLabels.length > 0 ? (
                  <p className="crosshook-launch-optimizations__tooltip-copy">
                    Conflict note: {conflictLabels.join(', ')} should not be enabled together with this option.
                  </p>
                ) : null}
              </div>
            </div>
          );
        })}
      </div>
    </fieldset>
  );
}

export function LaunchOptimizationsPanel({
  method,
  enabledOptionIds,
  onToggleOption,
  status,
  className,
}: LaunchOptimizationsPanelProps) {
  const titleId = useId();
  const tooltipIdPrefix = useId();
  const [tooltipId, setTooltipId] = useState<LaunchOptimizationId | null>(null);

  const isMethodSupported = method === 'proton_run';
  const seen = new Set<LaunchOptimizationId>();
  const selectedOptionIds = enabledOptionIds.filter((optionId) => {
    if (!LAUNCH_OPTIMIZATION_OPTIONS_BY_ID[optionId] || seen.has(optionId)) {
      return false;
    }

    seen.add(optionId);
    return true;
  });
  const enabledIdSet = new Set(selectedOptionIds);
  const selectedOptions = selectedOptionIds.map((optionId) => LAUNCH_OPTIMIZATION_OPTIONS_BY_ID[optionId]);
  const selectedConflicts = findLaunchOptimizationConflicts(selectedOptionIds);
  const commonOptions = LAUNCH_OPTIMIZATION_OPTIONS.filter((option) => !option.advanced);
  const advancedOptions = LAUNCH_OPTIMIZATION_OPTIONS.filter((option) => option.advanced);
  const commonGroups = groupOptions(commonOptions);
  const advancedGroups = groupOptions(advancedOptions);
  const enabledCommonCount = selectedOptions.filter((option) => !option.advanced).length;
  const enabledAdvancedCount = selectedOptions.filter((option) => option.advanced).length;
  const defaultStatus = DEFAULT_STATUS[method] ?? DEFAULT_STATUS[''];
  const resolvedStatus = status ?? defaultStatus;
  const rootClassName = joinClasses('crosshook-panel', 'crosshook-launch-optimizations', className);
  const advancedOpen = enabledAdvancedCount > 0;

  return (
    <section className={rootClassName} aria-labelledby={titleId} data-method={method}>
      <div className="crosshook-launch-optimizations__header">
        <div className="crosshook-launch-optimizations__heading">
          <h2 id={titleId} className="crosshook-launch-optimizations__title">
            Launch Optimizations
          </h2>
          <p className="crosshook-help-text crosshook-launch-optimizations__intro">
            Curated Proton tweaks with readable labels, short guidance, and accessible info popovers.
          </p>
        </div>

        <div className="crosshook-launch-optimizations__summary-row" aria-label="Launch optimization summary">
          <span className="crosshook-launch-optimizations__summary-chip">
            {formatCountLabel(selectedOptions.length, 'enabled option', 'enabled options')}
          </span>
          <span className="crosshook-launch-optimizations__summary-chip">
            {formatCountLabel(enabledCommonCount, 'common option', 'common options')} /{' '}
            {formatCountLabel(enabledAdvancedCount, 'advanced option', 'advanced options')}
          </span>
          <span
            className={joinClasses(
              'crosshook-launch-optimizations__status-chip',
              getStatusToneClass(resolvedStatus.tone)
            )}
          >
            {resolvedStatus.label}
          </span>
        </div>
      </div>

      <div className="crosshook-launch-optimizations__status" aria-live="polite">
        <p className="crosshook-launch-optimizations__status-copy">{resolvedStatus.label}</p>
        {resolvedStatus.detail ? <p className="crosshook-help-text">{resolvedStatus.detail}</p> : null}
      </div>

      {!isMethodSupported ? (
        <div className="crosshook-warning-banner crosshook-launch-optimizations__method-warning">
          Proton launch optimizations are only editable when the profile method is <code>proton_run</code>.
        </div>
      ) : null}

      <div className="crosshook-launch-optimizations__sections">
        <div className="crosshook-launch-optimizations__section">
          <div className="crosshook-launch-optimizations__section-title">Recommended</div>
          <div className="crosshook-launch-optimizations__section-copy">
            Common launch fixes for controller handling, overlays, and windowing.
          </div>
          <div className="crosshook-launch-optimizations__group-list">
            {commonGroups.map((group) => (
              <OptionGroup
                key={group.category}
                group={group}
                enabledIds={enabledIdSet}
                selectedConflicts={selectedConflicts}
                isMethodSupported={isMethodSupported}
                onToggleOption={onToggleOption}
                tooltipIdPrefix={tooltipIdPrefix}
                tooltipId={tooltipId}
                setTooltipId={setTooltipId}
                sectionTone="default"
              />
            ))}
          </div>
        </div>

        <details className="crosshook-launch-optimizations__advanced" open={advancedOpen}>
          <summary className="crosshook-launch-optimizations__advanced-summary">
            <span>Advanced</span>
            <span className="crosshook-launch-optimizations__advanced-summary-meta">
              {formatCountLabel(advancedOptions.length, 'option', 'options')}
            </span>
          </summary>
          <p className="crosshook-help-text crosshook-launch-optimizations__advanced-copy">
            Experimental or hardware-specific toggles that are useful when the common fixes are not enough.
          </p>
          <div className="crosshook-launch-optimizations__group-list">
            {advancedGroups.map((group) => (
              <OptionGroup
                key={group.category}
                group={group}
                enabledIds={enabledIdSet}
                selectedConflicts={selectedConflicts}
                isMethodSupported={isMethodSupported}
                onToggleOption={onToggleOption}
                tooltipIdPrefix={tooltipIdPrefix}
                tooltipId={tooltipId}
                setTooltipId={setTooltipId}
                sectionTone="advanced"
              />
            ))}
          </div>
        </details>
      </div>
    </section>
  );
}

export default LaunchOptimizationsPanel;
