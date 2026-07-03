import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ProfileModInput, ProfileModRecord } from '@/types/mods';
import { ModForm } from '../mods/ModForm';

const onSubmit = vi.fn<(input: ProfileModInput) => Promise<void>>();
const onCancel = vi.fn();

function buildRecord(overrides: Partial<ProfileModRecord> = {}): ProfileModRecord {
  return {
    mod_id: 'mod-1',
    profile_id: 'profile-1',
    name: 'ReShade',
    category: 'overlay_injection',
    paths: ['dxgi.dll'],
    enabled: true,
    provenance: 'detected',
    created_at: '2026-01-01T00:00:00+00:00',
    updated_at: '2026-01-01T00:00:00+00:00',
    ...overrides,
  };
}

function renderForm(initial?: ProfileModRecord) {
  return render(
    <ModForm
      initial={initial}
      gameExecutablePath="/games/elden-ring/eldenring.exe"
      onSubmit={onSubmit}
      onCancel={onCancel}
    />
  );
}

function submitForm(container: HTMLElement) {
  fireEvent.submit(container.querySelector('form') as HTMLFormElement);
}

describe('ModForm', () => {
  beforeEach(() => {
    onSubmit.mockReset();
    onSubmit.mockResolvedValue(undefined);
    onCancel.mockReset();
  });

  it('requires name', async () => {
    const { container } = renderForm();

    submitForm(container);

    expect(await screen.findByRole('alert')).toHaveTextContent('Mod name is required.');
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it('rejects non-http source url', async () => {
    const { container } = renderForm();

    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'SKSE64' } });
    fireEvent.change(screen.getByLabelText('Source URL (optional)'), {
      target: { value: 'ftp://example.com/mod' },
    });
    submitForm(container);

    expect(await screen.findByRole('alert')).toHaveTextContent('Source URL must start with http:// or https://.');
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it('adds and removes path rows', () => {
    renderForm();

    expect(screen.getByLabelText('Path 1')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Add path' }));
    expect(screen.getByLabelText('Path 2')).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText('Path 2'), { target: { value: 'keep-me.dll' } });
    fireEvent.click(screen.getAllByRole('button', { name: 'Remove' })[0]);
    expect(screen.queryByLabelText('Path 2')).toBeNull();
    expect(screen.getByLabelText('Path 1')).toHaveValue('keep-me.dll');
  });

  it('drops empty path rows on submit', async () => {
    const { container } = renderForm();

    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'SKSE64' } });
    fireEvent.change(screen.getByLabelText('Path 1'), { target: { value: '  a.dll  ' } });
    fireEvent.click(screen.getByRole('button', { name: 'Add path' }));
    submitForm(container);

    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
    expect(onSubmit.mock.calls[0][0].paths).toEqual(['a.dll']);
  });

  it('submit payload uses snake_case input fields', async () => {
    const { container } = renderForm();

    fireEvent.change(screen.getByLabelText('Name'), { target: { value: '  SKSE64  ' } });
    fireEvent.change(screen.getByLabelText('Source URL (optional)'), {
      target: { value: 'https://skse.silverlock.org' },
    });
    submitForm(container);

    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
    const payload = onSubmit.mock.calls[0][0];
    expect(payload).toEqual({
      name: 'SKSE64',
      category: 'other',
      paths: [],
      enabled: true,
      source_url: 'https://skse.silverlock.org',
      provenance: 'manual',
    });
    expect(Object.keys(payload)).toContain('source_url');
  });

  it('edit preserves detected provenance', async () => {
    const { container } = renderForm(buildRecord({ provenance: 'detected' }));

    expect(screen.getByRole('button', { name: 'Save mod' })).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText('Name'), { target: { value: 'ReShade 6' } });
    submitForm(container);

    await waitFor(() => expect(onSubmit).toHaveBeenCalledTimes(1));
    expect(onSubmit.mock.calls[0][0].provenance).toBe('detected');
    expect(onSubmit.mock.calls[0][0].name).toBe('ReShade 6');
  });
});
