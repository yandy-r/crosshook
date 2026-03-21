using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Linq;
using System.Text;

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
            string dosDevicesDirectoryPath = unixPath[..(markerIndex + marker.Length - 1)];
            string driveRoot = ResolveDosDeviceLinkTarget(dosDevicesDirectoryPath, driveSegment);
            if (string.IsNullOrWhiteSpace(driveRoot))
            {
                string scannedHostPath = ResolveMountedHostPathByScanning(restOfPath, GetMountedHostSearchRoots());
                return string.IsNullOrWhiteSpace(scannedHostPath) ? unixPath : scannedHostPath;
            }

            string normalizedDriveRoot = driveRoot.Replace('\\', '/').TrimEnd('/');
            string normalizedRestOfPath = restOfPath.TrimStart('/');
            if (string.IsNullOrWhiteSpace(normalizedRestOfPath))
            {
                return normalizedDriveRoot;
            }

            return Path.Combine(normalizedDriveRoot, normalizedRestOfPath).Replace('\\', '/');
        }
        catch (Exception)
        {
            return unixPath;
        }
    }

    internal static string ResolveDosDeviceLinkTarget(string dosDevicesDirectoryPath, string driveSegment)
    {
        if (string.IsNullOrWhiteSpace(dosDevicesDirectoryPath) || string.IsNullOrWhiteSpace(driveSegment))
        {
            return string.Empty;
        }

        string normalizedDosDevicesDirectoryPath = dosDevicesDirectoryPath.Replace('\\', '/');
        if (!Directory.Exists(normalizedDosDevicesDirectoryPath))
        {
            return string.Empty;
        }

        DirectoryInfo dosDevicesDirectory = new DirectoryInfo(normalizedDosDevicesDirectoryPath);
        FileSystemInfo driveEntry = dosDevicesDirectory
            .EnumerateFileSystemInfos()
            .FirstOrDefault(entry => string.Equals(entry.Name, driveSegment, StringComparison.OrdinalIgnoreCase));
        if (driveEntry is null)
        {
            return string.Empty;
        }

        string linkTarget = driveEntry.LinkTarget;
        if (string.IsNullOrWhiteSpace(linkTarget))
        {
            FileSystemInfo resolvedTarget = driveEntry.ResolveLinkTarget(returnFinalTarget: true);
            if (resolvedTarget is not null)
            {
                return Path.GetFullPath(resolvedTarget.FullName).Replace('\\', '/');
            }

            string hostResolvedTarget = ResolveDosDeviceLinkTargetViaReadlink(normalizedDosDevicesDirectoryPath, driveSegment);
            if (string.IsNullOrWhiteSpace(hostResolvedTarget))
            {
                return string.Empty;
            }

            return hostResolvedTarget;
        }

        string normalizedLinkTarget = linkTarget.Replace('\\', '/');
        if (Path.IsPathRooted(normalizedLinkTarget))
        {
            return Path.GetFullPath(normalizedLinkTarget).Replace('\\', '/');
        }

        return Path.GetFullPath(Path.Combine(normalizedDosDevicesDirectoryPath, normalizedLinkTarget)).Replace('\\', '/');
    }

    internal static string ResolveMountedHostPathByScanning(string restOfPath, IEnumerable<string> searchRoots)
    {
        if (string.IsNullOrWhiteSpace(restOfPath) || searchRoots is null)
        {
            return string.Empty;
        }

        string normalizedRelativePath = restOfPath.TrimStart('/').Replace('\\', '/');
        if (string.IsNullOrWhiteSpace(normalizedRelativePath))
        {
            return string.Empty;
        }

        HashSet<string> matches = new HashSet<string>(StringComparer.Ordinal);
        foreach (string searchRoot in searchRoots)
        {
            if (string.IsNullOrWhiteSpace(searchRoot) || !Directory.Exists(searchRoot))
            {
                continue;
            }

            foreach (string baseDirectory in EnumerateMountedBaseDirectories(searchRoot))
            {
                string candidatePath = Path.Combine(baseDirectory, normalizedRelativePath).Replace('\\', '/');
                if (Directory.Exists(candidatePath) || File.Exists(candidatePath))
                {
                    _ = matches.Add(candidatePath);
                }
            }
        }

        return matches.Count == 1 ? matches.First() : string.Empty;
    }

    internal static IEnumerable<string> GetMountedHostSearchRoots()
    {
        return
        [
            "/mnt",
            "/media",
            "/run/media",
            "/var/run/media"
        ];
    }

    private static IEnumerable<string> EnumerateMountedBaseDirectories(string searchRoot)
    {
        if (!Directory.Exists(searchRoot))
        {
            yield break;
        }

        foreach (string firstLevelDirectory in Directory.EnumerateDirectories(searchRoot))
        {
            yield return firstLevelDirectory.Replace('\\', '/');

            foreach (string secondLevelDirectory in Directory.EnumerateDirectories(firstLevelDirectory))
            {
                yield return secondLevelDirectory.Replace('\\', '/');
            }
        }
    }

    internal static string ResolveDosDeviceLinkTargetViaReadlink(string dosDevicesDirectoryPath, string driveSegment)
    {
        if (string.IsNullOrWhiteSpace(dosDevicesDirectoryPath) || string.IsNullOrWhiteSpace(driveSegment))
        {
            return string.Empty;
        }

        string dosDeviceLinkPath = (dosDevicesDirectoryPath.TrimEnd('/') + "/" + driveSegment).Replace('\\', '/');
        string unixOutputPath = $"/tmp/crosshook-readlink-{Guid.NewGuid():N}.txt";
        string windowsOutputPath = ConvertToWindowsPath(unixOutputPath);
        string shellCommand =
            "readlink -f " + QuoteForShellSingleQuotedLiteral(dosDeviceLinkPath)
            + " > " + QuoteForShellSingleQuotedLiteral(unixOutputPath);

        try
        {
            ProcessStartInfo startInfo = new ProcessStartInfo
            {
                FileName = Path.Combine(Environment.SystemDirectory, "start.exe"),
                Arguments = "/unix /bin/sh -lc " + QuoteForCommand(shellCommand),
                UseShellExecute = false,
                CreateNoWindow = true
            };

            using Process process = Process.Start(startInfo);
            process.WaitForExit();

            if (process.ExitCode != 0 || !File.Exists(windowsOutputPath))
            {
                return string.Empty;
            }

            string output = File.ReadAllText(windowsOutputPath).Trim();
            return string.IsNullOrWhiteSpace(output)
                ? string.Empty
                : output.Replace('\\', '/');
        }
        catch (Exception)
        {
            return string.Empty;
        }
        finally
        {
            try
            {
                if (!string.IsNullOrWhiteSpace(windowsOutputPath) && File.Exists(windowsOutputPath))
                {
                    File.Delete(windowsOutputPath);
                }
            }
            catch (Exception)
            {
            }
        }
    }

    internal static string DescribeDosDevicesResolution(string unixPath)
    {
        if (string.IsNullOrWhiteSpace(unixPath))
        {
            return "input=<empty>";
        }

        const string marker = "/dosdevices/";
        int markerIndex = unixPath.IndexOf(marker, StringComparison.Ordinal);
        if (markerIndex < 0)
        {
            return $"input={unixPath} | marker=absent";
        }

        string remainder = unixPath[(markerIndex + marker.Length)..];
        int separatorIndex = remainder.IndexOf('/');
        if (separatorIndex < 0)
        {
            return $"input={unixPath} | marker=present | remainder={remainder} | separator=absent";
        }

        string dosDevicesDirectoryPath = unixPath[..(markerIndex + marker.Length - 1)].Replace('\\', '/');
        string driveSegment = remainder[..separatorIndex];
        string restOfPath = remainder[separatorIndex..];
        bool directoryExists = Directory.Exists(dosDevicesDirectoryPath);

        StringBuilder builder = new StringBuilder();
        _ = builder.Append($"input={unixPath}");
        _ = builder.Append($" | dosdevices_dir={dosDevicesDirectoryPath}");
        _ = builder.Append($" | dir_exists={directoryExists}");
        _ = builder.Append($" | drive_segment={driveSegment}");
        _ = builder.Append($" | rest={restOfPath}");

        if (!directoryExists)
        {
            return builder.ToString();
        }

        try
        {
            DirectoryInfo dosDevicesDirectory = new DirectoryInfo(dosDevicesDirectoryPath);
            string[] entryNames = dosDevicesDirectory
                .EnumerateFileSystemInfos()
                .Select(entry => entry.Name)
                .OrderBy(name => name, StringComparer.OrdinalIgnoreCase)
                .ToArray();
            _ = builder.Append($" | entries=[{string.Join(", ", entryNames)}]");

            FileSystemInfo driveEntry = dosDevicesDirectory
                .EnumerateFileSystemInfos()
                .FirstOrDefault(entry => string.Equals(entry.Name, driveSegment, StringComparison.OrdinalIgnoreCase));
            if (driveEntry is null)
            {
                _ = builder.Append(" | drive_entry=<missing>");
                return builder.ToString();
            }

            _ = builder.Append($" | drive_entry={driveEntry.FullName.Replace('\\', '/')}");
            _ = builder.Append($" | link_target={driveEntry.LinkTarget ?? "<null>"}");

            FileSystemInfo resolvedTarget = driveEntry.ResolveLinkTarget(returnFinalTarget: true);
            _ = builder.Append($" | resolved_target={(resolvedTarget is null ? "<null>" : resolvedTarget.FullName.Replace('\\', '/'))}");

            string driveRoot = ResolveDosDeviceLinkTarget(dosDevicesDirectoryPath, driveSegment);
            _ = builder.Append($" | computed_drive_root={driveRoot}");
            return builder.ToString();
        }
        catch (Exception ex)
        {
            _ = builder.Append($" | error={ex.GetType().Name}: {ex.Message}");
            return builder.ToString();
        }
    }

    internal static string QuoteForCommand(string value)
    {
        string normalizedValue = value ?? string.Empty;
        return "\"" + normalizedValue.Replace("\"", "\\\"") + "\"";
    }

    internal static string QuoteForShellSingleQuotedLiteral(string value)
    {
        string normalizedValue = value ?? string.Empty;
        return "'" + normalizedValue.Replace("'", "'\"'\"'") + "'";
    }

    internal static string QuoteForProcessStart(string value)
    {
        string normalizedValue = value ?? string.Empty;
        return "\"" + normalizedValue.Replace("\"", "\\\"") + "\"";
    }

}
