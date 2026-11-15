import type { BackendEventName, BackendEventPayloads } from '../../types/backend-events';
import { emitMockEvent as emitUntypedMockEvent } from '../events';

/**
 * Typed browser-dev emitter for mock handlers: only backend event names are
 * accepted, each with the payload shape the frontend listeners expect.
 */
export function emitMockEvent<K extends BackendEventName>(name: K, payload: BackendEventPayloads[K]): boolean {
  return emitUntypedMockEvent(name, payload);
}
