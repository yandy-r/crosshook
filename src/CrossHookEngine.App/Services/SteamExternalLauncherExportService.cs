using System;
using System.IO;
using System.Text;

namespace CrossHookEngine.App.Services;

public sealed class SteamExternalLauncherExportRequest
{
    public string LauncherName { get; set; } = string.Empty;

    public string TrainerPath { get; set; } = string.Empty;

    public string LauncherIconPath { get; set; } = string.Empty;

    public string SteamAppId { get; set; } = string.Empty;

    public string SteamCompatDataPath { get; set; } = string.Empty;

    public string SteamProtonPath { get; set; } = string.Empty;

    public string SteamClientInstallPath { get; set; } = string.Empty;

    public string TargetHomePath { get; set; } = string.Empty;
}

public sealed class SteamExternalLauncherExportValidationResult
{
    public bool IsValid { get; }

    public string ErrorMessage { get; }

    public SteamExternalLauncherExportValidationResult(bool isValid, string errorMessage = "")
    {
        IsValid = isValid;
        ErrorMessage = errorMessage ?? string.Empty;
    }
}

public sealed class SteamExternalLauncherExportResult
{
    public string DisplayName { get; init; } = string.Empty;

    public string LauncherSlug { get; init; } = string.Empty;

    public string ScriptPath { get; init; } = string.Empty;

    public string DesktopEntryPath { get; init; } = string.Empty;
}

public static class SteamExternalLauncherExportService
{
    private const string LauncherDirectoryRelativePath = ".local/share/crosshook/launchers";
    private const string DesktopEntryDirectoryRelativePath = ".local/share/applications";

    public static SteamExternalLauncherExportValidationResult Validate(SteamExternalLauncherExportRequest request)
    {
        ArgumentNullException.ThrowIfNull(request);

        if (string.IsNullOrWhiteSpace(request.TrainerPath))
        {
            return new SteamExternalLauncherExportValidationResult(false, "External launcher export requires a trainer path.");
        }

        if (string.IsNullOrWhiteSpace(request.SteamAppId))
        {
            return new SteamExternalLauncherExportValidationResult(false, "External launcher export requires a Steam App ID.");
        }

        if (string.IsNullOrWhiteSpace(request.SteamCompatDataPath))
        {
            return new SteamExternalLauncherExportValidationResult(false, "External launcher export requires a compatdata path.");
        }

        if (string.IsNullOrWhiteSpace(request.SteamProtonPath))
        {
            return new SteamExternalLauncherExportValidationResult(false, "External launcher export requires a Proton path.");
        }

        if (string.IsNullOrWhiteSpace(request.SteamClientInstallPath))
        {
            return new SteamExternalLauncherExportValidationResult(false, "External launcher export requires a Steam client install path.");
        }

        if (string.IsNullOrWhiteSpace(request.TargetHomePath))
        {
            return new SteamExternalLauncherExportValidationResult(false, "External launcher export requires a host home path.");
        }

        if (!string.IsNullOrWhiteSpace(request.LauncherIconPath))
        {
            string normalizedLauncherIconPath = NormalizeHostUnixPath(request.LauncherIconPath);
            string resolvedLauncherIconPath = ResolveWritablePath(normalizedLauncherIconPath);
            if (!File.Exists(resolvedLauncherIconPath))
            {
                return new SteamExternalLauncherExportValidationResult(false, "External launcher export icon path does not exist.");
            }

            string extension = Path.GetExtension(normalizedLauncherIconPath);
            if (!extension.Equals(".png", StringComparison.OrdinalIgnoreCase)
                && !extension.Equals(".jpg", StringComparison.OrdinalIgnoreCase)
                && !extension.Equals(".jpeg", StringComparison.OrdinalIgnoreCase))
            {
                return new SteamExternalLauncherExportValidationResult(false, "External launcher export icon must be a PNG or JPG image.");
            }
        }

        return new SteamExternalLauncherExportValidationResult(true);
    }

