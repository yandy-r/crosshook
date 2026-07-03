import { describe, expect, it } from 'vitest';
import type { LaunchValidationIssue } from '@/types/launch';
import { mapValidationToNode } from '../mapValidationToNode';

function issue(code?: string): LaunchValidationIssue {
  return { message: 'message', help: 'help', severity: 'info', code };
}

describe('mapValidationToNode', () => {
  it('mod_coexistence codes map to trainer node', () => {
    const codes = [
      'mod_coexistence_injection_vector',
      'mod_coexistence_overlay_conflict',
      'mod_coexistence_file_replacement_notice',
      'mod_coexistence_script_extender_launch_order',
      'mod_coexistence_registered_path_missing',
    ];
    for (const code of codes) {
      expect(mapValidationToNode(issue(code))).toBe('trainer');
    }
  });

  it('trainer_hash still maps to trainer', () => {
    expect(mapValidationToNode(issue('trainer_hash_mismatch'))).toBe('trainer');
  });

  it('codeless issue maps to launch', () => {
    expect(mapValidationToNode(issue())).toBe('launch');
    expect(mapValidationToNode(issue('   '))).toBe('launch');
  });
});
