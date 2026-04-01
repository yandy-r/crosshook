import { useCallback, useId, useMemo, useState } from 'react';
import * as Tabs from '@radix-ui/react-tabs';
import type { BundledOptimizationPreset, LaunchMethod } from '../types';
import { ThemedSelect, type SelectOptionGroup } from './ui/ThemedSelect';
import {
  LAUNCH_OPTIMIZATION_CATEGORIES,
  LAUNCH_OPTIMIZATION_CATEGORY_LABELS,
  findLaunchOptimizationConflicts,
  getConflictingLaunchOptimizationIds,
  type LaunchOptimizationCategory,
  type LaunchOptimizationConflict,
  type LaunchOptimizationId,
} from '../types/launch-optimizations';
import type { OptimizationCatalogPayload, OptimizationEntry } from '../utils/optimization-catalog';
import { buildOptionsById, buildConflictMatrix } from '../utils/optimization-catalog';

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
  /** Sorted preset names from `launch.presets` (empty hides the preset row). */
  optimizationPresetNames?: readonly string[];
  activeOptimizationPreset?: string;
  onSelectOptimizationPreset?: (presetName: string) => void;
  /** Bundled GPU presets from the app catalog (metadata DB). */
  bundledOptimizationPresets?: readonly BundledOptimizationPreset[];
  onApplyBundledPreset?: (presetId: string) => void;
  /** Disables bundled / manual preset actions while IPC runs. */
  optimizationPresetActionBusy?: boolean;
  /** Saves current checkbox selection as a new named user preset. */
  onSaveManualPreset?: (presetName: string) => Promise<void>;
  /** Runtime optimization catalog from the backend. Null while loading. */
  catalog: OptimizationCatalogPayload | null;
}

interface GroupedOptions {
  category: LaunchOptimizationCategory;
  options: OptimizationEntry[];
}

const DEFAULT_STATUS: Record<LaunchMethod, LaunchOptimizationsPanelStatus> = {
  '': {
    tone: 'warning',
    label: 'Profile method is not set',
    detail: 'Launch optimizations apply to proton_run and steam_applaunch profiles.',
  },
  native: {
    tone: 'warning',
    label: 'Unavailable for native launches',
    detail: 'Switch the launch method to proton_run or steam_applaunch to edit these toggles.',
  },
  proton_run: {
    tone: 'idle',
    label: 'Ready for Proton-backed launches',
    detail: 'These settings stay profile-scoped and apply to direct proton_run launches.',
  },
  steam_applaunch: {
    tone: 'idle',
    label: 'Ready for Steam launch options',
    detail:
      'Use the Steam launch options panel below to copy a line into Steam; CrossHook does not inject it automatically.',
  },
};

function joinClasses(...values: Array<string | false | null | undefined>): string {
  return values.filter(Boolean).join(' ');
}

const BUNDLED_PRESET_KEY_PREFIX = 'bundled/';

/** TOML key under `[launch.presets]` for a catalog GPU preset; matches crosshook-core `bundled_optimization_preset_toml_key`. */
function bundledOptimizationTomlKey(presetId: string): string {
  return `${BUNDLED_PRESET_KEY_PREFIX}${presetId.trim()}`;
}

function formatCountLabel(count: number, singular: string, plural: string): string {
  return `${count} ${count === 1 ? singular : plural}`;
}

function groupOptions(options: readonly OptimizationEntry[]): GroupedOptions[] {
  return LAUNCH_OPTIMIZATION_CATEGORIES.map((category) => ({
    category,
    options: options.filter((option) => option.category === category),
  })).filter((group) => group.options.length > 0);
}

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

function getStatusToneClass(tone: LaunchOptimizationsPanelStatusTone): string {
  return `crosshook-launch-optimizations__status-chip--${tone}`;
}

