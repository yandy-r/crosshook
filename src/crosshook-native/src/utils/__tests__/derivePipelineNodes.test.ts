import { describe, expect, it } from 'vitest';
import { makeLaunchPreview } from '@/test/fixtures';
import { LaunchPhase } from '@/types/launch';
import { derivePipelineNodes } from '../derivePipelineNodes';

describe('derivePipelineNodes omitNodes', () => {
  it('removes the omitted nodes from the output', () => {
    const nodes = derivePipelineNodes('proton_run', null, null, LaunchPhase.Idle, {
      omitNodes: ['trainer', 'optimizations'],
    });

    expect(nodes.map((node) => node.id)).toEqual(['game', 'wine-prefix', 'proton', 'launch']);
  });

  it('launch node ignores omitted nodes when aggregating', () => {
    const preview = makeLaunchPreview();

    const withOmit = derivePipelineNodes('proton_run', null, preview, LaunchPhase.Idle, {
      omitNodes: ['trainer', 'optimizations'],
    });
    const launchWithOmit = withOmit.find((node) => node.id === 'launch');
    expect(launchWithOmit?.status).toBe('configured');
    expect(launchWithOmit?.detail).toBe('Command ready');

    // Without the filter, the unresolved trainer tier-2 node pins Launch to
    // "Complete steps above" — the regression the pre-loop filter prevents.
    const withoutOmit = derivePipelineNodes('proton_run', null, preview, LaunchPhase.Idle);
    const launchWithoutOmit = withoutOmit.find((node) => node.id === 'launch');
    expect(launchWithoutOmit?.status).toBe('not-configured');
    expect(launchWithoutOmit?.detail).toBe('Complete steps above');
  });
});

describe('derivePipelineNodes null profile', () => {
  it('yields tier-1 not-configured nodes without a preview', () => {
    const nodes = derivePipelineNodes('proton_run', null, null, LaunchPhase.Idle);

    expect(nodes).toHaveLength(6);
    for (const node of nodes) {
      expect(node.status).toBe('not-configured');
    }
  });

  it('does not affect tier-2 derivation when a preview is set', () => {
    const nodes = derivePipelineNodes('proton_run', null, makeLaunchPreview(), LaunchPhase.Idle);
    const game = nodes.find((node) => node.id === 'game');
    const winePrefix = nodes.find((node) => node.id === 'wine-prefix');

    expect(game?.status).toBe('configured');
    expect(game?.detail).toBe('game.exe');
    expect(winePrefix?.status).toBe('configured');
  });
});

describe('derivePipelineNodes phase overlay', () => {
  it('still marks the game node active under GameLaunching with omitted nodes', () => {
    const nodes = derivePipelineNodes('proton_run', null, makeLaunchPreview(), LaunchPhase.GameLaunching, {
      omitNodes: ['trainer', 'optimizations'],
    });

    expect(nodes.find((node) => node.id === 'game')?.status).toBe('active');
    expect(nodes.map((node) => node.id)).toEqual(['game', 'wine-prefix', 'proton', 'launch']);
  });
});
