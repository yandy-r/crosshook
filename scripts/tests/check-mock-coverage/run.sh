#!/usr/bin/env bash
# Self-test for scripts/check-mock-coverage.sh: runs the real script copy in isolated scratch repos.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SRC_SCRIPT="$HERE/../../check-mock-coverage.sh"
NATIVE=src/crosshook-native
TAURI=$NATIVE/src-tauri/src
CORE=$NATIVE/crates/crosshook-core/src
APP=$NATIVE/crates/crosshook-app/src
MOCKS=$NATIVE/src/lib/mocks
HANDLERS=$MOCKS/handlers
TYPES=$NATIVE/src/types
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
FAILURES=0

# new_repo <name>: a scratch repo whose mocks fully cover one command and two events
# (one emitted through a literal, one through a resolved constant).
new_repo() {
  local r="$TMP/$1"
  mkdir -p "$r/scripts" "$r/$TAURI" "$r/$CORE" "$r/$APP" "$r/$HANDLERS" "$r/$TYPES"
  cp "$SRC_SCRIPT" "$r/scripts/check-mock-coverage.sh"
  printf '#[tauri::command]\npub fn alpha() {}\n' > "$r/$TAURI/commands.rs"
  printf 'pub const ALPHA_EVENT: &str = "alpha-done";\npub const BETA_EVENT: &str = "beta-done";\n' \
    > "$r/$APP/ipc.rs"
  printf "export const BETA_DONE_EVENT = 'beta-done';\n" > "$r/$TYPES/beta.ts"
  cat > "$r/$HANDLERS/alpha.ts" <<'EOF'
import { emitMockEvent } from '../eventBus';
export function registerAlpha(map) {
  map.set('alpha', () => {
    emitMockEvent('alpha-done', {});
    emitMockEvent(
      BETA_DONE_EVENT,
      {}
    );
  });
  map.set('_mock_reset', () => null);
}
EOF
  printf "export interface BackendEventPayloads {\n  'alpha-done': unknown;\n  'beta-done': unknown;\n}\n" \
    > "$r/$TYPES/backend-events.ts"
  printf '# comment line\n\n' > "$r/$MOCKS/unmocked-events.txt"
  echo "$r"
}

# expect <name> <repo> <want-exit> [output substring]
expect() {
  local name="$1" repo="$2" want="$3" needle="${4:-}" out rc=0
  out="$(bash "$repo/scripts/check-mock-coverage.sh" 2>&1)" || rc=$?
  if [[ "$rc" != "$want" ]] || { [[ -n "$needle" ]] && [[ "$out" != *"$needle"* ]]; }; then
    echo "FAIL: $name (exit $rc, want $want, needle '$needle')" >&2
    echo "$out" >&2
    FAILURES=$((FAILURES + 1))
  else
    echo "ok: $name"
  fi
}

add_backend_event() { # <repo> <CONST> <name>: emitted by the backend and typed
  printf 'pub const %s: &str = "%s";\n' "$2" "$3" >> "$1/$APP/ipc.rs"
  sed -i "s/^}$/  '$3': unknown;\n}/" "$1/$TYPES/backend-events.ts"
}

r="$(new_repo clean)"
expect "covered commands and events pass" "$r" 0 "mock coverage: passed"

r="$(new_repo missing-command)"
printf '#[tauri::command]\npub async fn gamma() {}\n' >> "$r/$TAURI/commands.rs"
expect "command without a mock fails" "$r" 1 $'## Missing mock handlers (1)\ngamma'

r="$(new_repo core-command)"
printf '#[tauri::command]\npub fn delta() {}\n' > "$r/$CORE/delta.rs"
expect "crosshook-core commands are scanned" "$r" 1 $'## Missing mock handlers (1)\ndelta'

r="$(new_repo orphan-command)"
sed -i "s/map.set('_mock_reset'/map.set('omega'/" "$r/$HANDLERS/alpha.ts"
expect "mock without a command fails" "$r" 1 $'## Orphaned mock handlers (1)\nomega'

r="$(new_repo missing-event)"
add_backend_event "$r" GAMMA_EVENT gamma-done
expect "backend event without a mock fails" "$r" 1 $'(add an emitter or an unmocked-events.txt entry) (1)\ngamma-done'

r="$(new_repo tauri-event)"
printf 'const DELTA_EVENT: &str = "delta-done";\n' > "$r/$TAURI/events.rs"
sed -i "s/^}$/  'delta-done': unknown;\n}/" "$r/$TYPES/backend-events.ts"
expect "src-tauri event constants are scanned" "$r" 1 \
  $'(add an emitter or an unmocked-events.txt entry) (1)\ndelta-done'

r="$(new_repo allowlisted-event)"
add_backend_event "$r" GAMMA_EVENT gamma-done
echo 'gamma-done Never needed in browser dev mode.' >> "$r/$MOCKS/unmocked-events.txt"
expect "allow-listed event with a reason passes" "$r" 0 "mock coverage: passed"

r="$(new_repo allowlist-no-reason)"
add_backend_event "$r" GAMMA_EVENT gamma-done
echo 'gamma-done' >> "$r/$MOCKS/unmocked-events.txt"
expect "allow-list entry without a reason fails" "$r" 1 $'entries without a reason (1)\ngamma-done'

r="$(new_repo allowlist-stale)"
echo 'gone-event Removed long ago.' >> "$r/$MOCKS/unmocked-events.txt"
expect "allow-list entry for an unknown event fails" "$r" 1 $'(not a backend event) (1)\ngone-event'