function formatConflictSummary(
  conflict: LaunchOptimizationConflict,
  optionsById: Record<string, OptimizationEntry>
): string {
  return `${optionsById[conflict.optionId]?.label ?? conflict.optionId} conflicts with ${optionsById[conflict.conflictsWith]?.label ?? conflict.conflictsWith}.`;
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

function OptionGroup(props: {
  group: GroupedOptions;
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
}) {
  const {
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
  } = props;
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

          return (
            <div
              key={option.id}
              className={joinClasses(
                'crosshook-launch-optimizations__option',
                isEnabled && 'crosshook-launch-optimizations__option--enabled',
                isTooltipOpen && 'crosshook-launch-optimizations__option--tooltip-open',
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
                    {!isSupported ? (
                      <span className="crosshook-launch-optimizations__option-pill crosshook-launch-optimizations__option-pill--disabled">
                        {isBlockedByConflict
                          ? 'Resolve conflict first'
                          : isMethodSupported
                            ? 'Unavailable'
                            : 'Not for this method'}
                      </span>
                    ) : null}
                  </div>
                </div>
              </div>

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

export function LaunchOptimizationsPanel({
  method,
  enabledOptionIds,
  onToggleOption,
  status,
  className,
  optimizationPresetNames = [],
  activeOptimizationPreset = '',
  onSelectOptimizationPreset,
  bundledOptimizationPresets = [],
  onApplyBundledPreset,
  optimizationPresetActionBusy = false,
  onSaveManualPreset,
  catalog,
}: LaunchOptimizationsPanelProps) {
  const titleId = useId();
  const presetSelectId = useId();
  const manualPresetInputId = useId();
  const tooltipIdPrefix = useId();
  const [tooltipId, setTooltipId] = useState<LaunchOptimizationId | null>(null);
  const [manualPresetName, setManualPresetName] = useState('');
  const [manualSavePending, setManualSavePending] = useState(false);
  const [optimizationSection, setOptimizationSection] = useState<'recommended' | 'advanced'>('recommended');

  const isMethodSupported = method === 'proton_run' || method === 'steam_applaunch';

  const optionsById = useMemo(() => (catalog ? buildOptionsById(catalog.entries) : {}), [catalog]);
  const conflictMatrix = useMemo(() => (catalog ? buildConflictMatrix(catalog.entries) : {}), [catalog]);

  const savedPresetNameOptions = useMemo(
    () =>
      optimizationPresetNames
        .filter((n) => !n.startsWith(BUNDLED_PRESET_KEY_PREFIX))
        .map((n) => ({ value: n, label: n })),
    [optimizationPresetNames]
  );

  const savedPresetOptionsWithOrphan = useMemo(() => {
    const active = activeOptimizationPreset.trim();
    const catalogBundledKeys = new Set(
      bundledOptimizationPresets.map((p) => bundledOptimizationTomlKey(p.preset_id))
    );
    const opts = [...savedPresetNameOptions];
    if (
      active &&
      optimizationPresetNames.includes(active) &&
      !catalogBundledKeys.has(active) &&
      !opts.some((o) => o.value === active)
    ) {
      opts.push({ value: active, label: active });
    }
    return opts;
  }, [
    savedPresetNameOptions,
    activeOptimizationPreset,
    optimizationPresetNames,
    bundledOptimizationPresets,
  ]);

  const optimizationPresetGroups = useMemo((): SelectOptionGroup[] => {
    const groups: SelectOptionGroup[] = [];
    if (bundledOptimizationPresets.length > 0 && onApplyBundledPreset) {
      groups.push({
        label: 'Built-in',
        options: bundledOptimizationPresets.map((p) => ({
          value: bundledOptimizationTomlKey(p.preset_id),
          label: p.display_name,
          badge: 'Built-in',
        })),
      });
    }
    if (savedPresetOptionsWithOrphan.length > 0 && onSelectOptimizationPreset) {
      groups.push({ label: 'Saved', options: savedPresetOptionsWithOrphan });
    }
    return groups;
  }, [
    bundledOptimizationPresets,
    onApplyBundledPreset,
    onSelectOptimizationPreset,
    savedPresetOptionsWithOrphan,
  ]);

  const presetSelectValue = useMemo(() => {
    const active = activeOptimizationPreset.trim();
    if (!active) {
      return '';
    }
    const catalogKeys = new Set(bundledOptimizationPresets.map((p) => bundledOptimizationTomlKey(p.preset_id)));
    const inSaved = savedPresetOptionsWithOrphan.some((o) => o.value === active);
    if (catalogKeys.has(active) || inSaved) {
      return active;
    }
    return '';
  }, [activeOptimizationPreset, bundledOptimizationPresets, savedPresetOptionsWithOrphan]);

  const handleOptimizationPresetChange = useCallback(
    (value: string) => {
      if (value.startsWith(BUNDLED_PRESET_KEY_PREFIX)) {
        const presetId = value.slice(BUNDLED_PRESET_KEY_PREFIX.length);
        const isCatalogBundled = bundledOptimizationPresets.some((p) => p.preset_id === presetId);
        if (isCatalogBundled && onApplyBundledPreset) {
          void onApplyBundledPreset(presetId);
          return;
        }
      }
      void onSelectOptimizationPreset?.(value);
    },
    [bundledOptimizationPresets, onApplyBundledPreset, onSelectOptimizationPreset]
  );

  if (!catalog) {
    return <div className="crosshook-optimization-panel">Loading optimizations...</div>;
  }

  const presetActionBusy = optimizationPresetActionBusy || manualSavePending;
  const seen = new Set<LaunchOptimizationId>();
  const selectedOptionIds = enabledOptionIds.filter((optionId) => {
    if (!optionsById[optionId] || seen.has(optionId)) {
      return false;
    }

    seen.add(optionId);
    return true;
  });
  const enabledIdSet = new Set(selectedOptionIds);
  const selectedOptions = selectedOptionIds.map((optionId) => optionsById[optionId]);
  const selectedConflicts = findLaunchOptimizationConflicts(selectedOptionIds, conflictMatrix);
  const commonOptions = catalog.entries.filter((option) => !option.advanced);
  const advancedOptions = catalog.entries.filter((option) => option.advanced);
  const commonGroups = groupOptions(commonOptions);
  const advancedGroups = groupOptions(advancedOptions);
  const enabledCommonCount = selectedOptions.filter((option) => !option.advanced).length;
  const enabledAdvancedCount = selectedOptions.filter((option) => option.advanced).length;
  const defaultStatus = DEFAULT_STATUS[method] ?? DEFAULT_STATUS[''];
  const resolvedStatus = status ?? defaultStatus;
  const rootClassName = joinClasses('crosshook-panel', 'crosshook-launch-optimizations', className);
  const showManualPresetSave = isMethodSupported && onSaveManualPreset !== undefined;
  const showOptimizedPresetSelect = isMethodSupported && optimizationPresetGroups.length > 0;
  const showPresetToolbar = showManualPresetSave || showOptimizedPresetSelect;

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
            aria-live="polite"
            aria-atomic="true"
          >
            {resolvedStatus.label}
          </span>
        </div>
      </div>

      {showPresetToolbar ? (
        <div className="crosshook-launch-optimizations__preset-toolbar">
          <div className="crosshook-launch-optimizations__preset-toolbar-row">
            {showManualPresetSave ? (
              <>
                <div className="crosshook-launch-optimizations__manual-save-field">
                  <label htmlFor={manualPresetInputId}>Save current toggles as preset</label>
                  <input
                    id={manualPresetInputId}
                    className="crosshook-launch-optimizations__manual-save-input"
                    type="text"
                    autoComplete="off"
                    placeholder="e.g. My DXVK tweaks"
                    value={manualPresetName}
                    disabled={presetActionBusy}
                    onChange={(event) => setManualPresetName(event.target.value)}
                  />
                </div>
                <button
                  type="button"
                  className="crosshook-button crosshook-button--secondary crosshook-launch-optimizations__preset-save-btn"
                  disabled={presetActionBusy || !manualPresetName.trim()}
                  onClick={() => {
                    const save = onSaveManualPreset;
                    if (!save) {
                      return;
                    }
                    void (async () => {
                      setManualSavePending(true);
                      try {
                        await save(manualPresetName.trim());
                        setManualPresetName('');
                      } catch {
                        /* status / error surfaced by profile hook */
                      } finally {
                        setManualSavePending(false);
                      }
                    })();
                  }}
                >
                  Save preset
                </button>
              </>
            ) : null}
            {showOptimizedPresetSelect ? (
              <div className="crosshook-launch-optimizations__preset-select-cluster">
                <label className="crosshook-launch-optimizations__preset-label" htmlFor={presetSelectId}>
                  Optimized Presets
                </label>
                <ThemedSelect
                  id={presetSelectId}
                  value={presetSelectValue}
                  onValueChange={handleOptimizationPresetChange}
                  groups={optimizationPresetGroups}
                  placeholder="Select a preset"
                />
              </div>
            ) : null}
          </div>
          {showOptimizedPresetSelect ? (
            <p className="crosshook-help-text crosshook-launch-optimizations__preset-help">
              Built-in entries apply CrossHook&apos;s curated GPU sets under <code>[launch.presets.bundled/&lt;id&gt;]</code>.
              Saved names are profile presets; use &quot;Save preset&quot; to add more without editing TOML.
            </p>
          ) : null}
        </div>
      ) : null}

      {!isMethodSupported ? (
        <div className="crosshook-warning-banner crosshook-launch-optimizations__method-warning">
          Launch optimizations are only editable when the profile method is <code>proton_run</code> or{' '}
          <code>steam_applaunch</code>.
        </div>
      ) : null}

      <div className="crosshook-launch-optimizations__sections">
        <Tabs.Root
          value={optimizationSection}
          onValueChange={(value) => setOptimizationSection(value as 'recommended' | 'advanced')}
          className="crosshook-launch-optimizations__section-tabs"
        >
          <Tabs.List
            className="crosshook-launch-optimizations__section-tab-list"
            aria-label="Optimization detail level"
          >
            <Tabs.Trigger value="recommended" className="crosshook-launch-optimizations__section-tab-trigger">
              Recommended
            </Tabs.Trigger>
            <Tabs.Trigger value="advanced" className="crosshook-launch-optimizations__section-tab-trigger">
              <span className="crosshook-launch-optimizations__section-tab-trigger-inner">
                <span>Advanced</span>
                <span className="crosshook-launch-optimizations__section-tab-meta">
                  {formatCountLabel(advancedOptions.length, 'option', 'options')}
                  {enabledAdvancedCount > 0 ? (
                    <span className="crosshook-launch-optimizations__section-tab-meta-badge">
                      {enabledAdvancedCount} on
                    </span>
                  ) : null}
                </span>
              </span>
            </Tabs.Trigger>
          </Tabs.List>

          <div className="crosshook-launch-optimizations__section-tab-panels">
            <Tabs.Content value="recommended" className="crosshook-launch-optimizations__section-tab-panel">
              <p className="crosshook-launch-optimizations__section-copy">
                Common launch fixes for controller handling, overlays, and windowing.
              </p>
              <div className="crosshook-launch-optimizations__group-list">
                {commonGroups.map((group) => (
                  <OptionGroup
                    key={group.category}
                    group={group}
                    enabledIds={enabledIdSet}
                    selectedConflicts={selectedConflicts}
                    isMethodSupported={isMethodSupported}
                    method={method}
                    onToggleOption={onToggleOption}
                    tooltipIdPrefix={tooltipIdPrefix}
                    tooltipId={tooltipId}
                    setTooltipId={setTooltipId}
                    sectionTone="default"
                    optionsById={optionsById}
                    conflictMatrix={conflictMatrix}
                  />
                ))}
              </div>
            </Tabs.Content>

            <Tabs.Content
              value="advanced"
              className={joinClasses(
                'crosshook-launch-optimizations__section-tab-panel',
                'crosshook-launch-optimizations__section-tab-panel--advanced'
              )}
            >
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
                    method={method}
                    onToggleOption={onToggleOption}
                    tooltipIdPrefix={tooltipIdPrefix}
                    tooltipId={tooltipId}
                    setTooltipId={setTooltipId}
                    sectionTone="advanced"
                    optionsById={optionsById}
                    conflictMatrix={conflictMatrix}
                  />
                ))}
              </div>
            </Tabs.Content>
          </div>
        </Tabs.Root>
      </div>
    </section>
  );
}

export default LaunchOptimizationsPanel;
