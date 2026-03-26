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

interface ThemedSelectProps {
  value: string;
  onValueChange: (value: string) => void;
  options: SelectOption[];
  placeholder?: string;
  id?: string;
  className?: string;
}

export function ThemedSelect({
  value,
  onValueChange,
  options,
  placeholder = 'Select\u2026',
  id,
  className,
}: ThemedSelectProps) {
  const radixValue = toRadix(value);
  const hasValue = options.some((o) => o.value === value);

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
            {options.map((opt) => (
              <Select.Item
                key={opt.value}
                value={toRadix(opt.value)}
                disabled={opt.disabled}
                className="crosshook-themed-select__item"
              >
                <Select.ItemText>{opt.label}</Select.ItemText>
                <Select.ItemIndicator className="crosshook-themed-select__check">
                  <CheckIcon />
                </Select.ItemIndicator>
              </Select.Item>
            ))}
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
