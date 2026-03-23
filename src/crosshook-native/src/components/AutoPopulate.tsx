import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type SteamFieldState = "NotFound" | "Found" | "Ambiguous";

interface SteamAutoPopulateRequest {
  game_path: string;
  steam_client_install_path: string;
}

interface SteamAutoPopulateResult {
  app_id_state: SteamFieldState;
  app_id: string;
  compatdata_state: SteamFieldState;
  compatdata_path: string;
  proton_state: SteamFieldState;
  proton_path: string;
  diagnostics: string[];
  manual_hints: string[];
}

interface AutoPopulateProps {
  gamePath: string;
  steamClientInstallPath: string;
  currentAppId: string;
  currentCompatdataPath: string;
  currentProtonPath: string;
  onApplyAppId: (value: string) => void;
  onApplyCompatdataPath: (value: string) => void;
  onApplyProtonPath: (value: string) => void;
}

interface FieldCardProps {
  label: string;
  state: SteamFieldState;
  currentValue: string;
  proposedValue: string;
  onApply: (() => void) | null;
}

const panelStyle = {
  background: "rgba(13, 20, 31, 0.92)",
  border: "1px solid rgba(120, 145, 177, 0.2)",
  borderRadius: 18,
  boxShadow: "0 24px 60px rgba(0, 0, 0, 0.35)",
  padding: 20,
};

const buttonStyle = {
  minHeight: 42,
  borderRadius: 12,
  border: "1px solid rgba(120, 145, 177, 0.35)",
  background: "linear-gradient(180deg, #1a2b45 0%, #132034 100%)",
  color: "#f3f6fb",
  padding: "0 14px",
  cursor: "pointer",
};

const subtleButtonStyle = {
  ...buttonStyle,
  background: "#0b1624",
};

const mutedTextStyle = {
  margin: 0,
  color: "#99a8bd",
  fontSize: 13,
  lineHeight: 1.5,
};

const stateStyles: Record<
  SteamFieldState,
  { label: string; color: string; background: string; border: string }
> = {
  Found: {
    label: "Found",
    color: "#c6f6d5",
    background: "rgba(20, 83, 45, 0.4)",
    border: "1px solid rgba(74, 222, 128, 0.3)",
  },
  Ambiguous: {
    label: "Ambiguous",
    color: "#fde68a",
    background: "rgba(113, 63, 18, 0.32)",
    border: "1px solid rgba(245, 158, 11, 0.35)",
  },
  NotFound: {
    label: "Not Found",
    color: "#fecaca",
    background: "rgba(127, 29, 29, 0.28)",
    border: "1px solid rgba(248, 113, 113, 0.32)",
  },
};

function FieldCard({
  label,
  state,
  currentValue,
  proposedValue,
  onApply,
}: FieldCardProps) {
  const styles = stateStyles[state];
  const hasProposedValue = proposedValue.trim().length > 0;
  const showApply = state === "Found" && onApply !== null && hasProposedValue;

  return (
    <div
      style={{
        borderRadius: 16,
        border: styles.border,
        background: styles.background,
        padding: 16,
        display: "grid",
        gap: 10,
      }}
    >
      <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "start" }}>
        <div>
          <div style={{ color: "#e5eefc", fontSize: 15, fontWeight: 700 }}>{label}</div>
          <div style={{ color: styles.color, fontSize: 12, fontWeight: 700, marginTop: 4 }}>
            {styles.label}
          </div>
        </div>
        {showApply ? (
          <button type="button" style={subtleButtonStyle} onClick={onApply}>
            Apply
          </button>
        ) : null}
      </div>

      <div style={{ display: "grid", gap: 8, fontSize: 13, color: "#cbd5e1" }}>
        <div>
          <strong style={{ color: "#f8fafc" }}>Current:</strong>{" "}
          {currentValue.trim().length > 0 ? currentValue : "unset"}
        </div>
        <div>
          <strong style={{ color: "#f8fafc" }}>Proposed:</strong>{" "}
          {hasProposedValue ? proposedValue : "none"}
        </div>
      </div>
    </div>
  );
}

