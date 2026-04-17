import type { Capability } from '../types/onboarding';

/**
 * First non-empty docs URL from missing required/optional tool rows (required first).
 */
export function getFirstCapabilityDocsUrl(capability: Capability | undefined | null): string | null {
  if (capability == null) {
    return null;
  }

  const missingTools = [...capability.missing_required, ...capability.missing_optional];
  const docsUrl = missingTools.find((tool) => (tool.docs_url ?? '').trim() !== '')?.docs_url?.trim();
  return docsUrl && docsUrl.length > 0 ? docsUrl : null;
}
