using System;
using System.Diagnostics;
using System.IO;

namespace CrossHookEngine.App.Services;

public sealed class SteamLaunchRequest
{
    public string GamePath { get; set; } = string.Empty;

    public string TrainerPath { get; set; } = string.Empty;

    public string TrainerHostPath { get; set; } = string.Empty;

    public string SteamAppId { get; set; } = string.Empty;

    public string SteamCompatDataPath { get; set; } = string.Empty;

    public string SteamProtonPath { get; set; } = string.Empty;

    public string SteamClientInstallPath { get; set; } = string.Empty;

    public bool LaunchTrainerOnly { get; set; }

    public bool LaunchGameOnly { get; set; }
}

public sealed class SteamLaunchValidationResult
{
    public bool IsValid { get; }

    public string ErrorMessage { get; }

    public SteamLaunchValidationResult(bool isValid, string errorMessage = "")
    {
        IsValid = isValid;
        ErrorMessage = errorMessage ?? string.Empty;
    }
}

public sealed class SteamLaunchExecutionResult
{
    public bool Succeeded { get; }

    public string Message { get; }

    public string HelperLogPath { get; }

    public SteamLaunchExecutionResult(bool succeeded, string message, string helperLogPath = "")
    {
        Succeeded = succeeded;
        Message = message ?? string.Empty;
        HelperLogPath = helperLogPath ?? string.Empty;
    }
}

public static class SteamLaunchService
{
    public const string RuntimeHelpersDirectoryName = "runtime-helpers";
    public const string HelperScriptFileName = "steam-launch-helper.sh";
    public const string TrainerScriptFileName = "steam-launch-trainer.sh";
    public const int DefaultGameTimeoutSeconds = 90;
    public const int DefaultGameStartupDelaySeconds = 30;
    public const int DefaultTrainerTimeoutSeconds = 10;

    public static SteamLaunchValidationResult Validate(SteamLaunchRequest request)
    {
        ArgumentNullException.ThrowIfNull(request);

        if (!request.LaunchTrainerOnly && string.IsNullOrWhiteSpace(request.GamePath))
        {
            return new SteamLaunchValidationResult(false, "Steam mode requires a game executable path so CrossHook can identify the game process.");
        }

        if (string.IsNullOrWhiteSpace(request.TrainerPath))
        {
            return new SteamLaunchValidationResult(false, "Steam mode requires a trainer path.");
        }

        if (string.IsNullOrWhiteSpace(request.TrainerHostPath))
        {
            return new SteamLaunchValidationResult(false, "Steam mode requires a trainer host path.");
        }

        if (string.IsNullOrWhiteSpace(request.SteamAppId))
        {
            return new SteamLaunchValidationResult(false, "Steam mode requires a Steam App ID.");
        }

        if (string.IsNullOrWhiteSpace(request.SteamCompatDataPath))
        {
            return new SteamLaunchValidationResult(false, "Steam mode requires a compatdata path.");
        }

        if (string.IsNullOrWhiteSpace(request.SteamProtonPath))
        {
            return new SteamLaunchValidationResult(false, "Steam mode requires a Proton path.");
        }

        if (string.IsNullOrWhiteSpace(request.SteamClientInstallPath))
        {
            return new SteamLaunchValidationResult(false, "Steam mode requires a Steam client install path.");
        }

        return new SteamLaunchValidationResult(true);
    }

    public static string ResolveGameExecutableName(string gamePath)
    {
        if (string.IsNullOrWhiteSpace(gamePath))
        {
            return string.Empty;
        }

        int lastForwardSlash = gamePath.LastIndexOf('/');
        int lastBackslash = gamePath.LastIndexOf('\\');
        int separatorIndex = Math.Max(lastForwardSlash, lastBackslash);

        return separatorIndex >= 0
            ? gamePath[(separatorIndex + 1)..]
            : gamePath;
    }

    public static string ResolveHelperScriptPath(string startupPath)
    {
        ArgumentNullException.ThrowIfNull(startupPath);
        return Path.Combine(startupPath, RuntimeHelpersDirectoryName, HelperScriptFileName);
    }

    public static string ResolveTrainerScriptPath(string startupPath)
    {
        ArgumentNullException.ThrowIfNull(startupPath);
        return Path.Combine(startupPath, RuntimeHelpersDirectoryName, TrainerScriptFileName);
    }