export function AutoPopulate({
  gamePath,
  steamClientInstallPath,
  currentAppId,
  currentCompatdataPath,
  currentProtonPath,
  onApplyAppId,
  onApplyCompatdataPath,
  onApplyProtonPath,
}: AutoPopulateProps) {
  const [loading, setLoading] = useState(false);
  const [result, setResult] = useState<SteamAutoPopulateResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function runAutoPopulate() {
    setLoading(true);
    setError(null);

    try {
      const response = await invoke<SteamAutoPopulateResult>("auto_populate_steam", {
        request: {
          game_path: gamePath,
          steam_client_install_path: steamClientInstallPath,
        } satisfies SteamAutoPopulateRequest,
      });

      setResult(response);
    } catch (invokeError) {
      setError(invokeError instanceof Error ? invokeError.message : String(invokeError));
      setResult(null);
    } finally {
      setLoading(false);
    }
  }

  const appIdState = result?.app_id_state ?? "NotFound";
  const compatdataState = result?.compatdata_state ?? "NotFound";
  const protonState = result?.proton_state ?? "NotFound";

  return (
    <section style={panelStyle} aria-label="Auto-populate Steam values">
      <div style={{ display: "flex", justifyContent: "space-between", gap: 16, alignItems: "start", flexWrap: "wrap" }}>
        <div style={{ display: "grid", gap: 6 }}>
          <h2 style={{ margin: 0, fontSize: 18 }}>Auto-Populate Steam</h2>
          <p style={mutedTextStyle}>
            Scan the selected game and Steam install to fill App ID, compatdata, and Proton values.
          </p>
        </div>

        <button
          type="button"
          style={buttonStyle}
          onClick={() => void runAutoPopulate()}
          disabled={loading || gamePath.trim().length === 0 || steamClientInstallPath.trim().length === 0}
        >
          {loading ? "Scanning..." : "Auto-Populate"}
        </button>
      </div>

      <div style={{ display: "grid", gap: 12, gridTemplateColumns: "repeat(3, minmax(0, 1fr))", marginTop: 18 }}>
        <FieldCard
          label="Steam App ID"
          state={appIdState}
          currentValue={currentAppId}
          proposedValue={result?.app_id ?? ""}
          onApply={
            result?.app_id_state === "Found" && result.app_id.trim().length > 0
              ? () => onApplyAppId(result.app_id)
              : null
          }
        />
        <FieldCard
          label="Compatdata Path"
          state={compatdataState}
          currentValue={currentCompatdataPath}
          proposedValue={result?.compatdata_path ?? ""}
          onApply={
            result?.compatdata_state === "Found" && result.compatdata_path.trim().length > 0
              ? () => onApplyCompatdataPath(result.compatdata_path)
              : null
          }
        />
        <FieldCard
          label="Proton Path"
          state={protonState}
          currentValue={currentProtonPath}
          proposedValue={result?.proton_path ?? ""}
          onApply={
            result?.proton_state === "Found" && result.proton_path.trim().length > 0
              ? () => onApplyProtonPath(result.proton_path)
              : null
          }
        />
      </div>

      {error ? (
        <div
          style={{
            marginTop: 16,
            borderRadius: 12,
            padding: 12,
            background: "rgba(140, 40, 40, 0.2)",
            border: "1px solid rgba(255, 90, 90, 0.3)",
            color: "#ffd4d4",
          }}
        >
          {error}
        </div>
      ) : null}

      <div style={{ display: "grid", gap: 16, marginTop: 18 }}>
        <section
          style={{
            borderRadius: 16,
            background: "rgba(7, 12, 24, 0.55)",
            border: "1px solid rgba(255, 255, 255, 0.07)",
            padding: 16,
          }}
        >
          <h3 style={{ margin: 0, fontSize: 15, color: "#eef4ff" }}>Diagnostics</h3>
          <div style={{ marginTop: 10, display: "grid", gap: 8 }}>
            {loading ? (
              <div style={{ color: "#9fb1d6" }}>Waiting for Steam discovery output...</div>
            ) : result?.diagnostics?.length ? (
              result.diagnostics.map((entry, index) => (
                <div key={`${index}-${entry}`} style={{ color: "#cbd5e1", fontSize: 13, lineHeight: 1.5 }}>
                  {entry}
                </div>
              ))
            ) : (
              <div style={{ color: "#9fb1d6" }}>Run auto-populate to see discovery steps.</div>
            )}
          </div>
        </section>

        <section
          style={{
            borderRadius: 16,
            background: "rgba(7, 12, 24, 0.55)",
            border: "1px solid rgba(255, 255, 255, 0.07)",
            padding: 16,
          }}
        >
          <h3 style={{ margin: 0, fontSize: 15, color: "#eef4ff" }}>Manual Hints</h3>
          <div style={{ marginTop: 10, display: "grid", gap: 8 }}>
            {result?.manual_hints?.length ? (
              result.manual_hints.map((entry, index) => (
                <div key={`${index}-${entry}`} style={{ color: "#cbd5e1", fontSize: 13, lineHeight: 1.5 }}>
                  {entry}
                </div>
              ))
            ) : (
              <div style={{ color: "#9fb1d6" }}>Hints appear here when discovery is incomplete or ambiguous.</div>
            )}
          </div>
        </section>
      </div>
    </section>
  );
}

export default AutoPopulate;
