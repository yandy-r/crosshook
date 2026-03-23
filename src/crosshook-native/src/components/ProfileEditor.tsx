import type { CSSProperties, ChangeEvent } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import AutoPopulate from "./AutoPopulate";
import LauncherExport from "./LauncherExport";
import { useProfile, type UseProfileResult } from "../hooks/useProfile";

const panelStyle: CSSProperties = {
  background: "rgba(13, 20, 31, 0.92)",
  border: "1px solid rgba(120, 145, 177, 0.2)",
  borderRadius: 18,
  boxShadow: "0 24px 60px rgba(0, 0, 0, 0.35)",
  padding: 20,
};

const fieldStyle: CSSProperties = {
  display: "grid",
  gap: 8,
};

const inputStyle: CSSProperties = {
  width: "100%",
  minHeight: 44,
  borderRadius: 12,
  border: "1px solid rgba(120, 145, 177, 0.35)",
  background: "#08111c",
  color: "#f3f6fb",
  padding: "0 14px",
  boxSizing: "border-box",
};

const labelStyle: CSSProperties = {
  fontSize: 13,
  fontWeight: 600,
  color: "#b8c4d7",
  letterSpacing: "0.02em",
};

const buttonStyle: CSSProperties = {
  minHeight: 42,
  borderRadius: 12,
  border: "1px solid rgba(120, 145, 177, 0.35)",
  background: "linear-gradient(180deg, #1a2b45 0%, #132034 100%)",
  color: "#f3f6fb",
  padding: "0 14px",
  cursor: "pointer",
};

const subtleButtonStyle: CSSProperties = {
  ...buttonStyle,
  background: "#0b1624",
};

const helperStyle: CSSProperties = {
  margin: 0,
  color: "#99a8bd",
  fontSize: 13,
  lineHeight: 1.5,
};

function FieldRow(props: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  browseLabel?: string;
  onBrowse?: () => Promise<void>;
}) {
  return (
    <div style={fieldStyle}>
      <label style={labelStyle}>{props.label}</label>
      <div style={{ display: "flex", gap: 10, alignItems: "center" }}>
        <input
          style={{ ...inputStyle, flex: 1 }}
          value={props.value}
          placeholder={props.placeholder}
          onChange={(event: ChangeEvent<HTMLInputElement>) => props.onChange(event.target.value)}
        />
        {props.onBrowse ? (
          <button type="button" style={subtleButtonStyle} onClick={props.onBrowse}>
            {props.browseLabel ?? "Browse"}
          </button>
        ) : null}
      </div>
    </div>
  );
}

async function chooseFile(title: string, filters?: { name: string; extensions: string[] }[]) {
  const result = await open({
    directory: false,
    multiple: false,
    title,
    filters,
  });

  if (Array.isArray(result)) {
    return result[0] ?? null;
  }

  return result ?? null;
}

async function chooseDirectory(title: string) {
  const result = await open({
    directory: true,
    multiple: false,
    title,
  });

  if (Array.isArray(result)) {
    return result[0] ?? null;
  }

  return result ?? null;
}

function deriveSteamClientInstallPath(compatdataPath: string): string {
  const marker = "/steamapps/compatdata/";
  const normalized = compatdataPath.trim().replace(/\\/g, "/");
  const index = normalized.indexOf(marker);

  return index >= 0 ? normalized.slice(0, index) : "";
}

function deriveTargetHomePath(steamClientInstallPath: string): string {
  const normalized = steamClientInstallPath.trim().replace(/\\/g, "/");

  for (const suffix of ["/.local/share/Steam", "/.steam/root"]) {
    if (normalized.endsWith(suffix)) {
      return normalized.slice(0, -suffix.length);
    }
  }

  return "";
}

export interface ProfileEditorProps {
  state: UseProfileResult;
}