    public static SteamExternalLauncherExportResult ExportLaunchers(SteamExternalLauncherExportRequest request)
    {
        ArgumentNullException.ThrowIfNull(request);

        SteamExternalLauncherExportValidationResult validation = Validate(request);
        if (!validation.IsValid)
        {
            throw new InvalidOperationException(validation.ErrorMessage);
        }

        string displayName = ResolveDisplayName(request.LauncherName, request.SteamAppId, request.TrainerPath);
        string launcherSlug = SanitizeLauncherSlug(displayName);
        string targetHomePath = ResolveTargetHomePath(request.TargetHomePath, request.SteamClientInstallPath);
        if (string.IsNullOrWhiteSpace(targetHomePath))
        {
            throw new InvalidOperationException("Could not resolve a host home path for launcher export.");
        }

        string scriptPath = CombineHostUnixPath(targetHomePath, LauncherDirectoryRelativePath, $"{launcherSlug}-trainer.sh");
        string desktopEntryPath = CombineHostUnixPath(targetHomePath, DesktopEntryDirectoryRelativePath, $"crosshook-{launcherSlug}-trainer.desktop");

        WriteHostTextFile(scriptPath, BuildTrainerScriptContent(request, displayName));
        WriteHostTextFile(desktopEntryPath, BuildDesktopEntryContent(displayName, scriptPath, request.LauncherIconPath));

        return new SteamExternalLauncherExportResult
        {
            DisplayName = displayName,
            LauncherSlug = launcherSlug,
            ScriptPath = scriptPath,
            DesktopEntryPath = desktopEntryPath
        };
    }

    public static string ResolveTargetHomePath(string preferredHomePath, string steamClientInstallPath)
    {
        string normalizedPreferredHomePath = NormalizeHostUnixPath(preferredHomePath);
        if (LooksLikeUsableHostUnixPath(normalizedPreferredHomePath))
        {
            return normalizedPreferredHomePath;
        }

        string normalizedSteamClientInstallPath = NormalizeHostUnixPath(steamClientInstallPath);
        if (TryResolveHomeFromSteamClientInstallPath(normalizedSteamClientInstallPath, out string derivedHomePath))
        {
            return derivedHomePath;
        }

        return normalizedPreferredHomePath;
    }

    internal static string ResolveDisplayName(string preferredName, string steamAppId, string trainerPath)
    {
        if (!string.IsNullOrWhiteSpace(preferredName))
        {
            return preferredName.Trim();
        }

        string trainerName = Path.GetFileNameWithoutExtension(trainerPath ?? string.Empty)?.Trim() ?? string.Empty;
        if (!string.IsNullOrWhiteSpace(trainerName))
        {
            return trainerName;
        }

        return $"steam-{steamAppId}-trainer";
    }