    public static ProcessStartInfo CreateHelperStartInfo(
        string helperScriptUnixPath,
        string compatDataUnixPath,
        string protonUnixPath,
        string logFileUnixPath,
        SteamLaunchRequest request)
    {
        ArgumentNullException.ThrowIfNull(request);

        string arguments =
            "/unix /bin/bash "
            + QuoteForCommand(helperScriptUnixPath)
            + " --appid " + QuoteForCommand(request.SteamAppId)
            + " --compatdata " + QuoteForCommand(compatDataUnixPath)
            + " --proton " + QuoteForCommand(protonUnixPath)
            + " --steam-client " + QuoteForCommand(request.SteamClientInstallPath)
            + " --game-exe-name " + QuoteForCommand(ResolveGameExecutableName(request.GamePath))
            + " --trainer-path " + QuoteForCommand(request.TrainerPath)
            + " --trainer-host-path " + QuoteForCommand(request.TrainerHostPath)
            + " --log-file " + QuoteForCommand(logFileUnixPath)
            + " --game-startup-delay-seconds " + DefaultGameStartupDelaySeconds.ToString()
            + " --game-timeout-seconds " + DefaultGameTimeoutSeconds.ToString()
            + " --trainer-timeout-seconds " + DefaultTrainerTimeoutSeconds.ToString();

        if (request.LaunchTrainerOnly)
        {
            arguments += " --trainer-only";
        }

        if (request.LaunchGameOnly)
        {
            arguments += " --game-only";
        }

        return CreateUnixBridgeStartInfo(arguments, compatDataUnixPath, request.SteamClientInstallPath);
    }

    public static ProcessStartInfo CreateTrainerStartInfo(
        string trainerScriptUnixPath,
        string compatDataUnixPath,
        string protonUnixPath,
        string logFileUnixPath,
        SteamLaunchRequest request)
    {
        ArgumentNullException.ThrowIfNull(request);

        string arguments =
            "/unix /bin/bash "
            + QuoteForCommand(trainerScriptUnixPath)
            + " --compatdata " + QuoteForCommand(compatDataUnixPath)
            + " --proton " + QuoteForCommand(protonUnixPath)
            + " --steam-client " + QuoteForCommand(request.SteamClientInstallPath)
            + " --trainer-path " + QuoteForCommand(request.TrainerPath)
            + " --trainer-host-path " + QuoteForCommand(request.TrainerHostPath)
            + " --log-file " + QuoteForCommand(logFileUnixPath);

        return CreateUnixBridgeStartInfo(arguments, compatDataUnixPath, request.SteamClientInstallPath);
    }

    private static ProcessStartInfo CreateUnixBridgeStartInfo(
        string arguments,
        string compatDataUnixPath,
        string steamClientInstallPath)
    {
        ProcessStartInfo startInfo = new ProcessStartInfo
        {
            // Wine's `start.exe /unix` bridge is required here because the
            // Windows process hosting CrossHook cannot reliably Process.Start
            // a native Unix executable like `/bin/bash` directly.
            FileName = Path.Combine(Environment.SystemDirectory, "start.exe"),
            Arguments = arguments,
            UseShellExecute = false,
            CreateNoWindow = true
        };

        ApplyCleanSteamEnvironment(startInfo, compatDataUnixPath, steamClientInstallPath);
        return startInfo;
    }

    private static void ApplyCleanSteamEnvironment(
        ProcessStartInfo startInfo,
        string compatDataUnixPath,
        string steamClientInstallPath)
    {
        foreach (string environmentVariable in GetEnvironmentVariablesToClear())
        {
            _ = startInfo.Environment.Remove(environmentVariable);
        }

        startInfo.Environment["STEAM_COMPAT_DATA_PATH"] = compatDataUnixPath;
        startInfo.Environment["STEAM_COMPAT_CLIENT_INSTALL_PATH"] = steamClientInstallPath;
        startInfo.Environment["WINEPREFIX"] = $"{compatDataUnixPath.TrimEnd('/')}/pfx";
    }

    internal static string[] GetEnvironmentVariablesToClear()
    {
        return
        [
            "WINESERVER",
            "WINELOADER",
            "WINEDLLPATH",
            "WINEDLLOVERRIDES",
            "WINEDEBUG",
            "WINEESYNC",
            "WINEFSYNC",
            "WINELOADERNOEXEC",
            "WINE_LARGE_ADDRESS_AWARE",
            "WINE_DISABLE_KERNEL_WRITEWATCH",
            "WINE_HEAP_DELAY_FREE",
            "WINEFSYNC_SPINCOUNT",
            "LD_PRELOAD",
            "LD_LIBRARY_PATH",
            "GST_PLUGIN_PATH",
            "GST_PLUGIN_SYSTEM_PATH",
            "GST_PLUGIN_SYSTEM_PATH_1_0",
            "SteamGameId",
            "SteamAppId",
            "GAMEID",
            "PROTON_LOG",
            "PROTON_DUMP_DEBUG_COMMANDS",
            "PROTON_USE_WINED3D",
            "PROTON_NO_ESYNC",
            "PROTON_NO_FSYNC",
            "PROTON_ENABLE_NVAPI",
            "DXVK_CONFIG_FILE",
            "DXVK_STATE_CACHE_PATH",
            "DXVK_LOG_PATH",
            "VKD3D_CONFIG",
            "VKD3D_DEBUG"
        ];
    }

