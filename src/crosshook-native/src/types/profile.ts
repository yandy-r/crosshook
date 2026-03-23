export interface ProfileData {
  GamePath: string;
  TrainerPath: string;
  Dll1Path: string;
  Dll2Path: string;
  LaunchInject1: boolean;
  LaunchInject2: boolean;
  LaunchMethod: string;
  UseSteamMode: boolean;
  SteamAppId: string;
  SteamCompatDataPath: string;
  SteamProtonPath: string;
  SteamLauncherIconPath: string;
}

export interface GameProfile {
  game: {
    name: string;
    executable_path: string;
  };
  trainer: {
    path: string;
    type: string;
  };
  injection: {
    dll_paths: string[];
    inject_on_launch: boolean[];
  };
  steam: {
    enabled: boolean;
    app_id: string;
    compatdata_path: string;
    proton_path: string;
    launcher: {
      icon_path: string;
      display_name: string;
    };
  };
  launch: {
    method: string;
  };
}
