import { useMemo } from "react";
import ConsoleView from "./components/ConsoleView";
import LaunchPanel from "./components/LaunchPanel";
import { ProfileEditorView } from "./components/ProfileEditor";
import { useProfile } from "./hooks/useProfile";
import type { SteamLaunchRequest } from "./types";

function deriveSteamClientInstallPath(compatdataPath: string): string {
  const marker = "/steamapps/compatdata/";
  const normalized = compatdataPath.trim().replace(/\\/g, "/");
  const index = normalized.indexOf(marker);

  return index >= 0 ? normalized.slice(0, index) : "";
}

export function App() {
  const profileState = useProfile();
  const { profile, profileName } = profileState;

  const launchRequest = useMemo<SteamLaunchRequest | null>(() => {
    if (!profile.steam.enabled) {
      return null;
    }

    return {
      game_path: profile.game.executable_path,
      trainer_path: profile.trainer.path,
      trainer_host_path: profile.trainer.path,
      steam_app_id: profile.steam.app_id,
      steam_compat_data_path: profile.steam.compatdata_path,
      steam_proton_path: profile.steam.proton_path,
      steam_client_install_path: deriveSteamClientInstallPath(
        profile.steam.compatdata_path,
      ),
      launch_trainer_only: false,
      launch_game_only: false,
    };
  }, [profile]);

  return (
    <main
      style={{
        minHeight: "100vh",
        padding: "32px",
        background:
          "radial-gradient(circle at top left, rgba(59, 130, 246, 0.16), transparent 28%), linear-gradient(180deg, #030712 0%, #0f172a 100%)",
        color: "#f8fafc",
        fontFamily:
          '"Avenir Next", "Segoe UI", "Helvetica Neue", system-ui, -apple-system, sans-serif',
      }}
    >
      <div
        style={{
          display: "grid",
          gap: "20px",
          maxWidth: "1100px",
          margin: "0 auto",
        }}
      >
        <header style={{ display: "grid", gap: "8px" }}>
          <div
            style={{
              color: "#60a5fa",
              fontSize: "0.75rem",
              letterSpacing: "0.2em",
              textTransform: "uppercase",
            }}
          >
            CrossHook Native
          </div>
          <h1 style={{ fontSize: "2rem", fontWeight: 700, margin: 0 }}>
            Two-step Steam launch
          </h1>
          <p style={{ margin: 0, color: "#cbd5e1", maxWidth: "48rem" }}>
            Launch the game first, then switch to trainer mode once the game
            reaches the main menu. The console below streams helper output.
          </p>
        </header>

        <div
          style={{
            display: "grid",
            gap: "20px",
            gridTemplateColumns: "minmax(0, 1.3fr) minmax(320px, 0.9fr)",
            alignItems: "start",
          }}
        >
          <ProfileEditorView state={profileState} />
          <div style={{ display: "grid", gap: "20px" }}>
            <LaunchPanel
              profileId={profileName || "new-profile"}
              steamModeEnabled={profile.steam.enabled}
              request={launchRequest}
            />
            <ConsoleView />
          </div>
        </div>
      </div>
    </main>
  );
}

export default App;