    public static string ConvertToUnixPath(string windowsOrUnixPath)
    {
        if (string.IsNullOrWhiteSpace(windowsOrUnixPath))
        {
            return string.Empty;
        }

        string trimmedPath = windowsOrUnixPath.Trim();
        if (trimmedPath.StartsWith("/", StringComparison.Ordinal))
        {
            return trimmedPath;
        }

        if (LooksLikeWindowsPath(trimmedPath) && char.ToUpperInvariant(trimmedPath[0]) == 'Z')
        {
            string unixPath = trimmedPath[2..].Replace('\\', '/');
            return string.IsNullOrEmpty(unixPath) ? "/" : unixPath;
        }

        string winePathExecutable = Path.Combine(Environment.SystemDirectory, "winepath.exe");
        ProcessStartInfo startInfo = new ProcessStartInfo
        {
            FileName = winePathExecutable,
            Arguments = "-u " + QuoteForProcessStart(trimmedPath),
            UseShellExecute = false,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            CreateNoWindow = true
        };

        using Process process = Process.Start(startInfo);
        string output = process.StandardOutput.ReadToEnd().Trim();
        string error = process.StandardError.ReadToEnd().Trim();
        process.WaitForExit();

        if (process.ExitCode != 0 || string.IsNullOrWhiteSpace(output))
        {
            throw new InvalidOperationException($"Failed to convert path '{trimmedPath}' to Unix format: {error}".Trim());
        }

        return output;
    }

    public static string ConvertToWindowsPath(string unixOrWindowsPath)
    {
        if (string.IsNullOrWhiteSpace(unixOrWindowsPath))
        {
            return string.Empty;
        }

        string trimmedPath = unixOrWindowsPath.Trim();
        if (LooksLikeWindowsPath(trimmedPath))
        {
            return trimmedPath.Replace('/', '\\');
        }

        string winePathExecutable = Path.Combine(Environment.SystemDirectory, "winepath.exe");
        ProcessStartInfo startInfo = new ProcessStartInfo
        {
            FileName = winePathExecutable,
            Arguments = "-w " + QuoteForProcessStart(trimmedPath),
            UseShellExecute = false,
            RedirectStandardOutput = true,
            RedirectStandardError = true,
            CreateNoWindow = true
        };

        using Process process = Process.Start(startInfo);
        string output = process.StandardOutput.ReadToEnd().Trim();
        string error = process.StandardError.ReadToEnd().Trim();
        process.WaitForExit();

        if (process.ExitCode != 0 || string.IsNullOrWhiteSpace(output))
        {
            throw new InvalidOperationException($"Failed to convert path '{trimmedPath}' to Windows format: {error}".Trim());
        }

        return output;
    }

    public static string NormalizeSteamHostPath(string pathValue)
    {
        string unixPath = ConvertToUnixPath(pathValue);
        return ResolveDosDevicesPath(unixPath);
    }

    internal static bool LooksLikeWindowsPath(string pathValue)
    {
        return !string.IsNullOrWhiteSpace(pathValue)
            && pathValue.Length >= 3
            && char.IsLetter(pathValue[0])
            && pathValue[1] == ':'
            && (pathValue[2] == '\\' || pathValue[2] == '/');
    }

    internal static string ResolveDosDevicesPath(string unixPath)
    {
        if (string.IsNullOrWhiteSpace(unixPath))
        {
            return string.Empty;
        }

        const string marker = "/dosdevices/";
        int markerIndex = unixPath.IndexOf(marker, StringComparison.Ordinal);
        if (markerIndex < 0)
        {
            return unixPath;
        }

        string prefix = unixPath[..(markerIndex + marker.Length)];
        string remainder = unixPath[(markerIndex + marker.Length)..];
        int separatorIndex = remainder.IndexOf('/');
        if (separatorIndex < 0)
        {
            return unixPath;
        }

        string driveSegment = remainder[..separatorIndex];
        string restOfPath = remainder[separatorIndex..];
        if (driveSegment.Length != 2 || driveSegment[1] != ':' || !char.IsLetter(driveSegment[0]))
        {
            return unixPath;
        }

        try
        {
            string driveRoot = ConvertToUnixPath(char.ToUpperInvariant(driveSegment[0]) + @":\");
            if (string.IsNullOrWhiteSpace(driveRoot))
            {
                return unixPath;
            }

            return driveRoot.TrimEnd('/') + restOfPath;
        }
        catch (Exception)
        {
            return unixPath;
        }
    }

    internal static string QuoteForCommand(string value)
    {
        string normalizedValue = value ?? string.Empty;
        return "\"" + normalizedValue.Replace("\"", "\\\"") + "\"";
    }

    internal static string QuoteForProcessStart(string value)
    {
        string normalizedValue = value ?? string.Empty;
        return "\"" + normalizedValue.Replace("\"", "\\\"") + "\"";
    }

}
