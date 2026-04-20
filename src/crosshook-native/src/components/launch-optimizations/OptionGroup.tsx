import { open as openShell } from '@/lib/plugin-stubs/shell';
import type { CapabilityGate } from '../../hooks/useCapabilityGate';
import type { LaunchMethod } from '../../types';
import {
  getConflictingLaunchOptimizationIds,
  LAUNCH_OPTIMIZATION_CATEGORY_LABELS,
  type LaunchOptimizationCategory,
  type LaunchOptimizationConflict,
  type LaunchOptimizationId,
} from '../../types/launch-optimizations';
import type { OptimizationEntry } from '../../utils/optimization-catalog';
import { type CapabilityId, joinClasses } from './utils';

function getConflictLabels(
  option: OptimizationEntry,
  optionsById: Record<string, OptimizationEntry>,
  conflictMatrix: Record<string, readonly string[]>
): string[] {
  return (conflictMatrix[option.id] ?? [])
    .map((conflictId) => optionsById[conflictId])
    .map((conflictOption) => conflictOption?.label)
    .filter((label): label is string => Boolean(label));
}

function getMainCaveat(option: OptimizationEntry, conflictLabels: string[]): string {
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

function formatConflictSummary(
  conflict: LaunchOptimizationConflict,
  optionsById: Record<string, OptimizationEntry>
): string {
  return `${optionsById[conflict.optionId]?.label ?? conflict.optionId} conflicts with ${optionsById[conflict.conflictsWith]?.label ?? conflict.conflictsWith}.`;
}

function capabilityIdForRequiredBinary(requiredBinary: string): CapabilityId | null {
  switch (requiredBinary.trim()) {
    case 'gamescope':
      return 'gamescope';
    case 'mangohud':
      return 'mangohud';
    case 'gamemoderun':
      return 'gamemode';
    case 'winetricks':
    case 'protontricks':
      return 'prefix_tools';
    case 'umu-run':
    case 'umu_run':
      return 'non_steam_launch';
    default:
      return null;
  }
}

/** Catalog `required_binary` name → `HostToolCheckResult.tool_id` / capability map tool id. */
function toolIdForRequiredBinary(requiredBinary: string): string | null {
  switch (requiredBinary.trim()) {
    case 'gamemoderun':
      return 'gamemode';
    case 'umu-run':
    case 'umu_run':
      return 'umu_run';
    default: {
      const trimmed = requiredBinary.trim();
      return trimmed.length > 0 ? trimmed : null;
    }
  }
}

function getGpuVendorLabel(option: OptimizationEntry): string | null {
  if (option.target_gpu_vendor === 'nvidia') {
    return 'NVIDIA';
  }

  if (option.target_gpu_vendor === 'amd') {
    return 'AMD';
  }

  return null;
}

interface OptionGroupProps {
  group: {
    category: LaunchOptimizationCategory;
    options: OptimizationEntry[];
  };
  enabledIds: Set<LaunchOptimizationId>;
  selectedConflicts: readonly LaunchOptimizationConflict[];
  isMethodSupported: boolean;
  method: LaunchMethod;
  onToggleOption: (optionId: LaunchOptimizationId, nextEnabled: boolean) => void;
  tooltipIdPrefix: string;
  tooltipId: LaunchOptimizationId | null;
  setTooltipId: (optionId: LaunchOptimizationId | null) => void;
  sectionTone: 'default' | 'advanced';
  optionsById: Record<string, OptimizationEntry>;
  conflictMatrix: Record<string, readonly string[]>;
  capabilityGates: Record<CapabilityId, CapabilityGate>;
}

function OptionGroup({
  group,
  enabledIds,
  selectedConflicts,
  isMethodSupported,
  method,
  onToggleOption,
  tooltipIdPrefix,
  tooltipId,
  setTooltipId,
  sectionTone,
  optionsById,
  conflictMatrix,
  capabilityGates,
}: OptionGroupProps) {
  const groupOptionIds = group.options.map((option) => option.id);
  const groupConflicts = selectedConflicts.filter((conflict) => {
    return groupOptionIds.includes(conflict.optionId) || groupOptionIds.includes(conflict.conflictsWith);
  });

  return (
    <fieldset
      className={joinClasses(
        'crosshook-launch-optimizations__group',
        `crosshook-launch-optimizations__group--${sectionTone}`
      )}
    >
      <legend className="crosshook-launch-optimizations__group-title">
        {LAUNCH_OPTIMIZATION_CATEGORY_LABELS[group.category]}
      </legend>
      {groupConflicts.length > 0 ? (
        <div className="crosshook-warning-banner crosshook-launch-optimizations__group-warning">
          {groupConflicts.map((conflict) => formatConflictSummary(conflict, optionsById)).join(' ')}
        </div>
      ) : null}
      <div className="crosshook-launch-optimizations__option-list">
        {group.options.map((option) => {
          const isEnabled = enabledIds.has(option.id);
          const isTooltipOpen = tooltipId === option.id;
          const conflictingIds = getConflictingLaunchOptimizationIds(
            option.id,
            [...enabledIds].filter((enabledOptionId) => enabledOptionId !== option.id),
            conflictMatrix
          );
          const blockedByLabels = conflictingIds.map(
            (conflictingId) => optionsById[conflictingId]?.label ?? conflictingId
          );
          const isBlockedByConflict = !isEnabled && blockedByLabels.length > 0;
          const isSupported = isMethodSupported && option.applicable_methods.includes(method) && !isBlockedByConflict;
          const checkboxId = `${tooltipIdPrefix}-${option.id}`;
          const tooltipIdValue = `${tooltipIdPrefix}-${option.id}-tooltip`;
          const conflictLabels = getConflictLabels(option, optionsById, conflictMatrix);
          const capabilityId = capabilityIdForRequiredBinary(option.required_binary);
          const capabilityGate = capabilityId ? capabilityGates[capabilityId] : null;
          const requiredToolId = toolIdForRequiredBinary(option.required_binary);
          const isRequiredBinaryMissing =
            capabilityGate != null && requiredToolId != null && capabilityGate.missingToolIds.includes(requiredToolId);
          const rowDisabled = !isSupported || isRequiredBinaryMissing;

          return (
            <div
              key={option.id}
              className={joinClasses(
                'crosshook-launch-optimizations__option',
                isEnabled && 'crosshook-launch-optimizations__option--enabled',
                isTooltipOpen && 'crosshook-launch-optimizations__option--tooltip-open',
                rowDisabled && 'crosshook-launch-optimizations__option--disabled'
              )}
            >
              <div className="crosshook-launch-optimizations__option-body">
                <input
                  id={checkboxId}
                  className="crosshook-launch-optimizations__checkbox"
                  type="checkbox"
                  checked={isEnabled}
                  disabled={rowDisabled}
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
                    {getGpuVendorLabel(option) ? (
                      <span
                        className={joinClasses(
                          'crosshook-launch-optimizations__option-pill',
                          'crosshook-launch-optimizations__option-pill--vendor',
                          option.target_gpu_vendor === 'nvidia' &&
                            'crosshook-launch-optimizations__option-pill--vendor-nvidia',
                          option.target_gpu_vendor === 'amd' &&
                            'crosshook-launch-optimizations__option-pill--vendor-amd'
                        )}
                      >
                        {getGpuVendorLabel(option)}
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
                    {rowDisabled ? (
                      <span className="crosshook-launch-optimizations__option-pill crosshook-launch-optimizations__option-pill--disabled">
                        {isBlockedByConflict
                          ? 'Resolve conflict first'
                          : isRequiredBinaryMissing
                            ? 'Host tool missing'
                            : isMethodSupported
                              ? 'Unavailable'
                              : 'Not for this method'}
                      </span>
                    ) : null}
                  </div>
                </div>
              </div>

              {/* biome-ignore lint/a11y/noStaticElementInteractions: container-level hover only; interactive child button carries full a11y */}
              <div
                className="crosshook-launch-optimizations__info-anchor"
                onMouseEnter={() => setTooltipId(option.id)}
                onMouseLeave={() => setTooltipId(null)}
              >
                <button
                  type="button"
                  className="crosshook-launch-optimizations__info-button"
                  aria-label={`More information about ${option.label}`}
                  aria-expanded={isTooltipOpen}
                  aria-describedby={isTooltipOpen ? tooltipIdValue : undefined}
                  onFocus={() => setTooltipId(option.id)}
                  onBlur={() => setTooltipId(null)}
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
                  <p className="crosshook-launch-optimizations__tooltip-copy">{option.help_text}</p>
                  <p className="crosshook-launch-optimizations__tooltip-kicker">Main caveat</p>
                  <p className="crosshook-launch-optimizations__tooltip-copy">
                    {getMainCaveat(option, conflictLabels)}
                  </p>
                  {blockedByLabels.length > 0 ? (
                    <p className="crosshook-launch-optimizations__tooltip-copy">
                      Selection is currently blocked by {formatConflictLabels(blockedByLabels)}.
                    </p>
                  ) : null}
                  {isRequiredBinaryMissing && capabilityGate?.rationale ? (
                    <>
                      <p className="crosshook-launch-optimizations__tooltip-copy">{capabilityGate.rationale}</p>
                      <div style={{ display: 'flex', gap: 8, flexWrap: 'wrap' }}>
                        {capabilityGate.onCopyCommand ? (
                          <button
                            type="button"
                            className="crosshook-button crosshook-button--ghost crosshook-button--small"
                            onClick={() => {
                              void capabilityGate.onCopyCommand?.();
                            }}
                          >
                            Copy install command
                          </button>
                        ) : null}
                        {capabilityGate.docsUrl ? (
                          <button
                            type="button"
                            className="crosshook-button crosshook-button--ghost crosshook-button--small"
                            onClick={() => {
                              void openShell(capabilityGate.docsUrl ?? '');
                            }}
                          >
                            Open docs
                          </button>
                        ) : null}
                      </div>
                    </>
                  ) : null}
                  {conflictLabels.length > 0 ? (
                    <p className="crosshook-launch-optimizations__tooltip-copy">
                      Conflict note: {conflictLabels.join(', ')} should not be enabled together with this option.
                    </p>
                  ) : null}
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </fieldset>
  );
}

export default OptionGroup;