r="$(new_repo allowlist-redundant)"
echo 'alpha-done Already mocked.' >> "$r/$MOCKS/unmocked-events.txt"
expect "allow-list entry for a mocked event fails" "$r" 1 $'(already mocked) (1)\nalpha-done'

r="$(new_repo orphan-event)"
sed -i "s/emitMockEvent('alpha-done', {});/emitMockEvent('alpha-done', {}); emitMockEvent('ghost-done', {});/" \
  "$r/$HANDLERS/alpha.ts"
expect "mock emitting an unknown event fails" "$r" 1 $'(no backend emitter) (1)\nghost-done'

r="$(new_repo untyped-event)"
printf 'pub const GAMMA_EVENT: &str = "gamma-done";\n' >> "$r/$APP/ipc.rs"
echo 'gamma-done Not needed.' >> "$r/$MOCKS/unmocked-events.txt"
expect "backend event without a payload type fails" "$r" 1 $'without a payload type in types/backend-events.ts (1)\ngamma-done'

r="$(new_repo unknown-type)"
sed -i "s/^}$/  'ghost-done': unknown;\n}/" "$r/$TYPES/backend-events.ts"
expect "payload type for an unknown event fails" "$r" 1 $'for unknown events in types/backend-events.ts (1)\nghost-done'

r="$(new_repo inline-literal)"
printf 'fn f() { let _ = WireEvent::new(\n    "sneaky-event", &()); }\n' > "$r/$APP/sneaky.rs"
expect "WireEvent::new string literal fails" "$r" 1 $'are not *_EVENT constants (1)\n"sneaky-event"'

r="$(new_repo unnamed-const)"
printf 'const SNEAKY: &str = "sneaky";\nfn f() { let _ = WireEvent::new(SNEAKY, &()); }\n' > "$r/$APP/sneaky.rs"
expect "WireEvent::new with a non-*_EVENT constant fails" "$r" 1 $'are not *_EVENT constants (1)\nSNEAKY'

r="$(new_repo named-binding)"
printf 'fn f(name: &str) { let _ = WireEvent::new(ALPHA_EVENT, &()); let _ = WireEvent::new(name, &()); }\n' \
  > "$r/$APP/ok.rs"
expect "WireEvent::new with a *_EVENT constant or name binding passes" "$r" 0 "mock coverage: passed"

r="$(new_repo struct-literal)"
printf 'fn f() -> WireEvent { WireEvent { name: "sneaky", payload: String::new() } }\n' > "$r/$APP/sneaky.rs"
printf 'fn t() -> WireEvent { WireEvent { name: "x", payload: String::new() } }\n' > "$r/$APP/tests.rs"
printf 'impl From<()> for WireEvent {\n    fn from(_: ()) -> WireEvent { unimplemented!() }\n}\n' > "$r/$APP/impls.rs"
expect "WireEvent struct literal outside tests fails" "$r" 1 \
  $'(string-literal emit or WireEvent { … }) (1)\nsrc/crosshook-native/crates/crosshook-app/src/sneaky.rs'

r="$(new_repo inline-emit)"
printf 'fn f(h: AppHandle) { h.emit_str(\n    "sneaky-event", String::new()); }\n' > "$r/$TAURI/sneaky.rs"
expect "inline emit literal fails" "$r" 1 "(string-literal emit or WireEvent { … }) (1)"

r="$(new_repo wrapped-const)"
printf 'pub const A_VERY_LONG_GAMMA_EVENT: &%sstatic str =\n    "gamma-done";\n' "'" >> "$r/$APP/ipc.rs"
expect "rustfmt-wrapped event constant is discovered" "$r" 1 \
  $'(add an emitter or an unmocked-events.txt entry) (1)\ngamma-done'

r="$(new_repo command-variants)"
printf '#[tauri::command(rename_all = "snake_case")]\npub(crate) async fn epsilon() {}\n#[tauri::command]\nfn zeta() {}\n' \
  > "$r/$TAURI/more.rs"
expect "command attribute arguments and visibility variants are scanned" "$r" 1 \
  $'## Missing mock handlers (2)\nepsilon\nzeta'

r="$(new_repo unresolved-emitter)"
sed -i "s/emitMockEvent('alpha-done', {});/emitMockEvent('alpha-done', {}); emitMockEvent(UNKNOWN_EVENT, {});/" \
  "$r/$HANDLERS/alpha.ts"
expect "emitter constant that cannot be resolved fails" "$r" 1 $'Unresolved mock event emitters (1)\nUNKNOWN_EVENT'

r="$(new_repo untyped-import)"
printf "import { emitMockEvent } from '../../events';\n" > "$r/$HANDLERS/raw.ts"
expect "handler importing the untyped emitter fails" "$r" 1 "importing the untyped emitter (import from '../eventBus') (1)"

r="$(new_repo untyped-import-alias)"
printf "import {\n  emitMockEvent,\n  resetBrowserEventBus,\n} from '@/lib/events';\n" > "$r/$HANDLERS/raw.ts"
expect "multi-line alias import of the untyped emitter fails" "$r" 1 "importing the untyped emitter (import from '../eventBus') (1)"

r="$(new_repo missing-allowlist)"
rm "$r/$MOCKS/unmocked-events.txt"
expect "missing allow-list file fails" "$r" 1 "required mock-coverage input not found"

if [[ $FAILURES -gt 0 ]]; then
  echo "check-mock-coverage self-test: $FAILURES failure(s)" >&2
  exit 1
fi
echo "check-mock-coverage self-test: all cases passed"
