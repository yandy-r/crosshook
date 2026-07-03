import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { DetectedModCandidate, DetectionScanReport, ProfileModInput } from '@/types/mods';
import { ModDetectionReviewModal } from '../mods/ModDetectionReviewModal';

const onRegister = vi.fn<(inputs: ProfileModInput[]) => Promise<{ name: string; error: string }[]>>();
const onClose = vi.fn();

function buildCandidate(overrides: Partial<DetectedModCandidate> = {}): DetectedModCandidate {
  return {
    detector_id: 'reshade',
    suggested_name: 'ReShade',
    category: 'overlay_injection',
    matched_paths: ['ReShade.ini', 'dxgi.dll'],
    already_registered: false,
    ...overrides,
  };
}

function buildReport(overrides: Partial<DetectionScanReport> = {}): DetectionScanReport {
  return {
    scanned_root: '/games/elden-ring',
    candidates: [
      buildCandidate(),
      buildCandidate({
        detector_id: 'enb',
        suggested_name: 'ENB Series',
        matched_paths: ['enbseries'],
        already_registered: true,
      }),
    ],
    entries_scanned: 12,
    truncated: false,
    ...overrides,
  };
}

function renderModal(report: DetectionScanReport = buildReport()) {
  return render(<ModDetectionReviewModal report={report} onRegister={onRegister} onClose={onClose} />);
}

describe('ModDetectionReviewModal', () => {
  beforeEach(() => {
    onRegister.mockReset();
    onRegister.mockResolvedValue([]);
    onClose.mockReset();
  });

  it('pre-checks candidates except already registered', () => {
    renderModal();

    expect(screen.getByRole('checkbox', { name: /ReShade/ })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: /ENB Series/ })).not.toBeChecked();
    expect(screen.getByRole('button', { name: 'Register 1 selected' })).toBeEnabled();
  });

  it('already registered checkbox disabled', () => {
    renderModal();

    expect(screen.getByRole('checkbox', { name: /ENB Series/ })).toBeDisabled();
    expect(screen.getByText('Already registered')).toBeInTheDocument();
  });

  it('zero candidate copy', () => {
    renderModal(buildReport({ candidates: [] }));

    expect(screen.getByText('No known mod artifacts were found in the game directory.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Register 0 selected' })).toBeDisabled();
  });

  it('truncated notice', () => {
    renderModal(buildReport({ truncated: true }));

    expect(
      screen.getByText(
        'Scan stopped at the entry budget — deep mod folders may be missing. Register anything missing manually.'
      )
    ).toBeInTheDocument();
  });

  it('register uses detected provenance inputs', async () => {
    renderModal();

    fireEvent.click(screen.getByRole('button', { name: 'Register 1 selected' }));

    await waitFor(() => expect(onRegister).toHaveBeenCalledTimes(1));
    expect(onRegister.mock.calls[0][0]).toEqual([
      {
        name: 'ReShade',
        category: 'overlay_injection',
        paths: ['ReShade.ini', 'dxgi.dll'],
        enabled: true,
        source_url: undefined,
        provenance: 'detected',
      },
    ]);
    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
  });

  it('partial failure keeps failed candidates listed', async () => {
    const report = buildReport({
      candidates: [
        buildCandidate(),
        buildCandidate({
          detector_id: 'script_extender:skse64',
          suggested_name: 'SKSE64',
          category: 'script_extender',
          matched_paths: ['skse64_loader.exe'],
        }),
      ],
    });
    onRegister.mockResolvedValue([{ name: 'SKSE64', error: 'already registered for this profile' }]);
    renderModal(report);

    fireEvent.click(screen.getByRole('button', { name: 'Register 2 selected' }));

    expect(await screen.findByText('SKSE64: already registered for this profile')).toBeInTheDocument();
    expect(screen.getByRole('checkbox', { name: /SKSE64/ })).toBeChecked();
    expect(screen.getByRole('checkbox', { name: /ReShade/ })).not.toBeChecked();
    expect(onClose).not.toHaveBeenCalled();
  });

  it('escape closes and clears report', () => {
    renderModal();

    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' });
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
