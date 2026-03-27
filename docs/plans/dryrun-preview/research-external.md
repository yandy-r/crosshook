# External API & Pattern Research: Dry Run / Preview Launch Mode

**Researcher**: api-researcher
**Date**: 2026-03-27
**Feature**: Dry Run / Preview Launch Mode (GitHub Issue #40)

---

## Executive Summary

This research surveys how industry-leading tools implement "plan" / "dry-run" / "preview" modes and identifies React libraries, integration patterns, and constraints relevant to building a preview launch panel in CrossHook's Tauri v2 + React frontend.

**Key insight**: The most successful preview implementations (Terraform, Pulumi, Ansible) share three UX pillars: (1) **structured change summaries** with action symbols, (2) **hierarchical detail drill-down** from summary to attribute-level changes, and (3) **machine-readable output** alongside human-readable display. CrossHook's preview feature should adopt these patterns while keeping the implementation lightweight — the data already exists in side-effect-free functions, so the effort is purely presentational.

---

## Primary APIs

This feature has no external API dependencies — all computation is performed by existing pure functions in `crosshook-core`. The research below focuses on comparable tool implementations and UI rendering patterns.

## Comparable "Plan/Preview" Implementations

### 1.1 Terraform `plan`

**Confidence**: High — official documentation and extensive community documentation.

Terraform's `plan` command is the gold standard for infrastructure preview UX. It creates an execution plan showing what will be created, updated, or destroyed _before_ any changes are applied.

#### Output Structure

The human-readable output follows a consistent hierarchy:

1. **Header**: Explains the symbols used in the output
2. **Resource change blocks**: Each affected resource listed with action symbol, address, and attribute-level changes
3. **Summary line**: `Plan: X to add, Y to change, Z to destroy.`

#### Symbol System

| Symbol | Color     | Meaning                            |
| ------ | --------- | ---------------------------------- |
| `+`    | Green     | Create (new resource)              |
| `-`    | Red       | Destroy (remove resource)          |
| `~`    | Yellow    | Update in-place                    |
| `-/+`  | Red/Green | Destroy and recreate (replacement) |
| `<=`   | Cyan      | Read (data source)                 |

#### JSON Machine-Readable Format

Terraform also provides `-json` output with a well-defined schema:

```json
{
  "format_version": "1.0",
  "resource_changes": [
    {
      "address": "aws_instance.web",
      "type": "aws_instance",
      "change": {
        "actions": ["create"],
        "before": null,
        "after": { "ami": "ami-12345", "instance_type": "t2.micro" },
        "after_unknown": { "id": true, "public_ip": true }
      },
      "action_reason": "replace_because_tainted"
    }
  ],
  "output_changes": { ... },
  "planned_values": { ... }
}
```

Key design decisions:

- **`actions` is an array**: Supports compound actions like `["delete", "create"]` for replacement
- **`before`/`after` objects**: Enable diff-style rendering of attribute changes
- **`after_unknown`**: Marks values that won't be known until apply (analogous to CrossHook's "resolved at launch time" values)
- **`action_reason`**: Provides context for _why_ a change is needed

#### CI/CD Integration Pattern

The `-no-color` flag strips ANSI codes for CI pipelines. A common pattern is posting plan output as a PR comment. The `-out` flag saves the plan as an artifact for later apply, ensuring what was reviewed is exactly what gets applied.

**Relevance to CrossHook**: The symbol system, hierarchical layout (summary -> detail -> attribute), and before/after diffing are directly applicable. The JSON schema concept maps well to the Tauri IPC response structure.

**Sources**:

