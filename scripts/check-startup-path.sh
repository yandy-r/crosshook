#!/usr/bin/env bash
# Lexical guard for the pre-runtime/bootstrap ordering contract (YAN-781).
# Suppress one line with: // allow: startup-path <reason>
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

usage() {
  cat <<'EOF'
Usage: ./scripts/check-startup-path.sh [--help|-h] [--selftest]

Rejects startup sleeps (sleep, sleep_until, Duration::from_millis), the removed
startup.rs, app-layer process exits (process::exit/abort, bare exit/abort),
non-centralized app env mutations, and direct BaseDirs construction
(BaseDirs::new, including BaseDirs::new passed as a function) anywhere in
crosshook-core, crosshook-app, crosshook-cli and src-tauri.
Requires all scan roots/files and rejects symlinked roots and Rust sources.
This is a single-line lexical guard, not a Rust parser; multiline aliases and
macros still require review. A trailing // comment is ignored, but a // inside
a string literal is not a comment. BaseDirs also ignores test items; an
unterminated #[cfg(test)] block fails the check instead of skipping the file.
Suppression: // allow: startup-path <non-empty reason> on the offending line.
EOF
}

# lex_line <line>: sets CODE (the line without a trailing // comment, string
# literals kept) and SKELETON (CODE without string and char literals, used for
# attribute and brace tracking). A // inside a string does not start a comment.
# Raw or multi-line strings are not modelled; that only leaves more text in
# CODE, so unparsed forms are scanned rather than hidden.
lex_line() {
  local s="$1" i=0 n=${#1} c in_str=0 skel=""
  CODE="$s"
  while (( i < n )); do
    c="${s:i:1}"
    if (( in_str )); then
      if [[ "$c" == '\' ]]; then i=$((i + 2)); continue; fi
      if [[ "$c" == '"' ]]; then in_str=0; fi
    elif [[ "$c" == '"' ]]; then
      in_str=1
    elif [[ "$c" == "'" && "${s:i+2:1}" == "'" ]]; then
      i=$((i + 3)); continue
    elif [[ "$c" == "'" && "${s:i+1:1}" == '\' && "${s:i+3:1}" == "'" ]]; then
      i=$((i + 4)); continue
    elif [[ "$c" == '/' && "${s:i+1:1}" == '/' ]]; then
      CODE="${s:0:i}"
      break
    else
      skel+="$c"
    fi
    i=$((i + 1))
  done
  SKELETON="$skel"
}

# BaseDirs checks ignore test items; the startup rules also cover test code.
# Inline cfg(test) items are skipped by balanced braces, not the rest of a file.
# Losing brace sync (file ends inside a test block) fails closed.
scan_file() {
  local file="$1" pattern="$2" label="$3" ignore_tests="${4:-0}"
  local line code skeleton comment number=0 status test_pending=0 test_depth=0 opens closes
  [[ -f "$file" && ! -L "$file" ]] || { echo "error: invalid source: $file" >&2; return 1; }
  status=0
  grep -E -q -- "$pattern" "$file" || status=$?
  if (( status > 1 )); then echo "error: grep failed: $file" >&2; return 1; fi
  if (( status == 1 )); then return 0; fi
  while IFS= read -r line || [[ -n "$line" ]]; do
    number=$((number + 1))
    lex_line "$line"
    code="$CODE"; skeleton="$SKELETON"
    if (( ignore_tests && test_pending == 0 )) && [[ "$skeleton" =~ ^[[:space:]]*#\[cfg\(test\)\][[:space:]]*$ ]]; then
      test_pending=1; test_depth=0; continue
    fi
    if (( test_pending )); then
      opens="${skeleton//[^\{]/}"; closes="${skeleton//[^\}]/}"
      test_depth=$((test_depth + ${#opens} - ${#closes}))
      if [[ "$skeleton" == *'{'* ]]; then test_pending=2; fi
      if (( test_pending == 1 )) && [[ "$skeleton" == *';'* ]]; then test_pending=0; fi
      if (( test_pending == 2 && test_depth <= 0 )); then test_pending=0; test_depth=0; fi
      continue
    fi
    [[ -n "$code" ]] || continue
    comment="${line:${#code}}"
    if [[ "$comment" =~ ^//[[:space:]]*allow:[[:space:]]startup-path[[:space:]]+[^[:space:]] ]]; then continue; fi
    status=0
    grep -E -q -- "$pattern" <<<"$code" || status=$?
    if (( status > 1 )); then echo "error: grep failed: $file" >&2; return 1; fi
    if (( status == 0 )); then
      printf '%s:%s: %s: %s\n' "$file" "$number" "$label" "$line" >&2
      return 1
    fi
  done <"$file"
  if (( test_pending )); then
    echo "error: $file: unterminated #[cfg(test)] block (refusing to skip the rest of the file)" >&2
    return 1
  fi
}

is_test_path() {
  case "$1" in */tests/*|*/tests.rs|*_test.rs) return 0 ;; esac
  return 1
}

check_tree() (
  local root="$1" native app core cli shell file scan_root parent tmp links files count=0
  native="$root/src/crosshook-native"
  app="$native/crates/crosshook-app/src"
  core="$native/crates/crosshook-core/src"
  cli="$native/crates/crosshook-cli/src"
  shell="$native/src-tauri/src"
  local roots=("$app" "$core" "$cli" "$shell")
  local required=("$app/bootstrap/mod.rs" "$app/bootstrap/pre_runtime.rs" "$app/bootstrap/env_policy.rs"
    "$core/platform/dirs.rs" "$shell/lib.rs" "$shell/startup_bridge.rs" "$shell/env_policy.rs"
    "$shell/fatal.rs" "$shell/commands/startup.rs")
  for scan_root in "${roots[@]}"; do
    [[ -d "$scan_root" && ! -L "$scan_root" ]] || { echo "error: required scan root missing: $scan_root" >&2; return 1; }
  done
  for file in "${required[@]}"; do
    [[ -f "$file" && ! -L "$file" ]] || { echo "error: required regular file missing: $file" >&2; return 1; }
    parent="${file%/*}"
    while [[ "$parent" != "$root" && "$parent" != / ]]; do
      [[ ! -L "$parent" ]] || { echo "error: symlinked scan root: $parent" >&2; return 1; }
      parent="${parent%/*}"
    done
  done
  [[ ! -e "$shell/startup.rs" && ! -L "$shell/startup.rs" ]] || {
    echo "error: removed startup.rs must not exist" >&2; return 1;
  }
  tmp="$(mktemp -d)" || return 1
  trap 'rm -rf "$tmp"' EXIT
  links="$tmp/links"; files="$tmp/files"
  find "${roots[@]}" -type l -print0 >"$links" || return 1
  while IFS= read -r -d '' file; do
    if [[ "$file" == *.rs || -d "$file" ]]; then
      echo "error: symlinked Rust source/directory: $file" >&2; return 1
    fi
  done <"$links"
  find "${roots[@]}" -type f -name '*.rs' -print0 >"$files" || return 1
  local sleep_pattern='(^|[^[:alnum:]_])sleep(_until)?[[:space:]]*\(|Duration[[:space:]]*::[[:space:]]*from_millis[[:space:]]*\('
  local exit_pattern='process[[:space:]]*::[[:space:]]*(exit|abort)([^[:alnum:]_]|$)|(^|[^[:alnum:]_.:])(exit|abort)[[:space:]]*\('
  local basedirs_pattern='BaseDirs[[:space:]]*::[[:space:]]*new([^[:alnum:]_]|$)'
  while IFS= read -r -d '' file; do
    case "$file" in
      "$app/bootstrap/"*)
        count=$((count + 1))
        scan_file "$file" "$sleep_pattern" 'startup sleep' || return 1
        scan_file "$file" "$exit_pattern" 'app-layer process exit' || return 1 ;;
    esac
    case "$file" in
      "$app/bootstrap/pre_runtime.rs"|"$app/bootstrap/env_policy.rs") ;;
      "$app/"*) scan_file "$file" '(^|[^[:alnum:]_])(set_var|remove_var)[[:space:]]*\(' 'env mutation outside pre-runtime boundary' || return 1 ;;
    esac
    if [[ "$file" != "$core/platform/dirs.rs" ]] && ! is_test_path "$file"; then
      scan_file "$file" "$basedirs_pattern" 'BaseDirs bypass' 1 || return 1
    fi
  done <"$files"
  (( count > 0 )) || { echo 'error: no bootstrap sources found' >&2; return 1; }
  for file in "$shell/lib.rs" "$shell/startup_bridge.rs" "$shell/env_policy.rs" "$shell/fatal.rs" "$shell/commands/startup.rs"; do
    scan_file "$file" "$sleep_pattern" 'startup sleep' || return 1
  done
  echo 'startup-path check passed.'
)

# Every case starts from a freshly rebuilt baseline tree, so no fixture can
# leak into another, and every negative case must fail with its own label.
selftest() (
  local scratch native app core cli shell fixture target
  scratch="$(mktemp -d)" || exit 1
  trap 'rm -rf "$scratch"' EXIT
  native="$scratch/src/crosshook-native"
  app="$native/crates/crosshook-app/src"
  core="$native/crates/crosshook-core/src"
  cli="$native/crates/crosshook-cli/src"
  shell="$native/src-tauri/src"
  local required=("$app/bootstrap/mod.rs" "$app/bootstrap/pre_runtime.rs" "$app/bootstrap/env_policy.rs"
    "$core/platform/dirs.rs" "$shell/lib.rs" "$shell/startup_bridge.rs" "$shell/env_policy.rs"
    "$shell/fatal.rs" "$shell/commands/startup.rs")
  die() { echo "selftest FAILED: $1" >&2; exit 1; }
  fresh() {
    local f
    rm -rf "${scratch:?}/src"
    mkdir -p "$app/bootstrap" "$core/platform" "$cli" "$shell/commands"
    for f in "${required[@]}"; do : >"$f"; done
  }
  expect_pass() {
    local out
    out="$(check_tree "$scratch" 2>&1)" || die "$1 (rejected: $out)"
  }
  expect_fail() {
    local out
    if out="$(check_tree "$scratch" 2>&1)"; then die "$2 (unexpectedly passed)"; fi
    [[ "$out" == *"$1"* ]] || die "$2 (expected '$1', got: $out)"
  }

  fresh; expect_pass 'baseline'

  for fixture in 'sleep(1);' 'std::thread::sleep(x);' 'tokio::time::sleep(x);' \
    'tokio::time::sleep_until(x);' 'sleep_until (x);' 'Duration::from_millis(1);' \
    'let s = "//"; sleep(d);'; do
    fresh; printf '%s\n' "$fixture" >"$shell/lib.rs"
    expect_fail 'startup sleep' "sleep in src-tauri: $fixture"
  done
  fresh; printf 'sleep_until(x);\n' >"$app/bootstrap/mod.rs"
  expect_fail 'startup sleep' 'sleep in app bootstrap'
  fresh; printf 'sleep(1); // allow: startup-path\n' >"$shell/lib.rs"
  expect_fail 'startup sleep' 'suppression needs a reason'
  fresh; printf '// sleep(1);\nsleep(1); // allow: startup-path fixture reason\n' >"$shell/lib.rs"
  expect_pass 'comments and reasoned suppression'

  for fixture in 'std::process::exit(1);' 'process::abort();' 'std::process::abort();' 'exit(1);' \
    'abort();' 'let f = process::exit;' 'use std::process::exit;'; do
    fresh; printf '%s\n' "$fixture" >"$app/bootstrap/mod.rs"
    expect_fail 'app-layer process exit' "exit form: $fixture"
  done
  fresh; printf 'handle.exit(1);\nlet code = exit_code();\n' >"$app/bootstrap/mod.rs"
  expect_pass 'method exit and exit_code are not process exits'

  fresh; printf 'std::env::set_var("x", "y");\n' >"$app/other.rs"
  expect_fail 'env mutation outside pre-runtime boundary' 'set_var'
  fresh; printf 'std::env::remove_var("x");\n' >"$app/other.rs"
  expect_fail 'env mutation outside pre-runtime boundary' 'remove_var'
  fresh; printf 'std::env::set_var("x", "y");\n' >"$app/bootstrap/pre_runtime.rs"
  expect_pass 'set_var inside the pre-runtime boundary'

  for fixture in 'BaseDirs::new();' 'let d = directories::BaseDirs::new();' 'x.or_else(BaseDirs::new)' \
    'let f = BaseDirs::new;' 'BaseDirs :: new ()' 'let u = "https://x"; BaseDirs::new();' \
    'let s = "//"; BaseDirs::new();'; do
    for target in "$core/other.rs" "$app/other.rs" "$cli/other.rs" "$shell/other.rs"; do
      fresh; printf '%s\n' "$fixture" >"$target"
      expect_fail 'BaseDirs bypass' "BaseDirs in $target: $fixture"
    done
  done
  fresh; printf 'BaseDirs::new()\n' >"$core/platform/dirs.rs"
  expect_pass 'dirs.rs may construct BaseDirs'
  fresh
  printf '// BaseDirs::new()\n/// directories::BaseDirs::new()\nlet u = "a//b"; // BaseDirs::new()\nBaseDirs::new_in(x);\n' >"$core/other.rs"
  expect_pass 'BaseDirs in comments and BaseDirs::new_in'
  fresh; printf 'BaseDirs::new(); // allow: startup-path fixture reason\n' >"$cli/other.rs"
  expect_pass 'reasoned BaseDirs suppression'
  fresh; mkdir -p "$core/tests"
  printf 'BaseDirs::new();\n' >"$core/tests/fixture.rs"
  printf 'BaseDirs::new();\n' >"$shell/commands/tests.rs"
  expect_pass 'test paths are ignored'
  fresh
  printf '#[cfg(test)]\nmod tests {\n fn t() { let _ = '"'"'{'"'"'; let _ = "{"; BaseDirs::new(); }\n}\n' >"$core/other.rs"
  expect_pass 'inline test module with braces in literals'
  printf 'BaseDirs::new();\n' >>"$core/other.rs"
  expect_fail 'BaseDirs bypass' 'code after test module'
  fresh; printf '#[cfg(test)]\nmod tests;\nBaseDirs::new();\n' >"$core/other.rs"
  expect_fail 'BaseDirs bypass' 'code after external test module'
  fresh; printf '#[cfg(test)]\nmod tests {\n fn t() { BaseDirs::new(); }\n' >"$core/other.rs"
  expect_fail 'unterminated #[cfg(test)] block' 'test block without closing brace'
  fresh; printf '#[cfg(test)]\n// BaseDirs::new()\n' >"$core/other.rs"
  expect_fail 'unterminated #[cfg(test)] block' 'dangling cfg(test) attribute'

  fresh; : >"$shell/startup.rs"
  expect_fail 'removed startup.rs must not exist' 'obsolete startup.rs'
  fresh; rm "$shell/lib.rs"
  expect_fail 'required regular file missing' 'missing required source'
  fresh; rm "$shell/lib.rs"; ln -s "$shell/env_policy.rs" "$shell/lib.rs"
  expect_fail 'required regular file missing' 'required source symlink'
  fresh; rm -rf "$cli"
  expect_fail 'required scan root missing' 'missing cli scan root'
  fresh; ln -s "$app/bootstrap" "$app/linked"
  expect_fail 'symlinked Rust source/directory' 'directory symlink'
  fresh; ln -s /nonexistent "$core/dangling.rs"
  expect_fail 'symlinked Rust source/directory' 'dangling Rust symlink'

  echo 'startup-path selftest passed.'
)

case "${1:-}" in
  --help|-h) usage ;;
  --selftest) [[ $# == 1 ]] || { usage >&2; exit 1; }; selftest ;;
  '') check_tree "$REPO_ROOT" ;;
  *) usage >&2; exit 1 ;;
esac