export function ProfileEditorView({ state }: ProfileEditorProps) {
  const {
    profiles,
    selectedProfile,
    profileName,
    profile,
    dirty,
    loading,
    saving,
    deleting,
    error,
    profileExists,
    setProfileName,
    selectProfile,
    updateProfile,
    saveProfile,
    deleteProfile,
    refreshProfiles,
  } = state;

  const canSave = profileName.trim().length > 0 && !saving && !deleting && !loading;
  const canDelete = profileExists && !saving && !deleting && !loading;
  const steamClientInstallPath = deriveSteamClientInstallPath(
    profile.steam.compatdata_path,
  );
  const targetHomePath = deriveTargetHomePath(steamClientInstallPath);

  return (
    <section style={panelStyle}>
      <div style={{ display: "flex", justifyContent: "space-between", gap: 16, alignItems: "center" }}>
        <div style={{ display: "grid", gap: 6 }}>
          <h2 style={{ margin: 0, fontSize: 18 }}>Profile</h2>
          <p style={helperStyle}>Select an existing profile or type a new name before saving.</p>
        </div>
        <button type="button" style={subtleButtonStyle} onClick={() => void refreshProfiles()}>
          Refresh
        </button>
      </div>

      <div style={{ display: "grid", gap: 12, gridTemplateColumns: "1fr auto" }}>
        <div style={fieldStyle}>
          <label style={labelStyle}>Profile Name</label>
          <input
            style={inputStyle}
            list="crosshook-profiles"
            value={profileName}
            placeholder="Enter or choose a profile name"
            onChange={(event) => setProfileName(event.target.value)}
          />
          <datalist id="crosshook-profiles">
            {profiles.map((name) => (
              <option key={name} value={name} />
            ))}
          </datalist>
        </div>

        <div style={fieldStyle}>
          <label style={labelStyle}>Load Existing</label>
          <select
            style={inputStyle}
            value={selectedProfile}
            onChange={(event) => void selectProfile(event.target.value)}
          >
            <option value="">Choose a profile</option>
            {profiles.map((name) => (
              <option key={name} value={name}>
                {name}
              </option>
            ))}
          </select>
        </div>
      </div>

      <div style={{ display: "grid", gap: 14, gridTemplateColumns: "repeat(2, minmax(0, 1fr))", marginTop: 16 }}>
        <FieldRow
          label="Game Path"
          value={profile.game.executable_path}
          onChange={(value) =>
            updateProfile((current) => ({
              ...current,
              game: { ...current.game, executable_path: value },
            }))
          }
          placeholder="/path/to/game.exe"
          browseLabel="Browse"
          onBrowse={async () => {
            const path = await chooseFile("Select Game Executable", [
              { name: "Windows Executable", extensions: ["exe"] },
            ]);

            if (path) {
              updateProfile((current) => ({
                ...current,
                game: { ...current.game, executable_path: path },
              }));
            }
          }}
        />

        <FieldRow
          label="Trainer Path"
          value={profile.trainer.path}
          onChange={(value) =>
            updateProfile((current) => ({
              ...current,
              trainer: { ...current.trainer, path: value },
            }))
          }
          placeholder="/path/to/trainer.exe"
          browseLabel="Browse"
          onBrowse={async () => {
            const path = await chooseFile("Select Trainer Executable", [
              { name: "Windows Executable", extensions: ["exe"] },
            ]);

            if (path) {
              updateProfile((current) => ({
                ...current,
                trainer: { ...current.trainer, path },
              }));
            }
          }}
        />
      </div>

      <label style={{ display: "flex", alignItems: "center", gap: 10, color: "#d9e3f0", fontWeight: 600, marginTop: 16 }}>
        <input
          type="checkbox"
          checked={profile.steam.enabled}
          onChange={(event) =>
            updateProfile((current) => ({
              ...current,
              steam: { ...current.steam, enabled: event.target.checked },
            }))
          }
        />
        Steam Mode
      </label>

      {profile.steam.enabled ? (
        <>
          <div style={{ display: "grid", gap: 14, gridTemplateColumns: "repeat(2, minmax(0, 1fr))", marginTop: 16 }}>
            <FieldRow
              label="Steam App ID"
              value={profile.steam.app_id}
              onChange={(value) =>
                updateProfile((current) => ({
                  ...current,
                  steam: { ...current.steam, app_id: value },
                }))
              }
              placeholder="1245620"
            />

            <FieldRow
              label="Compatdata Path"
              value={profile.steam.compatdata_path}
              onChange={(value) =>
                updateProfile((current) => ({
                  ...current,
                  steam: { ...current.steam, compatdata_path: value },
                }))
              }
              placeholder="/home/user/.local/share/Steam/steamapps/compatdata/1245620"
              browseLabel="Browse"
              onBrowse={async () => {
                const path = await chooseDirectory("Select Steam Compatdata Directory");

                if (path) {
                  updateProfile((current) => ({
                    ...current,
                    steam: { ...current.steam, compatdata_path: path },
                  }));
                }
              }}
            />

            <FieldRow
              label="Proton Path"
              value={profile.steam.proton_path}
              onChange={(value) =>
                updateProfile((current) => ({
                  ...current,
                  steam: { ...current.steam, proton_path: value },
                }))
              }
              placeholder="/home/user/.steam/root/steamapps/common/Proton - Experimental/proton"
              browseLabel="Browse"
              onBrowse={async () => {
                const path = await chooseFile("Select Proton Executable");

                if (path) {
                  updateProfile((current) => ({
                    ...current,
                    steam: { ...current.steam, proton_path: path },
                  }));
                }
              }}
            />

            <FieldRow
              label="Launcher Icon"
              value={profile.steam.launcher.icon_path}
              onChange={(value) =>
                updateProfile((current) => ({
                  ...current,
                  steam: {
                    ...current.steam,
                    launcher: { ...current.steam.launcher, icon_path: value },
                  },
                }))
              }
              placeholder="/path/to/icon.png"
              browseLabel="Browse"
              onBrowse={async () => {
                const path = await chooseFile("Select Launcher Icon", [
                  { name: "Images", extensions: ["png", "jpg", "jpeg"] },
                ]);

                if (path) {
                  updateProfile((current) => ({
                    ...current,
                    steam: {
                      ...current.steam,
                      launcher: { ...current.steam.launcher, icon_path: path },
                    },
                  }));
                }
              }}
            />
          </div>

          <div style={{ display: "grid", gap: 16, marginTop: 18 }}>
            <AutoPopulate
              gamePath={profile.game.executable_path}
              steamClientInstallPath={steamClientInstallPath}
              currentAppId={profile.steam.app_id}
              currentCompatdataPath={profile.steam.compatdata_path}
              currentProtonPath={profile.steam.proton_path}
              onApplyAppId={(value) =>
                updateProfile((current) => ({
                  ...current,
                  steam: { ...current.steam, app_id: value },
                }))
              }
              onApplyCompatdataPath={(value) =>
                updateProfile((current) => ({
                  ...current,
                  steam: { ...current.steam, compatdata_path: value },
                }))
              }
              onApplyProtonPath={(value) =>
                updateProfile((current) => ({
                  ...current,
                  steam: { ...current.steam, proton_path: value },
                }))
              }
            />

            <LauncherExport
              profile={profile}
              steamClientInstallPath={steamClientInstallPath}
              targetHomePath={targetHomePath}
            />
          </div>
        </>
      ) : null}

      <div style={{ display: "flex", gap: 12, flexWrap: "wrap", marginTop: 18 }}>
        <button type="button" style={buttonStyle} onClick={() => void saveProfile()} disabled={!canSave}>
          {saving ? "Saving..." : "Save"}
        </button>
        <button type="button" style={subtleButtonStyle} onClick={() => void deleteProfile()} disabled={!canDelete}>
          {deleting ? "Deleting..." : "Delete"}
        </button>
        <div style={{ display: "flex", alignItems: "center", color: dirty ? "#ffd166" : "#9bb1c8" }}>
          {loading ? "Loading..." : dirty ? "Unsaved changes" : "No unsaved changes"}
        </div>
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
    </section>
  );
}

export function ProfileEditor() {
  const state = useProfile();
  return (
    <div
      style={{
        minHeight: "100vh",
        padding: 24,
        background:
          "radial-gradient(circle at top, rgba(27, 59, 108, 0.35), transparent 35%), linear-gradient(180deg, #08111c 0%, #0b1320 100%)",
        color: "#f3f6fb",
      }}
    >
      <div style={{ display: "grid", gap: 18, maxWidth: 1180, margin: "0 auto" }}>
        <header style={{ display: "grid", gap: 8 }}>
          <div style={{ color: "#60a5fa", fontSize: 12, letterSpacing: "0.2em", textTransform: "uppercase" }}>
            CrossHook Native
          </div>
          <h1 style={{ margin: 0, fontSize: 32, fontWeight: 800 }}>Profile Editor</h1>
          <p style={{ ...helperStyle, maxWidth: 760 }}>
            Edit a profile, save it to Tauri storage, and keep Steam mode ready for launch.
          </p>
        </header>
        <ProfileEditorView state={state} />
      </div>
    </div>
  );
}

export default ProfileEditor;
