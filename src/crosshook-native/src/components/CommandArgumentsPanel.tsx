import * as Tabs from '@radix-ui/react-tabs';
import { type ChangeEvent, useEffect, useId, useMemo, useState } from 'react';
import type { LaunchMethod } from '../types';
import type {
  CommandArgumentCatalogPayload,
  CommandArgumentEntry,
  LaunchCommandArguments,
} from '../types/launch-command-arguments';
import { buildArgumentsById, buildConflictMatrix } from '../utils/command-argument-catalog';
import { formatCountLabel, joinClasses } from './launch-optimizations/utils';
import '../styles/launch-pipeline.css';

const MAX_COMMAND_ARGUMENT_TOKEN_LEN = 512;

const COMMAND_ARGUMENT_CATEGORIES = ['graphics', 'compatibility'] as const;

const COMMAND_ARGUMENT_CATEGORY_LABELS: Record<(typeof COMMAND_ARGUMENT_CATEGORIES)[number], string> = {
  graphics: 'Graphics',
  compatibility: 'Compatibility',
};

export interface CommandArgumentsPanelProps {
  method: LaunchMethod;
  commandArguments: LaunchCommandArguments;
  catalog: CommandArgumentCatalogPayload | null;
  onToggleArgument: (argumentId: string, nextEnabled: boolean) => void;
  onUpdateCustomArgs: (customArgs: readonly string[]) => void;
  className?: string;
}

interface CommandArgumentConflict {
  argumentId: string;
  conflictsWith: string;
}

interface GroupedArguments {
  category: (typeof COMMAND_ARGUMENT_CATEGORIES)[number];
  entries: CommandArgumentEntry[];
}

type CustomArgRow = { id: string; value: string };

function groupArguments(entries: readonly CommandArgumentEntry[]): GroupedArguments[] {
  return COMMAND_ARGUMENT_CATEGORIES.map((category) => ({
    category,
    entries: entries.filter((entry) => entry.category === category),
  })).filter((group) => group.entries.length > 0);
}

function findCommandArgumentConflicts(
  enabledIds: readonly string[],
  conflictMatrix: Readonly<Record<string, readonly string[]>>
): CommandArgumentConflict[] {
  const conflicts: CommandArgumentConflict[] = [];
  const seen = new Set<string>();

  for (const argumentId of enabledIds) {
    for (const conflictsWith of conflictMatrix[argumentId] ?? []) {
      if (!enabledIds.includes(conflictsWith)) {
        continue;
      }
      const key = [argumentId, conflictsWith].sort().join('|');
      if (seen.has(key)) {
        continue;
      }
      seen.add(key);
      conflicts.push({ argumentId, conflictsWith });
    }
  }

  return conflicts;
}

function getConflictingEnabledIds(
  argumentId: string,
  enabledIds: readonly string[],
  conflictMatrix: Readonly<Record<string, readonly string[]>>
): string[] {
  return (conflictMatrix[argumentId] ?? []).filter((id) => enabledIds.includes(id));
}

function getConflictLabels(
  entry: CommandArgumentEntry,
  argumentsById: Record<string, CommandArgumentEntry>,
  conflictMatrix: Readonly<Record<string, readonly string[]>>
): string[] {
  return (conflictMatrix[entry.id] ?? [])
    .map((conflictId) => argumentsById[conflictId]?.label)
    .filter((label): label is string => Boolean(label));
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
  conflict: CommandArgumentConflict,
  argumentsById: Record<string, CommandArgumentEntry>
): string {
  return `${argumentsById[conflict.argumentId]?.label ?? conflict.argumentId} conflicts with ${argumentsById[conflict.conflictsWith]?.label ?? conflict.conflictsWith}.`;
}

function customArgRowError(value: string): string | null {
  if (value.length === 0) {
    return null;
  }
  if (value.trim().length === 0) {
    return 'Custom argument tokens cannot be empty or whitespace-only.';
  }
  if ([...value].some((character) => character.charCodeAt(0) < 32 || character.charCodeAt(0) === 127)) {
    return 'Custom argument tokens cannot contain control characters.';
  }
  if (value.length > MAX_COMMAND_ARGUMENT_TOKEN_LEN) {
    return `Custom argument tokens must be ${MAX_COMMAND_ARGUMENT_TOKEN_LEN} characters or fewer.`;
  }
  return null;
}