    internal static string SanitizeLauncherSlug(string value)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            return "crosshook-trainer";
        }

        StringBuilder builder = new StringBuilder(value.Length);
        bool lastCharacterWasSeparator = false;

        foreach (char character in value.Trim().ToLowerInvariant())
        {
            if (char.IsLetterOrDigit(character))
            {
                builder.Append(character);
                lastCharacterWasSeparator = false;
                continue;
            }

            if (lastCharacterWasSeparator)
            {
                continue;
            }

            builder.Append('-');
            lastCharacterWasSeparator = true;
        }

        string slug = builder.ToString().Trim('-');
        return string.IsNullOrWhiteSpace(slug) ? "crosshook-trainer" : slug;
    }

    internal static string CombineHostUnixPath(string rootPath, params string[] segments)
    {
        string normalizedRootPath = NormalizeHostUnixPath(rootPath).TrimEnd('/');
        if (string.IsNullOrWhiteSpace(normalizedRootPath))
        {
            return string.Empty;
        }

        StringBuilder builder = new StringBuilder(normalizedRootPath);
        foreach (string segment in segments)
        {
            string normalizedSegment = NormalizeHostUnixPath(segment).Trim('/');
            if (string.IsNullOrWhiteSpace(normalizedSegment))
            {
                continue;
            }

            builder.Append('/');
            builder.Append(normalizedSegment);
        }

        return builder.ToString();
    }

    internal static string BuildTrainerScriptContent(SteamExternalLauncherExportRequest request, string displayName)
    {
        StringBuilder builder = new StringBuilder();
        _ = builder.AppendLine("#!/usr/bin/env bash");
        _ = builder.AppendLine("set -euo pipefail");
        _ = builder.AppendLine();
        _ = builder.AppendLine($"# {displayName} - Trainer launcher");
        _ = builder.AppendLine("# Generated by CrossHook");
        _ = builder.AppendLine("# https://github.com/yandy-r/crosshook");
        _ = builder.AppendLine("# Launch this after the Steam game has reached the in-game menu.");
        _ = builder.AppendLine($"export STEAM_COMPAT_DATA_PATH={ToShellSingleQuotedLiteral(request.SteamCompatDataPath)}");
        _ = builder.AppendLine($"export STEAM_COMPAT_CLIENT_INSTALL_PATH={ToShellSingleQuotedLiteral(request.SteamClientInstallPath)}");
        _ = builder.AppendLine("export WINEPREFIX=\"$STEAM_COMPAT_DATA_PATH/pfx\"");
        _ = builder.AppendLine($"PROTON={ToShellSingleQuotedLiteral(request.SteamProtonPath)}");
        _ = builder.AppendLine($"TRAINER_WINDOWS_PATH={ToShellSingleQuotedLiteral(request.TrainerPath)}");
        _ = builder.AppendLine("exec \"$PROTON\" run \"$TRAINER_WINDOWS_PATH\"");
        return builder.ToString();
    }

    internal static string BuildDesktopEntryContent(string displayName, string scriptPath, string launcherIconPath)
    {
        StringBuilder builder = new StringBuilder();
        _ = builder.AppendLine("[Desktop Entry]");
        _ = builder.AppendLine("Type=Application");
        _ = builder.AppendLine("Version=1.0");
        _ = builder.AppendLine($"Name={displayName} - Trainer");
        _ = builder.AppendLine($"Comment=Trainer launcher for {displayName}. Generated by CrossHook: https://github.com/yandy-r/crosshook");
        _ = builder.AppendLine($"Exec=/bin/bash {EscapeDesktopExecArgument(scriptPath)}");
        _ = builder.AppendLine("Terminal=false");
        _ = builder.AppendLine("Categories=Game;");
        _ = builder.AppendLine($"Icon={ResolveDesktopIconValue(launcherIconPath)}");
        _ = builder.AppendLine("StartupNotify=false");
        return builder.ToString();
    }

    private static void WriteHostTextFile(string hostPath, string content)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(hostPath);

        string writablePath = ResolveWritablePath(hostPath);
        string directoryPath = Path.GetDirectoryName(writablePath)
            ?? throw new InvalidOperationException($"Could not resolve a parent directory for '{hostPath}'.");

        Directory.CreateDirectory(directoryPath);
        File.WriteAllText(writablePath, content.Replace("\r\n", "\n"));
    }

    private static bool TryResolveHomeFromSteamClientInstallPath(string steamClientInstallPath, out string homePath)
    {
        const string LocalShareSteamSuffix = "/.local/share/Steam";
        const string DotSteamRootSuffix = "/.steam/root";

        if (string.IsNullOrWhiteSpace(steamClientInstallPath))
        {
            homePath = string.Empty;
            return false;
        }

        if (steamClientInstallPath.EndsWith(LocalShareSteamSuffix, StringComparison.Ordinal))
        {
            homePath = steamClientInstallPath[..^LocalShareSteamSuffix.Length];
            return !string.IsNullOrWhiteSpace(homePath);
        }

        if (steamClientInstallPath.EndsWith(DotSteamRootSuffix, StringComparison.Ordinal))
        {
            homePath = steamClientInstallPath[..^DotSteamRootSuffix.Length];
            return !string.IsNullOrWhiteSpace(homePath);
        }

        homePath = string.Empty;
        return false;
    }

    private static string ResolveWritablePath(string hostPath)
    {
        if (OperatingSystem.IsWindows() && hostPath.StartsWith("/", StringComparison.Ordinal))
        {
            return SteamLaunchService.ConvertToWindowsPath(hostPath);
        }

        return hostPath;
    }

    private static string ResolveDesktopIconValue(string launcherIconPath)
    {
        string normalizedLauncherIconPath = NormalizeHostUnixPath(launcherIconPath);
        return string.IsNullOrWhiteSpace(normalizedLauncherIconPath)
            ? "applications-games"
            : normalizedLauncherIconPath;
    }

    private static string NormalizeHostUnixPath(string path)
    {
        return (path ?? string.Empty).Trim().Replace('\\', '/');
    }

    private static bool LooksLikeUsableHostUnixPath(string path)
    {
        return !string.IsNullOrWhiteSpace(path)
            && path.StartsWith("/", StringComparison.Ordinal)
            && !path.Contains("/compatdata/", StringComparison.Ordinal);
    }

    private static string ToShellSingleQuotedLiteral(string value)
    {
        string normalizedValue = value ?? string.Empty;
        return "'" + normalizedValue.Replace("'", "'\"'\"'") + "'";
    }

    private static string EscapeDesktopExecArgument(string value)
    {
        string normalizedValue = value ?? string.Empty;
        return normalizedValue
            .Replace("\\", "\\\\")
            .Replace(" ", "\\ ")
            .Replace("\"", "\\\"");
    }
}
