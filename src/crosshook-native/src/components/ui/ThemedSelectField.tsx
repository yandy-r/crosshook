import { type ReactNode, useId } from 'react';

import type { SelectOption, SelectOptionGroup } from './ThemedSelect';
import { ThemedSelect } from './ThemedSelect';

export interface ThemedSelectFieldProps {
  /** Label content rendered inside a `<label>` element. */
  label: ReactNode;
  value: string;
  onValueChange: (value: string) => void;
  options?: SelectOption[];
  groups?: SelectOptionGroup[];
  pinnedValues?: ReadonlySet<string>;
  onTogglePin?: (value: string) => void;
  placeholder?: string;
  /** Additional className forwarded to the `ThemedSelect` trigger. */
  className?: string;
  /** When true, the select cannot be opened (non-interactive). */
  disabled?: boolean;
  /**
   * When true, renders the label with `crosshook-visually-hidden` so it is
   * accessible to assistive technology but not visually shown.
   */
  visuallyHiddenLabel?: boolean;
}

/**
 * Convenience wrapper that pairs a `<label>` with a `<ThemedSelect>`,
 * automatically wiring `htmlFor` / `id` / `aria-labelledby` so call-sites
 * do not have to manage their own id strings.
 *
 * The component renders a fragment (no wrapper div). Place it inside whatever
 * layout container the call-site uses (e.g. `crosshook-field`,
 * `crosshook-settings-field-row`, etc.).
 */
export function ThemedSelectField({
  label,
  value,
  onValueChange,
  options,
  groups,
  pinnedValues,
  onTogglePin,
  placeholder,
  className,
  disabled,
  visuallyHiddenLabel = false,
}: ThemedSelectFieldProps) {
  const selectId = useId();
  const labelId = useId();

  return (
    <>
      <label
        id={labelId}
        htmlFor={selectId}
        className={`crosshook-label${visuallyHiddenLabel ? ' crosshook-visually-hidden' : ''}`}
      >
        {label}
      </label>
      <ThemedSelect
        id={selectId}
        ariaLabelledby={labelId}
        value={value}
        onValueChange={onValueChange}
        options={options}
        groups={groups}
        pinnedValues={pinnedValues}
        onTogglePin={onTogglePin}
        placeholder={placeholder}
        className={className}
        disabled={disabled}
      />
    </>
  );
}

export default ThemedSelectField;