function customArgsToRows(customArgs: readonly string[]): CustomArgRow[] {
  return customArgs.map((value) => ({ id: crypto.randomUUID(), value }));
}

function rowsToCustomArgs(rows: readonly CustomArgRow[]): string[] {
  return rows.map((row) => row.value);
}

function rowsSignature(rows: readonly CustomArgRow[]): string {
  return JSON.stringify(rows.map((row) => row.value));
}

interface ArgumentGroupProps {
  group: GroupedArguments;
  enabledIds: Set<string>;
  selectedConflicts: readonly CommandArgumentConflict[];
  isMethodSupported: boolean;
  method: LaunchMethod;
  onToggleArgument: (argumentId: string, nextEnabled: boolean) => void;
  tooltipIdPrefix: string;
  tooltipId: string | null;
  setTooltipId: (argumentId: string | null) => void;
  sectionTone: 'default' | 'advanced';
  argumentsById: Record<string, CommandArgumentEntry>;
  conflictMatrix: Readonly<Record<string, readonly string[]>>;
}

function ArgumentGroup({
  group,
  enabledIds,
  selectedConflicts,
  isMethodSupported,
  method,
  onToggleArgument,
  tooltipIdPrefix,
  tooltipId,
  setTooltipId,
  sectionTone,
  argumentsById,
  conflictMatrix,
}: ArgumentGroupProps) {
  const groupArgumentIds = group.entries.map((entry) => entry.id);
  const groupConflicts = selectedConflicts.filter(
    (conflict) => groupArgumentIds.includes(conflict.argumentId) || groupArgumentIds.includes(conflict.conflictsWith)
  );

  return (
    <fieldset
      className={joinClasses(
        'crosshook-launch-optimizations__group',
        `crosshook-launch-optimizations__group--${sectionTone}`
      )}
    >
      <legend className="crosshook-launch-optimizations__group-title">
        {COMMAND_ARGUMENT_CATEGORY_LABELS[group.category]}
      </legend>
      {groupConflicts.length > 0 ? (
        <div className="crosshook-warning-banner crosshook-launch-optimizations__group-warning">
          {groupConflicts.map((conflict) => formatConflictSummary(conflict, argumentsById)).join(' ')}
        </div>
      ) : null}
      <div className="crosshook-launch-optimizations__option-list">
        {group.entries.map((entry) => {
          const isEnabled = enabledIds.has(entry.id);
          const isTooltipOpen = tooltipId === entry.id;
          const conflictingIds = getConflictingEnabledIds(
            entry.id,
            [...enabledIds].filter((enabledId) => enabledId !== entry.id),
            conflictMatrix
          );
          const blockedByLabels = conflictingIds.map(
            (conflictingId) => argumentsById[conflictingId]?.label ?? conflictingId
          );
          const isBlockedByConflict = !isEnabled && blockedByLabels.length > 0;
          const isApplicable = entry.applicable_methods.includes(method);
          const isSupported = isMethodSupported && isApplicable && !isBlockedByConflict;
          const checkboxId = `${tooltipIdPrefix}-${entry.id}`;
          const tooltipIdValue = `${tooltipIdPrefix}-${entry.id}-tooltip`;
          const conflictLabels = getConflictLabels(entry, argumentsById, conflictMatrix);
          const rowDisabled = !isSupported;

          return (
            <div
              key={entry.id}
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
                  onChange={(event) => onToggleArgument(entry.id, event.target.checked)}
                />

                <div className="crosshook-launch-optimizations__option-copy">
                  <label className="crosshook-launch-optimizations__option-label" htmlFor={checkboxId}>
                    {entry.label}
                  </label>
                  <p className="crosshook-launch-optimizations__option-description">{entry.description}</p>
                  <div className="crosshook-launch-optimizations__option-meta">
                    <span
                      className={joinClasses(
                        'crosshook-launch-optimizations__option-pill',
                        entry.advanced && 'crosshook-launch-optimizations__option-pill--advanced',
                        !entry.advanced && 'crosshook-launch-optimizations__option-pill--recommended'
                      )}
                    >
                      {entry.advanced ? 'Advanced' : 'Recommended'}
                    </span>
                    {entry.community ? (
                      <span className="crosshook-launch-optimizations__option-pill crosshook-launch-optimizations__option-pill--community">
                        Community
                      </span>
                    ) : null}
                    {entry.tokens.length > 0 ? (
                      <span className="crosshook-command-arguments__token-pill">{entry.tokens.join(' ')}</span>
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
                onMouseEnter={() => setTooltipId(entry.id)}
                onMouseLeave={() => setTooltipId(null)}
              >
                <button
                  type="button"
                  className="crosshook-launch-optimizations__info-button"
                  aria-label={`More information about ${entry.label}`}
                  aria-expanded={isTooltipOpen}
                  aria-describedby={isTooltipOpen ? tooltipIdValue : undefined}
                  onFocus={() => setTooltipId(entry.id)}
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
                  <p className="crosshook-launch-optimizations__tooltip-title">{entry.label}</p>
                  <p className="crosshook-launch-optimizations__tooltip-kicker">What it does</p>
                  <p className="crosshook-launch-optimizations__tooltip-copy">{entry.description}</p>
                  <p className="crosshook-launch-optimizations__tooltip-kicker">When it helps</p>
                  <p className="crosshook-launch-optimizations__tooltip-copy">{entry.help_text}</p>
                  <p className="crosshook-launch-optimizations__tooltip-kicker">Argv tokens</p>
                  <p className="crosshook-launch-optimizations__tooltip-copy">
                    {entry.tokens.length > 0 ? entry.tokens.join(' ') : 'No fixed tokens'}
                  </p>
                  {blockedByLabels.length > 0 ? (
                    <p className="crosshook-launch-optimizations__tooltip-copy">
                      Selection is currently blocked by {formatConflictLabels(blockedByLabels)}.
                    </p>
                  ) : null}
                  {conflictLabels.length > 0 ? (
                    <p className="crosshook-launch-optimizations__tooltip-copy">
                      Conflict note: {conflictLabels.join(', ')} should not be enabled together with this argument.
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

export function CommandArgumentsPanel({
  method,
  commandArguments,
  catalog,
  onToggleArgument,
  onUpdateCustomArgs,
  className,
}: CommandArgumentsPanelProps) {
  const titleId = useId();
  const customArgsHelpId = useId();
  const tooltipIdPrefix = useId();
  const [tooltipId, setTooltipId] = useState<string | null>(null);
  const [argumentSection, setArgumentSection] = useState<'recommended' | 'advanced'>('recommended');
  const [customRows, setCustomRows] = useState<CustomArgRow[]>(() => customArgsToRows(commandArguments.custom_args));

  const customArgsSignature = useMemo(
    () => JSON.stringify(commandArguments.custom_args),
    [commandArguments.custom_args]
  );

  useEffect(() => {
    setCustomRows((currentRows) => {
      if (rowsSignature(currentRows) === customArgsSignature) {
        return currentRows;
      }
      return customArgsToRows(commandArguments.custom_args);
    });
  }, [customArgsSignature, commandArguments.custom_args]);

  const isMethodSupported = method === 'proton_run' || method === 'steam_applaunch';

  const argumentsById = useMemo(() => (catalog ? buildArgumentsById(catalog.entries) : {}), [catalog]);
  const conflictMatrix = useMemo(() => (catalog ? buildConflictMatrix(catalog.entries) : {}), [catalog]);

  if (!catalog) {
    return <div className="crosshook-command-arguments">Loading command arguments...</div>;
  }

  const seen = new Set<string>();
  const selectedArgumentIds = commandArguments.enabled_argument_ids.filter((argumentId) => {
    if (!argumentsById[argumentId] || seen.has(argumentId)) {
      return false;
    }
    seen.add(argumentId);
    return true;
  });
  const enabledIdSet = new Set(selectedArgumentIds);
  const selectedEntries = selectedArgumentIds.map((argumentId) => argumentsById[argumentId]);
  const selectedConflicts = findCommandArgumentConflicts(selectedArgumentIds, conflictMatrix);
  const commonEntries = catalog.entries.filter((entry) => !entry.advanced);
  const advancedEntries = catalog.entries.filter((entry) => entry.advanced);
  const commonGroups = groupArguments(commonEntries);
  const advancedGroups = groupArguments(advancedEntries);
  const enabledAdvancedCount = selectedEntries.filter((entry) => entry.advanced).length;
  const rootClassName = joinClasses('crosshook-command-arguments', className);

  const applyCustomRows = (nextRows: CustomArgRow[]) => {
    setCustomRows(nextRows);
    const hasErrors = nextRows.some((row) => customArgRowError(row.value) !== null);
    if (hasErrors) {
      return;
    }
    onUpdateCustomArgs(rowsToCustomArgs(nextRows));
  };

  const moveCustomRow = (rowId: string, direction: -1 | 1) => {
    const index = customRows.findIndex((row) => row.id === rowId);
    if (index < 0) {
      return;
    }
    const targetIndex = index + direction;
    if (targetIndex < 0 || targetIndex >= customRows.length) {
      return;
    }
    const nextRows = [...customRows];
    const [row] = nextRows.splice(index, 1);
    nextRows.splice(targetIndex, 0, row);
    applyCustomRows(nextRows);
  };

  return (
    <section className={rootClassName} aria-labelledby={titleId} data-method={method}>
      <div className="crosshook-command-arguments__header">
        <div className="crosshook-command-arguments__heading">
          <h2 id={titleId} className="crosshook-command-arguments__title">
            Command Arguments
          </h2>
          <p className="crosshook-help-text crosshook-command-arguments__intro">
            Curated game argv switches and ordered custom tokens appended after the executable for Proton and Steam
            launch options.
          </p>
        </div>

        <div className="crosshook-command-arguments__summary-row">
          <span className="crosshook-launch-optimizations__summary-chip">
            {formatCountLabel(selectedEntries.length, 'enabled argument', 'enabled arguments')}
          </span>
          <span className="crosshook-launch-optimizations__summary-chip">
            {formatCountLabel(
              customRows.filter((row) => row.value.trim().length > 0).length,
              'custom token',
              'custom tokens'
            )}
          </span>
        </div>
      </div>

      {!isMethodSupported ? (
        <div className="crosshook-warning-banner crosshook-command-arguments__method-warning">
          Command arguments are only editable when the profile method is <code>proton_run</code> or{' '}
          <code>steam_applaunch</code>.
        </div>
      ) : null}

      <div className="crosshook-command-arguments__sections">
        <Tabs.Root
          value={argumentSection}
          onValueChange={(value) => setArgumentSection(value as 'recommended' | 'advanced')}
          className="crosshook-launch-optimizations__section-tabs"
        >
          <Tabs.List
            className="crosshook-launch-optimizations__section-tab-list"
            aria-label="Command argument detail level"
          >
            <Tabs.Trigger value="recommended" className="crosshook-launch-optimizations__section-tab-trigger">
              Recommended
            </Tabs.Trigger>
            <Tabs.Trigger value="advanced" className="crosshook-launch-optimizations__section-tab-trigger">
              <span className="crosshook-launch-optimizations__section-tab-trigger-inner">
                <span>Advanced</span>
                <span className="crosshook-launch-optimizations__section-tab-meta">
                  {formatCountLabel(advancedEntries.length, 'argument', 'arguments')}
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
                Common renderer and launcher switches with conservative defaults.
              </p>
              <div className="crosshook-launch-optimizations__group-list">
                {commonGroups.map((group) => (
                  <ArgumentGroup
                    key={group.category}
                    group={group}
                    enabledIds={enabledIdSet}
                    selectedConflicts={selectedConflicts}
                    isMethodSupported={isMethodSupported}
                    method={method}
                    onToggleArgument={onToggleArgument}
                    tooltipIdPrefix={tooltipIdPrefix}
                    tooltipId={tooltipId}
                    setTooltipId={setTooltipId}
                    sectionTone="default"
                    argumentsById={argumentsById}
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
                Less common switches that may help specific titles but are not safe defaults for every game.
              </p>
              <div className="crosshook-launch-optimizations__group-list">
                {advancedGroups.map((group) => (
                  <ArgumentGroup
                    key={group.category}
                    group={group}
                    enabledIds={enabledIdSet}
                    selectedConflicts={selectedConflicts}
                    isMethodSupported={isMethodSupported}
                    method={method}
                    onToggleArgument={onToggleArgument}
                    tooltipIdPrefix={tooltipIdPrefix}
                    tooltipId={tooltipId}
                    setTooltipId={setTooltipId}
                    sectionTone="advanced"
                    argumentsById={argumentsById}
                    conflictMatrix={conflictMatrix}
                  />
                ))}
              </div>
            </Tabs.Content>
          </div>
        </Tabs.Root>
      </div>

      <div className="crosshook-command-arguments__custom">
        <div className="crosshook-command-arguments__custom-header">
          <h3 className="crosshook-command-arguments__custom-title">Custom tokens</h3>
          <p className="crosshook-help-text" id={customArgsHelpId}>
            Ordered argv tokens appended after curated arguments. Use one token per row; paths with spaces stay in a
            single row.
          </p>
        </div>

        {customRows.length === 0 ? (
          <p className="crosshook-help-text">No custom tokens configured for this profile.</p>
        ) : null}

        <div className="crosshook-command-arguments__custom-rows">
          {customRows.map((row, index) => {
            const rowErr = customArgRowError(row.value);
            const rowErrorId = `${tooltipIdPrefix}-custom-arg-err-${row.id}`;
            const inputId = `${tooltipIdPrefix}-custom-arg-${row.id}`;
            const canMoveUp = index > 0;
            const canMoveDown = index < customRows.length - 1;

            return (
              <div key={row.id} className="crosshook-command-arguments__custom-row">
                <div className="crosshook-command-arguments__custom-row-main">
                  <span className="crosshook-command-arguments__custom-row-index" aria-hidden="true">
                    {index + 1}
                  </span>
                  <div className="crosshook-field crosshook-command-arguments__custom-field">
                    <label className="crosshook-label" htmlFor={inputId}>
                      Token
                    </label>
                    <input
                      id={inputId}
                      className="crosshook-input"
                      value={row.value}
                      placeholder="--nolauncher"
                      disabled={!isMethodSupported}
                      aria-invalid={Boolean(rowErr)}
                      aria-describedby={
                        [rowErr ? rowErrorId : null, customArgsHelpId].filter(Boolean).join(' ') || undefined
                      }
                      onChange={(event: ChangeEvent<HTMLInputElement>) => {
                        const nextValue = event.target.value;
                        applyCustomRows(
                          customRows.map((current) =>
                            current.id === row.id ? { ...current, value: nextValue } : current
                          )
                        );
                      }}
                    />
                  </div>
                  <div className="crosshook-command-arguments__custom-row-actions">
                    <button
                      type="button"
                      className="crosshook-button crosshook-button--secondary crosshook-button--small"
                      aria-label={`Move custom token ${index + 1} up`}
                      disabled={!isMethodSupported || !canMoveUp}
                      onClick={() => moveCustomRow(row.id, -1)}
                    >
                      Up
                    </button>
                    <button
                      type="button"
                      className="crosshook-button crosshook-button--secondary crosshook-button--small"
                      aria-label={`Move custom token ${index + 1} down`}
                      disabled={!isMethodSupported || !canMoveDown}
                      onClick={() => moveCustomRow(row.id, 1)}
                    >
                      Down
                    </button>
                    <button
                      type="button"
                      className="crosshook-button crosshook-button--secondary crosshook-button--small"
                      aria-label={`Remove custom token ${index + 1}`}
                      disabled={!isMethodSupported}
                      onClick={() => {
                        applyCustomRows(customRows.filter((current) => current.id !== row.id));
                      }}
                    >
                      Remove
                    </button>
                  </div>
                </div>
                {rowErr ? (
                  <p id={rowErrorId} className="crosshook-danger" role="alert">
                    {rowErr}
                  </p>
                ) : null}
              </div>
            );
          })}
        </div>

        <button
          type="button"
          className="crosshook-button crosshook-button--secondary crosshook-command-arguments__custom-add"
          disabled={!isMethodSupported}
          onClick={() => applyCustomRows([...customRows, { id: crypto.randomUUID(), value: '' }])}
        >
          Add token
        </button>
      </div>
    </section>
  );
}

export default CommandArgumentsPanel;
