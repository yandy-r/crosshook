using System.Diagnostics;
using CrossHookEngine.App.Services;

namespace CrossHookEngine.App.Tests;

public sealed class SteamLaunchServiceTests
{
    [Fact]
    public void Validate_ReturnsInvalid_WhenSteamAppIdIsMissing()
    {
        SteamLaunchRequest request = new SteamLaunchRequest
        {
            GamePath = @"D:\SteamLibrary\steamapps\common\MGS_TPP\mgsvtpp.exe",
            TrainerPath = @"D:\Games\Trainers\mgs-tpp-fling.exe",
            TrainerHostPath = "/mnt/sdb/Games/Trainers/mgs-tpp-fling.exe",
            SteamCompatDataPath = "/mnt/sdb/SteamLibrary/steamapps/compatdata/287700",
            SteamProtonPath = "/usr/share/steam/compatibilitytools.d/proton-cachyos-slr/proton",
            SteamClientInstallPath = "/home/yandy/.steam/root"
        };

        SteamLaunchValidationResult result = SteamLaunchService.Validate(request);

        Assert.False(result.IsValid);
        Assert.Contains("App ID", result.ErrorMessage);
    }

    [Fact]
    public void Validate_ReturnsValid_ForCompleteSteamRequest()
    {
        SteamLaunchRequest request = new SteamLaunchRequest
        {
            GamePath = @"D:\SteamLibrary\steamapps\common\MGS_TPP\mgsvtpp.exe",
            TrainerPath = @"D:\Games\Trainers\mgs-tpp-fling.exe",
            TrainerHostPath = "/mnt/sdb/Games/Trainers/mgs-tpp-fling.exe",
            SteamAppId = "287700",
            SteamCompatDataPath = "/mnt/sdb/SteamLibrary/steamapps/compatdata/287700",
            SteamProtonPath = "/usr/share/steam/compatibilitytools.d/proton-cachyos-slr/proton",
            SteamClientInstallPath = "/home/yandy/.steam/root"
        };

        SteamLaunchValidationResult result = SteamLaunchService.Validate(request);

        Assert.True(result.IsValid);
        Assert.Equal(string.Empty, result.ErrorMessage);
    }

    [Fact]
    public void ResolveGameExecutableName_ReturnsFileNameOnly()
    {
        string executableName = SteamLaunchService.ResolveGameExecutableName(@"D:\SteamLibrary\steamapps\common\MGS_TPP\mgsvtpp.exe");

        Assert.Equal("mgsvtpp.exe", executableName);
    }

    [Fact]
    public void NormalizeSteamHostPath_ConvertsZDriveBrowseSelectionsToUnixPaths()
    {
        string unixPath = SteamLaunchService.NormalizeSteamHostPath(@"Z:\mnt\sdb\SteamLibrary\steamapps\compatdata\287700");

        Assert.Equal("/mnt/sdb/SteamLibrary/steamapps/compatdata/287700", unixPath);
    }

    [Fact]
    public void CreateHelperStartInfo_EmbedsExpectedSteamArguments()
    {
        SteamLaunchRequest request = new SteamLaunchRequest
        {
            GamePath = @"D:\SteamLibrary\steamapps\common\MGS_TPP\mgsvtpp.exe",
            TrainerPath = @"D:\Games\Trainers\mgs tpp fling.exe",
            TrainerHostPath = "/mnt/sdb/Games/Trainers/mgs-tpp-fling.exe",
            SteamAppId = "287700",
            SteamCompatDataPath = "/mnt/sdb/SteamLibrary/steamapps/compatdata/287700",
            SteamProtonPath = "/usr/share/steam/compatibilitytools.d/proton-cachyos-slr/proton",
            SteamClientInstallPath = "/home/yandy/.steam/root"
        };

        ProcessStartInfo startInfo = SteamLaunchService.CreateHelperStartInfo(
            helperScriptUnixPath: "/opt/crosshook/runtime-helpers/steam-launch-helper.sh",
            compatDataUnixPath: request.SteamCompatDataPath,
            protonUnixPath: request.SteamProtonPath,
            logFileUnixPath: "/tmp/crosshook-steam-helper.log",
            request: request);

        Assert.EndsWith("start.exe", startInfo.FileName, StringComparison.OrdinalIgnoreCase);
        Assert.Contains("--appid \"287700\"", startInfo.Arguments);
        Assert.Contains("--game-exe-name \"mgsvtpp.exe\"", startInfo.Arguments);
        Assert.Contains("--trainer-path \"D:\\Games\\Trainers\\mgs tpp fling.exe\"", startInfo.Arguments);
        Assert.Contains("--trainer-host-path \"/mnt/sdb/Games/Trainers/mgs-tpp-fling.exe\"", startInfo.Arguments);
        Assert.Contains("/bin/bash", startInfo.Arguments);
        Assert.Contains("steam-launch-helper.sh", startInfo.Arguments);
        Assert.Contains("--log-file \"/tmp/crosshook-steam-helper.log\"", startInfo.Arguments);
        Assert.Equal("/mnt/sdb/SteamLibrary/steamapps/compatdata/287700", startInfo.Environment["STEAM_COMPAT_DATA_PATH"]);
        Assert.Equal("/home/yandy/.steam/root", startInfo.Environment["STEAM_COMPAT_CLIENT_INSTALL_PATH"]);
        Assert.Equal("/mnt/sdb/SteamLibrary/steamapps/compatdata/287700/pfx", startInfo.Environment["WINEPREFIX"]);
    }

