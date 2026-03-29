import { Fragment } from 'react';
import * as Select from '@radix-ui/react-select';

/**
 * Radix Select does not allow empty-string values; map them to a sentinel.
 * Values matching this literal must not appear in `SelectOption.value`.
 */
const EMPTY = '__empty__';
const toRadix = (v: string) => (v === '' ? EMPTY : v);
const fromRadix = (v: string) => (v === EMPTY ? '' : v);

export interface SelectOption {
  value: string;
  label: string;
  disabled?: boolean;
}

export interface SelectOptionGroup {
  label: string;
  options: SelectOption[];
}

interface ThemedSelectProps {
  value: string;
  onValueChange: (value: string) => void;
  options?: SelectOption[];
  groups?: SelectOptionGroup[];
  pinnedValues?: ReadonlySet<string>;
  onTogglePin?: (value: string) => void;
  placeholder?: string;
  id?: string;
  className?: string;
}

function SelectItemNode({
  opt,
  isPinned,
  onTogglePin,
}: {
  opt: SelectOption;
  isPinned?: boolean;
  onTogglePin?: (value: string) => void;
}) {
  return (
    <Select.Item
      value={toRadix(opt.value)}
      disabled={opt.disabled}
      className="crosshook-themed-select__item"
    >
      <Select.ItemText>{opt.label}</Select.ItemText>
      {onTogglePin ? (
        <span
          role="button"
          tabIndex={-1}
          className={`crosshook-themed-select__pin${isPinned ? ' crosshook-themed-select__pin--active' : ''}`}
          aria-label={isPinned ? `Unpin ${opt.label}` : `Pin ${opt.label}`}
          onPointerDown={(e) => { e.stopPropagation(); e.preventDefault(); }}
          onPointerUp={(e) => { e.stopPropagation(); e.preventDefault(); }}
          onClick={(e) => {
            e.stopPropagation();
            e.preventDefault();
            onTogglePin(opt.value);
          }}
        >
          {isPinned ? '\u2605' : '\u2606'}
        </span>
      ) : (
        <Select.ItemIndicator className="crosshook-themed-select__check">
          <CheckIcon />
        </Select.ItemIndicator>
      )}
    </Select.Item>
  );
}

export function ThemedSelect({
  value,
  onValueChange,
  options,
  groups,
  pinnedValues,
  onTogglePin,
  placeholder = 'Select\u2026',
  id,
  className,
}: ThemedSelectProps) {
  const allOptions = groups && groups.length > 0
    ? groups.flatMap((g) => g.options)
    : (options ?? []);
  const radixValue = toRadix(value);
  const hasValue = allOptions.some((o) => o.value === value);

  return (
    <Select.Root
      value={hasValue ? radixValue : undefined}
      onValueChange={(v) => onValueChange(fromRadix(v))}
    >
      <Select.Trigger
        id={id}
        className={`crosshook-themed-select__trigger ${className ?? ''}`.trim()}
      >
        <Select.Value placeholder={placeholder} />
        <Select.Icon className="crosshook-themed-select__icon">
          <ChevronIcon />
        </Select.Icon>
      </Select.Trigger>

      <Select.Portal>
        <Select.Content
          className="crosshook-themed-select__content"
          position="popper"
          sideOffset={4}
          align="start"
        >
          <Select.ScrollUpButton className="crosshook-themed-select__scroll-btn">
            <ChevronUpIcon />
          </Select.ScrollUpButton>

          <Select.Viewport className="crosshook-themed-select__viewport">
            {groups && groups.length > 0
              ? groups.map((group, gi) => (
                  <Fragment key={group.label}>
                    {gi > 0 && <Select.Separator className="crosshook-themed-select__separator" />}
                    <Select.Group>
                      <Select.Label className="crosshook-themed-select__group-label">
                        {group.label}
                      </Select.Label>
                      {group.options.map((opt) => (
                        <SelectItemNode key={opt.value} opt={opt} isPinned={pinnedValues?.has(opt.value)} onTogglePin={onTogglePin} />
                      ))}
                    </Select.Group>
                  </Fragment>
                ))
              : (options ?? []).map((opt) => (
                  <SelectItemNode key={opt.value} opt={opt} isPinned={pinnedValues?.has(opt.value)} onTogglePin={onTogglePin} />
                ))
            }
          </Select.Viewport>

          <Select.ScrollDownButton className="crosshook-themed-select__scroll-btn">
            <ChevronIcon />
          </Select.ScrollDownButton>
        </Select.Content>
      </Select.Portal>
    </Select.Root>
  );
}

function ChevronIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3.5 5.25 7 8.75l3.5-3.5" />
    </svg>
  );
}

function ChevronUpIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M10.5 8.75 7 5.25l-3.5 3.5" />
    </svg>
  );
}

function CheckIcon() {
  return (
    <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3 7.5 5.5 10 11 4" />
    </svg>
  );
}

export default ThemedSelect;
