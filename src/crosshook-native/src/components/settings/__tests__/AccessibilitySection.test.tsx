import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { DEFAULT_APP_SETTINGS } from '@/types/settings';
import { AccessibilitySection } from '../AccessibilitySection';
import { LoggingAndUiSection } from '../LoggingAndUiSection';

function renderSection(overrides: Partial<typeof DEFAULT_APP_SETTINGS> = {}) {
  const onPersistSettings = vi.fn().mockResolvedValue(undefined);
  render(
    <AccessibilitySection settings={{ ...DEFAULT_APP_SETTINGS, ...overrides }} onPersistSettings={onPersistSettings} />
  );
  return { onPersistSettings };
}

async function selectOption(fieldName: string, optionName: string): Promise<void> {
  const user = userEvent.setup();
  await user.click(screen.getByRole('combobox', { name: fieldName }));
  await user.click(await screen.findByRole('option', { name: optionName }));
}

describe('AccessibilitySection', () => {
  it('renders both themed selects with current values', () => {
    renderSection();

    expect(screen.getByRole('combobox', { name: 'High contrast' })).toHaveTextContent(
      'Auto — follow system contrast preference'
    );
    expect(screen.getByRole('combobox', { name: 'Motion' })).toHaveTextContent(
      'Auto — follow system reduced-motion preference'
    );
  });

  it('persists high contrast patch on change', async () => {
    const { onPersistSettings } = renderSection();

    await selectOption('High contrast', 'On — always high contrast');

    expect(onPersistSettings).toHaveBeenCalledTimes(1);
    expect(onPersistSettings).toHaveBeenCalledWith({ high_contrast: 'on' });
  });

  it('persists reduced motion patch on change', async () => {
    const { onPersistSettings } = renderSection();

    await selectOption('Motion', 'Reduced — minimize animation');

    expect(onPersistSettings).toHaveBeenCalledTimes(1);
    expect(onPersistSettings).toHaveBeenCalledWith({ reduced_motion: 'reduced' });
  });

  it('does not persist when the selected value is unchanged', async () => {
    const { onPersistSettings } = renderSection();

    await selectOption('High contrast', 'Auto — follow system contrast preference');
    await selectOption('Motion', 'Auto — follow system reduced-motion preference');

    expect(onPersistSettings).not.toHaveBeenCalled();
  });

  it('shows resolved-state note for manual override', () => {
    // Global matchMedia mock reports every query as non-matching.
    renderSection({ high_contrast: 'on', reduced_motion: 'auto' });

    expect(screen.getByText('Currently: high contrast on')).toBeInTheDocument();
    expect(screen.queryByText(/high contrast on \(from system preference\)/)).not.toBeInTheDocument();
    expect(screen.getByText('Currently: full motion (from system preference)')).toBeInTheDocument();
  });
});

describe('LoggingAndUiSection', () => {
  it('no longer renders a high-contrast checkbox', () => {
    render(
      <LoggingAndUiSection settings={DEFAULT_APP_SETTINGS} onPersistSettings={vi.fn().mockResolvedValue(undefined)} />
    );

    expect(screen.queryByRole('checkbox', { name: /high.contrast/i })).not.toBeInTheDocument();
    expect(screen.queryByText(/high.contrast/i)).not.toBeInTheDocument();
  });
});