- [Terraform plan command reference](https://developer.hashicorp.com/terraform/cli/commands/plan)
- [Terraform JSON output format](https://developer.hashicorp.com/terraform/internals/json-format)
- [Terraform machine-readable UI](https://developer.hashicorp.com/terraform/internals/machine-readable-ui)
- [Terraform Plan: Examples & How It Works (Spacelift)](https://spacelift.io/blog/terraform-plan)
- [Terraform Plan: Examples, Tips (env0)](https://www.env0.com/blog/terraform-plan)

---

### 1.2 Pulumi `preview`

**Confidence**: Medium — official docs, fewer UX-specific details available.

Pulumi's `preview` command serves the same role as Terraform's `plan` but with some UX enhancements:

- **Color-coded resource tree**: Shows resources in a hierarchical tree with `+`, `-`, `~` symbols (same convention as Terraform)
- **`--diff` flag**: Shows detailed property-level diffs
- **`--json` flag**: Machine-readable output for automation
- **`--save-plan` flag**: Saves preview as a plan file for guaranteed consistency at deploy time
- **AI-powered explain**: The `--copilot` flag (2025) adds an "explain" menu item that provides plain-language summaries of pending changes

**Relevance to CrossHook**: The `--diff` mode and AI explain feature demonstrate two ends of the UX spectrum — technical detail vs. plain-language summary. CrossHook could offer both a detailed technical view and a simplified "what will happen" summary.

**Sources**:

- [Pulumi preview CLI reference](https://www.pulumi.com/docs/iac/cli/commands/pulumi_preview/)
- [Pulumi Update Plans (Public Preview)](https://www.pulumi.com/blog/announcing-public-preview-update-plans/)
- [Pulumi CLI AI Extensions](https://www.pulumi.com/blog/cli-ai-extensions/)

---

### 1.3 Docker Compose `--dry-run`

**Confidence**: Medium — experimental feature with limited documentation.

Docker Compose's dry-run mode (still marked experimental/alpha) shows the steps Compose would take without executing them:

#### Output Format

```
[+] Pulling 1/1
 ✔ DRY-RUN MODE - db Pulled                                    0.9s
[+] Building 0/0
[+] Creating network app_default          DRY-RUN MODE
[+] Creating container app-db-1           DRY-RUN MODE
[+] Creating container app-web-1          DRY-RUN MODE
```

Key UX patterns:

- **`DRY-RUN MODE` label**: Prominently marks every step, making it impossible to confuse with actual execution
- **Progress-style output**: Uses the same visual format as real execution but appends the label
- **Timing estimates**: Shows estimated duration for each step
- **Step-by-step sequence**: Lists operations in execution order

**Limitations**: Only works with commands that change state (not `ps`, `ls`, `logs`). The feature is still marked experimental with potential for breaking changes.

**Relevance to CrossHook**: The "DRY-RUN MODE" label pattern is directly applicable — CrossHook should clearly label preview output to prevent confusion. The step-by-step sequential display maps well to showing the launch sequence (resolve env -> build command -> validate -> show final command).

**Sources**:

- [Docker Compose alpha dry-run docs](https://docs.docker.com/reference/cli/docker/compose/alpha/dry-run/)
- [Docker Compose dry-run GitHub issue #1203](https://github.com/docker/compose/issues/1203)
- [Docker Compose dry-run tutorial (LabEx)](https://labex.io/tutorials/docker-how-to-use-docker-compose-alpha-dry-run-command-to-test-changes-555069)

---

### 1.4 Ansible `--check` (Check Mode) + `--diff`

**Confidence**: High — well-documented, mature feature.

Ansible's check mode runs playbooks without making changes, reporting what _would_ change:

#### Check Mode Output

```
TASK [Install nginx] *****
changed: [webserver01]

PLAY RECAP *****
webserver01 : ok=1  changed=1  unreachable=0  failed=0  skipped=0
```

#### Combined Check + Diff Mode

When `--check` is combined with `--diff`, modules that support diff mode show before/after comparisons:

```diff
--- before: /etc/nginx/nginx.conf
+++ after: /etc/nginx/nginx.conf
@@ -1,3 +1,4 @@
 worker_processes 1;
+worker_connections 1024;
 events {
```

Key design decisions:

- **Task-level granularity**: Each task reports `ok`, `changed`, `skipped`, or `failed`
- **PLAY RECAP summary**: Aggregated counts at the end (analogous to Terraform's summary line)
- **Diff mode is opt-in**: Users can choose between simple changed/unchanged status or detailed diffs
- **Task-level control**: Individual tasks can force check mode on/off with `check_mode: yes/no`
- **Limitation acknowledgment**: The docs explicitly state "Check mode is just a simulation" and note that conditional tasks based on registered variables won't produce output

**Relevance to CrossHook**: The task-level status pattern (ok/changed/skipped/failed) maps well to validation results. The separate opt-in diff mode suggests CrossHook should default to a summary view with expandable details.

**Sources**:

- [Ansible Check Mode docs (2.9)](https://docs.ansible.com/ansible/2.9/user_guide/playbooks_checkmode.html)
- [Ansible Check Mode and Diff Mode (latest)](https://docs.ansible.com/projects/ansible/latest/playbook_guide/playbooks_checkmode.html)
- [Ansible Dry Run Tutorial (PhoenixNAP)](https://phoenixnap.com/kb/ansible-playbook-dry-run)
- [Ansible Dry Run: Check and Diff Mode](https://www.ansiblepilot.com/articles/ansible-playbook-dry-run-check-and-diff-mode)

---

### 1.5 CI/CD Pipeline Previews

**Confidence**: Medium — patterns exist but are less standardized.

#### GitHub Actions

GitHub Actions provides a DAG visualization of job dependencies showing which jobs run concurrently vs. sequentially, with status indicators (green/yellow/red). No native "dry-run" mode exists, but the workflow visualization acts as a preview of the pipeline structure.

#### GitLab CI

GitLab's CI Lint tool simulates pipeline creation for configuration validation. Merged results pipelines preview the outcome of merging source and target branches. The pipeline visualization shows job dependencies and execution order.

**Relevance to CrossHook**: The DAG visualization pattern could inspire a step-by-step launch sequence view showing the order of operations (resolve directives -> build command -> set up Proton env -> launch).

**Sources**:

- [GitHub Actions CI/CD guide](https://github.blog/enterprise-software/ci-cd/build-ci-cd-pipeline-github-actions-four-steps/)
- [GitLab Merge Request Pipelines](https://docs.gitlab.com/ci/pipelines/merge_request_pipelines/)
- [GitLab CI/CD Pipeline debugging](https://docs.gitlab.com/ci/debugging/)

---

### 1.6 Game Launcher Preview Patterns

**Confidence**: Medium — patterns extracted from multiple Linux game launchers.

#### SteamTinkerLaunch

- **Wait Requester dialog**: A pop-up before game launch (configurable 2-second timeout) that allows access to a full Settings Menu for reviewing/modifying launch configuration
- **Tab-organized settings**: Game Settings, Default Settings, and Global Settings tabs
- **Pre-launch menu**: Users can review and modify configuration before the game actually starts

#### Lutris

- **`-b` / `--output-script` flag**: Generates a bash script showing the full command with all parameters and environment variables, useful for debugging and previewing
- **Configuration tabs**: Game info, Game options, Runner options, System options
- **No built-in preview-before-launch GUI**: The script output is the closest equivalent

#### Bottles

- **Runner selection dropdown**: Visual display of the selected runner (Wine version)
- **CLI `--launch-options`**: Allows adding launch options programmatically
- **Environment variable management**: CLI-based env var configuration

#### Heroic Games Launcher

- **`--enable-logging` flag**: Shows backend messages, errors, and warnings in terminal
- **Launch Options setting**: Recently added per-game launch options UI

**Key takeaway**: No Linux game launcher currently offers a dedicated "preview what will happen" mode comparable to Terraform plan. **This is a differentiation opportunity for CrossHook.** The closest equivalent is Lutris's `--output-script` which dumps the launch command, but it's a CLI-only debugging tool, not an integrated UX feature.

**Sources**:

- [SteamTinkerLaunch Steam Launch Option Wiki](https://github.com/sonic2kk/steamtinkerlaunch/wiki/Steam-Launch-Option)
- [SteamTinkerLaunch Main Menu Wiki](https://github.com/sonic2kk/steamtinkerlaunch/wiki/Main-Menu)
- [Lutris GitHub](https://github.com/lutris/lutris)
- [Heroic Games Launcher Wiki](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/wiki/Linux-Quick-Start-Guide)
- [Bottles Runners docs](https://docs.usebottles.com/components/runners)

---

## 2. UI Libraries and Rendering Patterns

### 2.1 Syntax Highlighting for Shell Commands

**Confidence**: High — mature, widely-used libraries.

#### react-syntax-highlighter

- **Bundle**: ~30-50KB gzipped (full), lighter with light build
- **Engines**: Prism.js or Highlight.js under the hood
- **Shell support**: Built-in `bash`, `shell`, `powershell` language support
- **Theming**: Extensive theme library including dark themes (vs2015, atomDark, oneDark)
- **Usage**:

```tsx
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { vscDarkPlus } from 'react-syntax-highlighter/dist/esm/styles/prism';

<SyntaxHighlighter language="bash" style={vscDarkPlus}>
  {`STEAM_COMPAT_DATA_PATH="/home/user/.steam/steam/steamapps/compatdata/12345" \\
  gamemoderun mangohud \\
  /path/to/proton run /path/to/game.exe`}
</SyntaxHighlighter>;
```

- **Light build**: Register only needed languages to minimize bundle size

**Relevance**: Ideal for displaying the resolved launch command with syntax highlighting. The light build option keeps bundle size reasonable for a desktop app.

**Sources**:

- [react-syntax-highlighter npm](https://www.npmjs.com/package/react-syntax-highlighter)
- [react-syntax-highlighter GitHub](https://github.com/react-syntax-highlighter/react-syntax-highlighter)
- [Guide to Syntax Highlighting in React (LogRocket)](https://blog.logrocket.com/guide-syntax-highlighting-react/)

#### Alternative: CSS-only approach (recommended for CrossHook)

Given that CrossHook already uses custom CSS and has a dark theme, a simpler approach would be a `<pre><code>` block with custom CSS classes for different parts of the command (env vars, executable paths, arguments). This avoids adding a dependency entirely.

```css
.preview-command {
  font-family: monospace;
  background: var(--bg-secondary);
  padding: 1rem;
}
.preview-env {
  color: var(--color-env);
} /* e.g., yellow for env vars */
.preview-exe {
  color: var(--color-primary);
} /* e.g., blue for executables */
.preview-arg {
  color: var(--color-text);
} /* default for arguments */
```

---

### 2.2 Terminal-Style Output Renderers

**Confidence**: High — well-established ecosystem.

#### xterm.js + React Wrappers

- **xterm.js**: Full terminal emulator (~200KB), supports ANSI color codes, Unicode, cursor control
- **react-xtermjs**: React wrapper with `useXTerm` hook
- **xterm-for-react**: Simpler wrapper component

**Assessment**: **Overkill for CrossHook.** xterm.js is designed for interactive terminal sessions. CrossHook needs static, read-only output display. A styled `<pre>` block or syntax highlighter is more appropriate.

#### react-terminal-emulator-ui

- Simulates a command-line interface with customizable appearance
- More lightweight than xterm.js but still oriented toward interactive terminals

**Recommendation**: Skip terminal emulators entirely. Use a styled code block with custom CSS that matches CrossHook's existing dark theme. CrossHook already has `ConsoleView.tsx` for launch log output — the preview panel should use a similar visual style for consistency.

**Sources**:

- [xterm.js](https://xtermjs.org/)
- [react-xtermjs blog](https://www.qovery.com/blog/react-xtermjs-a-react-library-to-build-terminals)
- [react-terminal-emulator-ui GitHub](https://github.com/token-ed/react-terminal-emulator-ui)

---

### 2.3 Collapsible/Expandable Section Components

**Confidence**: High — standard UI pattern with many options.

#### Native HTML `<details>`/`<summary>` (Recommended)

The simplest approach — zero dependencies, accessible by default, keyboard-navigable:

```tsx
<details open>
  <summary>Environment Variables (12)</summary>
  <div className="preview-section">{/* env var list */}</div>
</details>
```

CrossHook already uses collapsible sections (per commit `79cba3c feat(ui): add collapsible sections to all pages`), so this pattern is established in the codebase.

#### Radix UI Accordion

- Headless, accessible primitives
- Full keyboard navigation and screen reader support
- ~5KB gzipped

#### react-collapsible

- Lightweight, single-purpose component
- Simple API: trigger text + children

**Recommendation**: Use native `<details>`/`<summary>` or the existing collapsible pattern from the codebase. No new dependency needed.

**Sources**:

- [Radix UI Accordion](https://www.radix-ui.com/primitives/docs/components/accordion)
- [react-collapsible npm](https://www.npmjs.com/package/react-collapsible)
- [shadcn/ui Accordion](https://www.shadcn.io/ui/accordion)

---

### 2.4 JSON/Object Tree Viewers

**Confidence**: High — well-established libraries.

For displaying structured data like environment variables as a tree:

#### react-json-view-lite (Recommended)

- **Bundle**: ~3KB gzipped — extremely lightweight
- **Features**: Collapsible nodes, keyboard navigation (arrow keys), dark/light themes
- **TypeScript**: Written in TypeScript, no dependencies
- **React 18**: Version 2.x supports React 18+ (CrossHook uses React 18)
- **Usage**:

```tsx
import { JsonView, darkStyles } from 'react-json-view-lite';
import 'react-json-view-lite/dist/index.css';

<JsonView data={envVars} shouldExpandNode={allExpanded} style={darkStyles} />;
```

#### Alternative: Custom key-value list

For environment variables, a simple two-column layout (key | value) may be more readable than a tree:

```tsx
<table className="preview-env-table">
  {Object.entries(envVars).map(([key, value]) => (
    <tr key={key}>
      <td className="env-key">{key}</td>
      <td className="env-value">{value}</td>
    </tr>
  ))}
</table>
```

**Recommendation**: Use a custom key-value table for environment variables (more readable for flat key-value pairs) and react-json-view-lite only if nested structures need to be displayed.

**Sources**:

- [react-json-view-lite GitHub](https://github.com/AnyRoad/react-json-view-lite)
- [react-json-view-lite Bundlephobia](https://bundlephobia.com/package/react-json-view-lite)
- [react-json-tree GitHub](https://github.com/alexkuz/react-json-tree)

---

## Integration Patterns

### 3.1 Diff-Style Output (Before/After Comparisons)

**Confidence**: High — mature pattern with multiple React libraries.

#### Libraries

| Library               | Bundle | Features                                                              | Maintenance    |
| --------------------- | ------ | --------------------------------------------------------------------- | -------------- |
| **react-diff-viewer** | ~15KB  | GitHub-style, split/unified views, word-level diff, line highlighting | Active         |
| **react-diff-view**   | ~20KB  | Powerful token system, code highlighting, web worker support          | Active         |
| **git-diff-view**     | ~25KB  | Multi-framework, GitHub-style, syntax highlighting                    | Active (2024+) |

#### Applicability to CrossHook

Diff views are most useful for comparing **profile changes** (e.g., what changed since last launch) rather than for the preview itself. The preview feature shows "what will happen" not "what changed." However, a lightweight diff could be valuable for:

- Showing differences between the profile config and the resolved launch state
- Highlighting overridden vs. default values
- Displaying wrapper chain modifications

**Recommendation**: Don't add a diff library for v1 of the preview feature. Use color coding (green for set values, gray for defaults/empty) instead. Consider diff views for a future "compare profiles" feature.

**Sources**:

- [react-diff-viewer GitHub](https://github.com/praneshr/react-diff-viewer)
- [react-diff-view GitHub](https://github.com/otakustay/react-diff-view)
- [git-diff-view GitHub](https://github.com/MrWangJustToDo/git-diff-view)

---

### 3.2 Tree View for Environment Variable Hierarchies

For flat environment variables (which is what CrossHook uses), a tree view adds unnecessary complexity. Instead, use **grouped sections**:

```
┌─ Proton Environment ─────────────────────┐
│ STEAM_COMPAT_DATA_PATH  /home/...        │
│ STEAM_COMPAT_CLIENT     /home/...        │
│ WINEPREFIX             /home/...         │
├─ Game Environment ────────────────────────┤
│ SteamAppId             12345             │
│ SteamGameId            12345             │
├─ Wrapper Variables ───────────────────────┤
│ MANGOHUD               1                 │
│ ENABLE_GAMESCOPE       0                 │
└───────────────────────────────────────────┘
```

This grouped key-value pattern is more scannable than a generic tree for flat data.

---

### 3.3 Copy-to-Clipboard for Command Strings

**Confidence**: High — well-established pattern.

#### Modern Approach: Navigator Clipboard API

```typescript
const copyToClipboard = async (text: string) => {
  try {
    await navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  } catch (err) {
    // Fallback for non-secure contexts
    const textArea = document.createElement('textarea');
    textArea.value = text;
    document.body.appendChild(textArea);
    textArea.select();
    document.execCommand('copy');
    document.body.removeChild(textArea);
  }
};
```

#### UX Pattern

- **Copy button** with clipboard icon next to each copyable section
- **Visual feedback**: Button text changes to "Copied!" or shows a checkmark for 2 seconds
- **Multiple copy targets**: Individual sections (env vars, command, full output)
- **Tauri consideration**: `navigator.clipboard` works in Tauri WebView (secure context)

#### Custom Hook Pattern (Recommended)

```typescript
function useCopyToClipboard() {
  const [copiedText, setCopiedText] = useState<string | null>(null);

  const copy = useCallback(async (text: string) => {
    await navigator.clipboard.writeText(text);
    setCopiedText(text);
    setTimeout(() => setCopiedText(null), 2000);
  }, []);

  return { copiedText, copy };
}
```

**Recommendation**: Implement a simple `useCopyToClipboard` hook. No external library needed. Provide copy buttons for: (1) the full resolved command, (2) the environment variable block, and (3) the complete preview output.

**Sources**:

- [useCopyToClipboard (usehooks-ts)](https://usehooks-ts.com/react-hook/use-copy-to-clipboard)
- [Implementing copy-clipboard in React (LogRocket)](https://blog.logrocket.com/implementing-copy-clipboard-react-clipboard-api/)
- [useClipboard Hook (react.wiki)](https://react.wiki/hooks/copy-to-clipboard/)

---

## 4. Constraints and Gotchas

### 4.1 Tauri v2 IPC Serialization

**Confidence**: High — official Tauri documentation.

#### How It Works

- All command arguments and return values are serialized as **JSON** via `serde::Serialize` / `serde::Deserialize`
- Arguments are passed as camelCase JSON objects from the frontend
- The `#[tauri::command]` macro generates the glue code

#### Performance Considerations

- **JSON overhead**: Tauri v1 had noticeable overhead for payloads > a few KB. Tauri v2 improved this with raw request/response support.
- **For CrossHook's preview**: The preview data (env vars, command strings, validation results) will be **well under 10KB** — JSON serialization will have negligible overhead.
- **Async commands**: Preview computation should use `async` to avoid blocking the main thread, though the existing functions are likely fast enough that it doesn't matter.

#### Recommended Pattern

```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DryRunResult {
    pub environment: HashMap<String, String>,
    pub wrapper_chain: Vec<String>,
    pub launch_command: String,
    pub steam_launch_options: Option<String>,
    pub validation_results: Vec<ValidationItem>,
    pub proton_setup: Option<ProtonInfo>,
}

#[tauri::command]
async fn preview_launch(profile_name: String) -> Result<DryRunResult, String> {
    // Call existing side-effect-free functions
    // ...
}
```

#### Binary Data

For this feature, **raw binary responses are not needed** — all preview data is text/structured data that serializes efficiently to JSON.

**Sources**:

- [Tauri v2: Calling Rust from Frontend](https://v2.tauri.app/develop/calling-rust/)
- [Tauri v2 IPC Concept](https://v2.tauri.app/concept/inter-process-communication/)
- [Tauri IPC Improvements Discussion #5690](https://github.com/tauri-apps/tauri/discussions/5690)

---

### 4.2 Performance of Serializing Large Environment Variable Sets

**Confidence**: High — based on data size analysis.

A typical CrossHook launch profile will generate:

- **Environment variables**: 10-30 key-value pairs (~2-5KB serialized)
- **Wrapper chain**: 3-8 commands (~500B)
- **Launch command**: Single string (~200B)
- **Validation results**: 5-15 items (~1-3KB)
- **Proton info**: ~500B

**Total estimated payload**: ~5-10KB JSON — well within Tauri v2's efficient handling range. No optimization needed.

---

### 4.3 Steam Deck Display Constraints (1280x800)

**Confidence**: High — hardware specs well-documented.

#### Key Constraints

| Factor           | Value                      | Impact                                           |
| ---------------- | -------------------------- | ------------------------------------------------ |
| Resolution       | 1280x800                   | Limited vertical space for multi-section preview |
| Screen size      | 7" (LCD) / 7.4" (OLED)     | Minimum 14pt font for readability                |
| Aspect ratio     | 16:10                      | Slightly more vertical space than 16:9           |
| Input            | Gamepad + touchscreen      | Must support both interaction modes              |
| CrossHook window | 1280x800 (tauri.conf.json) | Full-screen target on Deck                       |

#### Design Implications

1. **Vertical scrolling is essential**: The preview will not fit in a single viewport. Use collapsible sections so users can expand only what they need.
2. **Font size minimum**: 14px minimum for body text, 12px minimum for monospace command text.
3. **Touch targets**: Buttons and interactive elements need minimum 44x44px tap targets.
4. **Gamepad navigation**: CrossHook already has `useGamepadNav` — the preview panel must integrate with this. Collapsible sections should be focusable and expandable via gamepad.
5. **Modal vs. inline**: A full-screen modal is more practical on Steam Deck than a side panel, since there's no space for side-by-side layout.

**Recommendation**: Use a collapsible panel within `LaunchPanel.tsx` (not a separate modal) that expands to show the preview. Each section (env vars, command, validation) should be in a collapsible `<details>` block. This approach works on both desktop and Steam Deck without layout changes.

**Sources**:

- [Steam Deck Optimization Guide (PracticalMedia)](https://practicalmedia.io/article/optimizing-your-game-for-steam-deck-complete-guide)
- [Steam Deck Display Resolution (AndroidAuthority)](https://www.androidauthority.com/steam-deck-resolution-3339456/)
- [Steam Deck Resolution Suggestion (Paradox Forums)](https://forum.paradoxplaza.com/forum/threads/steam-deck-ui-or-rather-1280x800-resolution-suggestion.1624372/)

---

### 4.4 Tauri v2 Dialog/Modal Limitations

**Confidence**: Medium — based on community discussions.

- Tauri's built-in dialog plugin is limited to file pickers and message dialogs
- **Custom modals must be implemented in the web layer** (React), not as native windows
- Owner/parent window APIs don't implement blocking behavior consistently across platforms
- Web-based modals (React portals, overlay divs) are the recommended approach

**Recommendation**: Implement the preview as a web-based React component (inline panel or React modal), not a Tauri native dialog.

**Sources**:

- [Tauri Dialog Plugin](https://v2.tauri.app/plugin/dialog/)
- [Create Dialog-like Window Discussion #6569](https://github.com/tauri-apps/tauri/discussions/6569)
- [Custom Content Dialog Discussion #4079](https://github.com/tauri-apps/tauri/discussions/4079)

---

## 5. Code Examples

### 5.1 Terraform-Inspired Preview Output Structure

```tsx
// Preview section with Terraform-style symbols and summary
function PreviewOutput({ result }: { result: DryRunResult }) {
  return (
    <div className="preview-output">
      <div className="preview-header">
        <span className="preview-label">PREVIEW MODE</span>
        <span className="preview-profile">{result.profileName}</span>
      </div>

      <details open>
        <summary>
          <span className="symbol-create">+</span> Environment Variables ({Object.keys(result.environment).length})
        </summary>
        <EnvVarTable vars={result.environment} />
      </details>

      <details open>
        <summary>
          <span className="symbol-info">→</span> Wrapper Chain ({result.wrapperChain.length} wrappers)
        </summary>
        <WrapperList wrappers={result.wrapperChain} />
      </details>

      <details open>
        <summary>
          <span className="symbol-info">$</span> Launch Command
        </summary>
        <CommandBlock command={result.launchCommand} />
      </details>

      <details>
        <summary>
          <ValidationIcon results={result.validationResults} />
          Validation ({result.validationResults.length} checks)
        </summary>
        <ValidationList results={result.validationResults} />
      </details>

      <div className="preview-summary">
        Preview: {Object.keys(result.environment).length} env vars,
        {result.wrapperChain.length} wrappers,
        {result.validationResults.filter((v) => v.passed).length}/{result.validationResults.length} checks passed
      </div>
    </div>
  );
}
```

### 5.2 Copy-to-Clipboard Hook

```typescript
import { useState, useCallback } from 'react';

export function useCopyToClipboard() {
  const [copied, setCopied] = useState(false);

  const copy = useCallback(async (text: string) => {
    await navigator.clipboard.writeText(text);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }, []);

  return { copied, copy };
}
```

### 5.3 Tauri IPC Command Pattern

```rust
use serde::Serialize;
use std::collections::HashMap;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationItem {
    pub label: String,
    pub passed: bool,
    pub message: String,
    pub severity: String, // "error" | "warning" | "info"
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DryRunResult {
    pub profile_name: String,
    pub launch_method: String,
    pub environment: HashMap<String, String>,
    pub wrapper_chain: Vec<String>,
    pub launch_command: String,
    pub steam_launch_options: Option<String>,
    pub validation_results: Vec<ValidationItem>,
    pub proton_path: Option<String>,
    pub working_directory: Option<String>,
}
```

---

## 6. Search Queries Executed

1. `Terraform plan command output format UX patterns dry run preview 2025 2026`
2. `Docker Compose dry-run mode --dry-run implementation UX output 2025`
3. `Ansible --check mode dry run implementation output format patterns`
4. `React terminal output renderer component library syntax highlighting shell commands 2025 2026`
5. `React xterm.js terminal emulator component web UI command output display 2025`
6. `Tauri v2 IPC command serialization serde large data performance patterns 2025`
7. `React collapsible section accordion component lightweight 2025 tree view`
8. `GitHub Actions CI/CD pipeline preview dry run workflow visualization 2025`
9. `game launcher configuration preview mode dry run before launch Steam Deck Linux`
10. `react-json-tree react-json-view environment variable tree display component 2025`
11. `copy to clipboard React component pattern best practices 2025 navigator clipboard API`
12. `diff view React component before after comparison inline unified diff 2025`
13. `Steam Deck 1280x800 display UI design constraints web application layout responsive`
14. `Tauri v2 modal dialog React component pattern window create dialog 2025`
15. `Terraform plan output example actual terminal screenshot symbols color add change destroy`
16. `SteamTinkerLaunch configuration preview menu game launch options display before launch`
17. `Lutris game launcher Linux configuration preview command display before launch`
18. `Heroic Games Launcher Linux preview launch configuration display command environment variables`
19. `Bottles Linux game runner configuration display show launch command wrapper environment preview`
20. `Pulumi preview command output format UX changes display infrastructure preview 2025`
21. `GitLab CI CD pipeline preview simulation dry run merge request what-if analysis`
22. `react-json-view-lite bundle size features usage example structured data display lightweight`

---

## 7. Uncertainties and Gaps

1. **CrossHook's existing `ConsoleView.tsx` capabilities**: Need to assess whether the existing console view component can be extended for preview output or if a new component is needed. _(Requires codebase analysis by tech-designer.)_

2. **Exact data shape from existing functions**: The research assumes `resolve_launch_directives()`, `build_steam_launch_options_command()`, and `validate()` return structured data. The exact return types need verification. _(Requires codebase analysis by business-analyzer.)_

3. **Gamepad navigation for collapsible sections**: While `useGamepadNav` exists, the interaction pattern for expanding/collapsing `<details>` elements via gamepad needs testing. _(Requires UX validation.)_

4. **Performance of validation functions**: The assumption is that all preview computation is fast (<100ms). If any validation involves filesystem checks or network calls, the preview needs a loading state. _(Requires profiling.)_

5. **Copy-to-clipboard in Tauri WebView**: While `navigator.clipboard` should work in Tauri's secure context, this needs verification on Steam Deck's Game Mode WebView. _(Requires testing.)_

---

## 8. Recommendations Summary

| Decision             | Recommendation                          | Rationale                                        |
| -------------------- | --------------------------------------- | ------------------------------------------------ |
| UI component         | Inline collapsible panel in LaunchPanel | Consistent with existing UI; works on Steam Deck |
| Syntax highlighting  | Custom CSS classes on `<pre>` blocks    | Zero dependencies; matches existing dark theme   |
| Collapsible sections | Native `<details>`/`<summary>`          | Already used in codebase; accessible; zero deps  |
| Env var display      | Grouped key-value table                 | More readable than tree for flat data            |
| Copy to clipboard    | Custom `useCopyToClipboard` hook        | ~10 lines; no library needed                     |
| Diff views           | Skip for v1                             | Adds complexity without clear value for preview  |
| Terminal emulator    | Skip                                    | Overkill; static output doesn't need terminal    |
| JSON tree viewer     | Skip for v1                             | Custom table is sufficient                       |
| IPC serialization    | Standard JSON via serde                 | Payload <10KB; no optimization needed            |
| Dialog type          | Web-based React component               | Tauri native dialogs too limited                 |