    [Fact]
    public void CreateTrainerStartInfo_EmbedsExpectedTrainerArguments()
    {
        SteamLaunchRequest request = new SteamLaunchRequest
        {
            TrainerPath = @"D:\Games\Trainers\mgs tpp fling.exe",
            TrainerHostPath = "/mnt/sdb/Games/Trainers/mgs-tpp-fling.exe",
            SteamCompatDataPath = "/mnt/sdb/SteamLibrary/steamapps/compatdata/287700",
            SteamProtonPath = "/usr/share/steam/compatibilitytools.d/proton-cachyos-slr/proton",
            SteamClientInstallPath = "/home/yandy/.steam/root",
            LaunchTrainerOnly = true
        };

        ProcessStartInfo startInfo = SteamLaunchService.CreateTrainerStartInfo(
            trainerScriptUnixPath: "/opt/crosshook/runtime-helpers/steam-launch-trainer.sh",
            compatDataUnixPath: request.SteamCompatDataPath,
            protonUnixPath: request.SteamProtonPath,
            logFileUnixPath: "/tmp/crosshook-steam-trainer.log",
            request: request);

        Assert.EndsWith("start.exe", startInfo.FileName, StringComparison.OrdinalIgnoreCase);
        Assert.Contains("steam-launch-trainer.sh", startInfo.Arguments);
        Assert.Contains("--trainer-path \"D:\\Games\\Trainers\\mgs tpp fling.exe\"", startInfo.Arguments);
        Assert.Contains("--trainer-host-path \"/mnt/sdb/Games/Trainers/mgs-tpp-fling.exe\"", startInfo.Arguments);
        Assert.Contains("--proton \"/usr/share/steam/compatibilitytools.d/proton-cachyos-slr/proton\"", startInfo.Arguments);
        Assert.Contains("--log-file \"/tmp/crosshook-steam-trainer.log\"", startInfo.Arguments);
        Assert.Equal("/mnt/sdb/SteamLibrary/steamapps/compatdata/287700", startInfo.Environment["STEAM_COMPAT_DATA_PATH"]);
        Assert.Equal("/home/yandy/.steam/root", startInfo.Environment["STEAM_COMPAT_CLIENT_INSTALL_PATH"]);
        Assert.Equal("/mnt/sdb/SteamLibrary/steamapps/compatdata/287700/pfx", startInfo.Environment["WINEPREFIX"]);
    }

    [Fact]
    public void GetEnvironmentVariablesToClear_IncludesKnownWineBridgeVariables()
    {
        string[] variables = SteamLaunchService.GetEnvironmentVariablesToClear();

        Assert.Contains("WINESERVER", variables);
        Assert.Contains("LD_LIBRARY_PATH", variables);
        Assert.Contains("SteamGameId", variables);
        Assert.Contains("PROTON_DUMP_DEBUG_COMMANDS", variables);
    }

    [Fact]
    public void Validate_ReturnsInvalid_WhenTrainerHostPathIsMissing()
    {
        SteamLaunchRequest request = new SteamLaunchRequest
        {
            GamePath = @"D:\SteamLibrary\steamapps\common\MGS_TPP\mgsvtpp.exe",
            TrainerPath = @"D:\Games\Trainers\mgs-tpp-fling.exe",
            SteamAppId = "287700",
            SteamCompatDataPath = "/mnt/sdb/SteamLibrary/steamapps/compatdata/287700",
            SteamProtonPath = "/usr/share/steam/compatibilitytools.d/proton-cachyos-slr/proton",
            SteamClientInstallPath = "/home/yandy/.steam/root"
        };

        SteamLaunchValidationResult result = SteamLaunchService.Validate(request);

        Assert.False(result.IsValid);
        Assert.Contains("trainer host path", result.ErrorMessage, StringComparison.OrdinalIgnoreCase);
    }
}
