using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text;

namespace CrossHookEngine.App.Services;

public enum SteamAutoPopulateFieldState
{
    NotFound,
    Found,
    Ambiguous
}

public sealed class SteamAutoPopulateRequest
{
    public string GamePath { get; set; } = string.Empty;

    public string SteamClientInstallPath { get; set; } = string.Empty;
}

public sealed class SteamAutoPopulateResult
{
    public SteamAutoPopulateFieldState SteamAppIdState { get; init; }

    public string SteamAppId { get; init; } = string.Empty;

    public SteamAutoPopulateFieldState SteamCompatDataPathState { get; init; }

    public string SteamCompatDataPath { get; init; } = string.Empty;

    public SteamAutoPopulateFieldState SteamProtonPathState { get; init; }

    public string SteamProtonPath { get; init; } = string.Empty;

    public IReadOnlyList<string> Diagnostics { get; init; } = Array.Empty<string>();

    public IReadOnlyList<string> ManualHints { get; init; } = Array.Empty<string>();

    public bool HasAnyMatch =>
        SteamAppIdState == SteamAutoPopulateFieldState.Found
        || SteamCompatDataPathState == SteamAutoPopulateFieldState.Found
        || SteamProtonPathState == SteamAutoPopulateFieldState.Found;
}

public static class SteamAutoPopulateService
{
    public static SteamAutoPopulateResult AttemptAutoPopulate(SteamAutoPopulateRequest request)
    {
        ArgumentNullException.ThrowIfNull(request);

        List<string> diagnostics = new List<string>();
        List<string> manualHints = new List<string>();

        string normalizedGamePath = NormalizePathForHostLookup(request.GamePath);
        if (string.IsNullOrWhiteSpace(normalizedGamePath))
        {
            diagnostics.Add("No game executable path was provided for Steam auto-populate.");
            AddDefaultManualHints(manualHints, Array.Empty<string>(), Array.Empty<string>(), string.Empty);
            return CreateResult(
                SteamAutoPopulateFieldState.NotFound,
                string.Empty,
                SteamAutoPopulateFieldState.NotFound,
                string.Empty,
                SteamAutoPopulateFieldState.NotFound,
                string.Empty,
                diagnostics,
                manualHints);
        }

        diagnostics.Add($"Normalized game executable path: {normalizedGamePath}");
        if (!File.Exists(normalizedGamePath))
        {
            diagnostics.Add("The normalized game path does not currently exist on the host filesystem. CrossHook will still attempt a manifest match.");
        }

        List<string> steamRootCandidates = DiscoverSteamRootCandidates(request.SteamClientInstallPath, diagnostics);
        IReadOnlyList<SteamLibraryInfo> libraries = DiscoverSteamLibraries(steamRootCandidates, diagnostics);
        string resolvedGamePath = ResolveGamePathAgainstSteamLibraries(normalizedGamePath, libraries, diagnostics);
        if (!string.Equals(resolvedGamePath, normalizedGamePath, StringComparison.Ordinal))
        {
            diagnostics.Add($"Resolved game executable path against Steam libraries: {resolvedGamePath}");
        }

        SteamGameMatchSelection matchSelection = FindGameMatch(resolvedGamePath, libraries, diagnostics);

        SteamAutoPopulateFieldState appIdState = matchSelection.State;
        string steamAppId = matchSelection.Match?.SteamAppId ?? string.Empty;

        SteamAutoPopulateFieldState compatDataState = SteamAutoPopulateFieldState.NotFound;
        string compatDataPath = string.Empty;

        if (matchSelection.Match is not null)
        {
            string candidateCompatDataPath = CombineNormalizedPathSegments(
                matchSelection.Match.LibraryPath,
                "steamapps",
                "compatdata",
                matchSelection.Match.SteamAppId);

            if (Directory.Exists(candidateCompatDataPath))
            {
                compatDataState = SteamAutoPopulateFieldState.Found;
                compatDataPath = candidateCompatDataPath;
                diagnostics.Add($"Detected compatdata path: {candidateCompatDataPath}");
            }
            else
            {
                diagnostics.Add($"Derived compatdata path does not exist yet: {candidateCompatDataPath}");
            }
        }

        SteamAutoPopulateFieldState protonState = SteamAutoPopulateFieldState.NotFound;
        string protonPath = string.Empty;

        if (matchSelection.Match is not null)
        {
            ProtonResolution protonResolution = ResolveProtonPath(matchSelection.Match.SteamAppId, steamRootCandidates, diagnostics);
            protonState = protonResolution.State;
            protonPath = protonResolution.ProtonPath;
        }

        AddDefaultManualHints(manualHints, steamRootCandidates, libraries.Select(library => library.LibraryPath), steamAppId);

        if (matchSelection.Match is null && appIdState == SteamAutoPopulateFieldState.NotFound)
        {
            manualHints.Add("Select the game executable from inside a Steam library under steamapps/common so CrossHook can match it against Steam manifests.");
        }

        if (compatDataState != SteamAutoPopulateFieldState.Found && !string.IsNullOrWhiteSpace(steamAppId))
        {
            foreach (SteamLibraryInfo library in libraries)
            {
                manualHints.Add($"Compatdata is usually under: {CombineNormalizedPathSegments(library.LibraryPath, "steamapps", "compatdata", steamAppId)}");
            }
        }

        if (protonState != SteamAutoPopulateFieldState.Found)
        {
            foreach (string steamRootCandidate in steamRootCandidates)
            {
                manualHints.Add($"Proton is usually under: {CombineNormalizedPathSegments(steamRootCandidate, "steamapps", "common")}");
                manualHints.Add($"Custom Proton tools are usually under: {CombineNormalizedPathSegments(steamRootCandidate, "compatibilitytools.d")}");
            }

            foreach (string systemCompatToolRoot in GetSystemCompatToolRoots())
            {
                manualHints.Add($"System Steam compat tools may also be under: {systemCompatToolRoot}");
            }
        }

        return CreateResult(
            appIdState,
            steamAppId,
            compatDataState,
            compatDataPath,
            protonState,
            protonPath,
            diagnostics,
            manualHints);
    }

    internal static List<string> DiscoverSteamRootCandidates(string preferredSteamClientInstallPath, List<string> diagnostics)
    {
        HashSet<string> candidates = new HashSet<string>(StringComparer.Ordinal);
        AddDirectoryCandidate(candidates, preferredSteamClientInstallPath, diagnostics, "Configured Steam client path");

        if (candidates.Count > 0)
        {
            return candidates.ToList();
        }

        string homePath = Environment.GetEnvironmentVariable("HOME") ?? string.Empty;
        if (string.IsNullOrWhiteSpace(homePath))
        {
            homePath = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
        }

        if (!string.IsNullOrWhiteSpace(homePath))
        {
            string normalizedHomePath = NormalizePathForHostLookup(homePath);
            AddDirectoryCandidate(candidates, CombineNormalizedPathSegments(normalizedHomePath, ".steam", "root"), diagnostics, "Default Steam root");
            AddDirectoryCandidate(candidates, CombineNormalizedPathSegments(normalizedHomePath, ".local", "share", "Steam"), diagnostics, "Default local Steam install");
        }

        return candidates.ToList();
    }

    internal static IReadOnlyList<SteamLibraryInfo> DiscoverSteamLibraries(IEnumerable<string> steamRootCandidates, List<string> diagnostics)
    {
        List<SteamLibraryInfo> libraries = new List<SteamLibraryInfo>();
        HashSet<string> seenLibraryPaths = new HashSet<string>(StringComparer.Ordinal);

        foreach (string steamRootCandidate in steamRootCandidates)
        {
            string normalizedSteamRoot = NormalizePathForHostLookup(steamRootCandidate);
            if (string.IsNullOrWhiteSpace(normalizedSteamRoot) || !Directory.Exists(normalizedSteamRoot))
            {
                continue;
            }

            TryAddLibrary(libraries, seenLibraryPaths, normalizedSteamRoot, diagnostics, "Steam root");

            string libraryFoldersPath = CombineNormalizedPathSegments(normalizedSteamRoot, "steamapps", "libraryfolders.vdf");
            if (!File.Exists(libraryFoldersPath))
            {
                diagnostics.Add($"Steam library file not found: {libraryFoldersPath}");
                continue;
            }

            try
            {
                SteamKeyValueNode parsed = ParseKeyValueFile(libraryFoldersPath);
                SteamKeyValueNode libraryFoldersNode = parsed.GetChild("libraryfolders") ?? parsed;

                foreach (KeyValuePair<string, SteamKeyValueNode> entry in libraryFoldersNode.Children)
                {
                    string libraryPath = string.Empty;
                    if (!string.IsNullOrWhiteSpace(entry.Value.Value))
                    {
                        libraryPath = entry.Value.Value;
                    }
                    else if (entry.Value.GetChild("path") is SteamKeyValueNode pathNode)
                    {
                        libraryPath = pathNode.Value;
                    }

                    TryAddLibrary(libraries, seenLibraryPaths, libraryPath, diagnostics, $"Library folder entry '{entry.Key}'");
                }
            }
            catch (Exception ex)
            {
                diagnostics.Add($"Failed to parse Steam library file '{libraryFoldersPath}': {ex.Message}");
            }
        }

        return libraries;
    }

    internal static SteamGameMatchSelection FindGameMatch(
        string normalizedGamePath,
        IReadOnlyList<SteamLibraryInfo> libraries,
        List<string> diagnostics)
    {
        List<SteamGameMatch> matches = new List<SteamGameMatch>();

        foreach (SteamLibraryInfo library in libraries)
        {
            IEnumerable<string> manifestPaths = SafeEnumerateFiles(library.SteamAppsPath, "appmanifest_*.acf");
            foreach (string manifestPath in manifestPaths)
            {
                string normalizedManifestPath = NormalizeHostStylePath(manifestPath);
                try
                {
                    SteamKeyValueNode manifestRoot = ParseKeyValueFile(manifestPath);
                    SteamKeyValueNode appStateNode = manifestRoot.GetChild("AppState") ?? manifestRoot;
                    string steamAppId = appStateNode.GetChild("appid")?.Value;
                    if (string.IsNullOrWhiteSpace(steamAppId))
                    {
                        steamAppId = ExtractAppIdFromManifestPath(manifestPath);
                    }

                    string installDirectoryName = appStateNode.GetChild("installdir")?.Value;
                    if (string.IsNullOrWhiteSpace(steamAppId) || string.IsNullOrWhiteSpace(installDirectoryName))
                    {
                        continue;
                    }

                    string installDirectoryPath = CombineNormalizedPathSegments(
                        library.SteamAppsPath,
                        "common",
                        installDirectoryName);

                    if (!PathIsSameOrChild(normalizedGamePath, installDirectoryPath))
                    {
                        continue;
                    }

                    matches.Add(new SteamGameMatch(steamAppId, library.LibraryPath, installDirectoryPath, normalizedManifestPath));
                }
                catch (Exception ex)
                {
                    diagnostics.Add($"Failed to parse app manifest '{normalizedManifestPath}': {ex.Message}");
                }
            }
        }

        if (matches.Count == 0)
        {
            diagnostics.Add("No Steam app manifest matched the selected game executable path.");
            return new SteamGameMatchSelection(SteamAutoPopulateFieldState.NotFound, null);
        }

        List<SteamGameMatch> distinctMatches = matches
            .GroupBy(match => $"{match.SteamAppId}|{match.LibraryPath}", StringComparer.Ordinal)
            .Select(group => group.First())
            .ToList();

        if (distinctMatches.Count == 1)
        {
            SteamGameMatch match = distinctMatches[0];
            diagnostics.Add($"Matched Steam App ID {match.SteamAppId} using manifest: {match.ManifestPath}");
            return new SteamGameMatchSelection(SteamAutoPopulateFieldState.Found, match);
        }

        diagnostics.Add("Multiple Steam manifests matched the selected executable. Auto-populate will not guess the Steam App ID.");
        foreach (SteamGameMatch match in distinctMatches)
        {
            diagnostics.Add($"Conflicting manifest candidate: App ID {match.SteamAppId} in {match.LibraryPath}");
        }

        return new SteamGameMatchSelection(SteamAutoPopulateFieldState.Ambiguous, null);
    }

    internal static string ResolveGamePathAgainstSteamLibraries(
        string normalizedGamePath,
        IReadOnlyList<SteamLibraryInfo> libraries,
        List<string> diagnostics)
    {
        if (string.IsNullOrWhiteSpace(normalizedGamePath) || !normalizedGamePath.Contains("/dosdevices/", StringComparison.Ordinal))
        {
            return normalizedGamePath;
        }

        string restOfPath = ExtractDosDevicesRestOfPath(normalizedGamePath);
        if (string.IsNullOrWhiteSpace(restOfPath))
        {
            return normalizedGamePath;
        }

        HashSet<string> matches = new HashSet<string>(StringComparer.Ordinal);
        foreach (SteamLibraryInfo library in libraries)
        {
            string libraryLeafName = GetLastPathSegment(library.LibraryPath);
            if (string.IsNullOrWhiteSpace(libraryLeafName))
            {
                continue;
            }

            string libraryRelativePrefix = "/" + libraryLeafName;
            if (!restOfPath.StartsWith(libraryRelativePrefix, StringComparison.Ordinal))
            {
                continue;
            }

            string remainderAfterLibrary = restOfPath[libraryRelativePrefix.Length..].TrimStart('/');
            string candidatePath = string.IsNullOrWhiteSpace(remainderAfterLibrary)
                ? library.LibraryPath
                : CombineNormalizedPathSegments(library.LibraryPath, remainderAfterLibrary);

            if (File.Exists(candidatePath))
            {
                _ = matches.Add(candidatePath);
            }
        }

        if (matches.Count == 1)
        {
            return matches.First();
        }

        if (matches.Count > 1)
        {
            diagnostics.Add($"Multiple Steam library paths matched the selected game executable: {string.Join(", ", matches.OrderBy(value => value, StringComparer.Ordinal))}");
        }

        return normalizedGamePath;
    }

    internal static ProtonResolution ResolveProtonPath(
        string steamAppId,
        IEnumerable<string> steamRootCandidates,
        List<string> diagnostics)
    {
        Dictionary<string, HashSet<string>> compatToolMappings = CollectCompatToolMappings(steamRootCandidates, diagnostics);
        List<SteamCompatToolInstall> installedTools = DiscoverCompatTools(
            steamRootCandidates,
            GetSystemCompatToolRoots(),
            diagnostics);

        List<string> exactToolNames = compatToolMappings.TryGetValue(steamAppId, out HashSet<string> exactMappings)
            ? exactMappings.OrderBy(value => value, StringComparer.OrdinalIgnoreCase).ToList()
            : new List<string>();

        if (exactToolNames.Count > 1)
        {
            diagnostics.Add($"Multiple app-specific Proton mappings were found for App ID {steamAppId}: {string.Join(", ", exactToolNames)}");
            return new ProtonResolution(SteamAutoPopulateFieldState.Ambiguous, string.Empty);
        }

        List<string> defaultToolNames = compatToolMappings.TryGetValue("0", out HashSet<string> defaultMappings)
            ? defaultMappings.OrderBy(value => value, StringComparer.OrdinalIgnoreCase).ToList()
            : new List<string>();

        if (exactToolNames.Count == 0 && defaultToolNames.Count > 1)
        {
            diagnostics.Add($"Multiple default Proton mappings were found: {string.Join(", ", defaultToolNames)}");
            return new ProtonResolution(SteamAutoPopulateFieldState.Ambiguous, string.Empty);
        }

        string requestedToolName = exactToolNames.FirstOrDefault() ?? defaultToolNames.FirstOrDefault() ?? string.Empty;
        if (string.IsNullOrWhiteSpace(requestedToolName))
        {
            diagnostics.Add($"No Proton mapping was found for App ID {steamAppId}.");
            return new ProtonResolution(SteamAutoPopulateFieldState.NotFound, string.Empty);
        }

        List<SteamCompatToolInstall> matchingTools = ResolveCompatToolByName(requestedToolName, installedTools);
        if (matchingTools.Count == 1)
        {
            diagnostics.Add($"Resolved Proton tool '{requestedToolName}' to: {matchingTools[0].ProtonPath}");
            return new ProtonResolution(SteamAutoPopulateFieldState.Found, matchingTools[0].ProtonPath);
        }

        if (matchingTools.Count > 1)
        {
            diagnostics.Add($"Proton tool '{requestedToolName}' resolved to multiple installs. Auto-populate will not guess the Proton path.");
            foreach (SteamCompatToolInstall matchingTool in matchingTools)
            {
                diagnostics.Add($"Conflicting Proton install: {matchingTool.ProtonPath}");
            }

            return new ProtonResolution(SteamAutoPopulateFieldState.Ambiguous, string.Empty);
        }

        diagnostics.Add($"CrossHook could not resolve Proton mapping '{requestedToolName}' to an installed Proton executable.");
        return new ProtonResolution(SteamAutoPopulateFieldState.NotFound, string.Empty);
    }

    internal static Dictionary<string, HashSet<string>> CollectCompatToolMappings(
        IEnumerable<string> steamRootCandidates,
        List<string> diagnostics)
    {
        Dictionary<string, HashSet<string>> mappings = new Dictionary<string, HashSet<string>>(StringComparer.OrdinalIgnoreCase);

        foreach (string steamRootCandidate in steamRootCandidates)
        {
            string normalizedSteamRoot = NormalizePathForHostLookup(steamRootCandidate);
            if (string.IsNullOrWhiteSpace(normalizedSteamRoot) || !Directory.Exists(normalizedSteamRoot))
            {
                continue;
            }

            List<string> configPaths = new List<string>
            {
                CombineNormalizedPathSegments(normalizedSteamRoot, "config", "config.vdf")
            };

            foreach (string userDataDirectory in SafeEnumerateDirectories(CombineNormalizedPathSegments(normalizedSteamRoot, "userdata")))
            {
                configPaths.Add(CombineNormalizedPathSegments(userDataDirectory, "config", "localconfig.vdf"));
            }

            foreach (string configPath in configPaths)
            {
                if (!File.Exists(configPath))
                {
                    continue;
                }

                try
                {
                    SteamKeyValueNode configRoot = ParseKeyValueFile(configPath);
                    SteamKeyValueNode compatToolMappingNode = FindDescendantByKey(configRoot, "CompatToolMapping");
                    if (compatToolMappingNode is null)
                    {
                        continue;
                    }

                    foreach (KeyValuePair<string, SteamKeyValueNode> mappingEntry in compatToolMappingNode.Children)
                    {
                        string toolName = mappingEntry.Value.GetChild("name")?.Value ?? string.Empty;
                        if (string.IsNullOrWhiteSpace(toolName))
                        {
                            continue;
                        }

                        if (!mappings.TryGetValue(mappingEntry.Key, out HashSet<string> toolNames))
                        {
                            toolNames = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
                            mappings[mappingEntry.Key] = toolNames;
                        }

                        _ = toolNames.Add(toolName);
                    }
                }
                catch (Exception ex)
                {
                    diagnostics.Add($"Failed to parse Steam config '{configPath}': {ex.Message}");
                }
            }
        }

        return mappings;
    }

    internal static List<SteamCompatToolInstall> DiscoverCompatTools(
        IEnumerable<string> steamRootCandidates,
        IEnumerable<string> systemCompatToolRoots,
        List<string> diagnostics)
    {
        List<SteamCompatToolInstall> tools = new List<SteamCompatToolInstall>();
        HashSet<string> seenProtonPaths = new HashSet<string>(StringComparer.Ordinal);

        foreach (string steamRootCandidate in steamRootCandidates)
        {
            string normalizedSteamRoot = NormalizePathForHostLookup(steamRootCandidate);
            if (string.IsNullOrWhiteSpace(normalizedSteamRoot) || !Directory.Exists(normalizedSteamRoot))
            {
                continue;
            }

            string officialToolsRoot = CombineNormalizedPathSegments(normalizedSteamRoot, "steamapps", "common");
            foreach (string toolDirectoryPath in SafeEnumerateDirectories(officialToolsRoot))
            {
                TryAddCompatToolInstall(tools, seenProtonPaths, toolDirectoryPath, isOfficial: true, diagnostics);
            }

            string customToolsRoot = CombineNormalizedPathSegments(normalizedSteamRoot, "compatibilitytools.d");
            foreach (string toolDirectoryPath in SafeEnumerateDirectories(customToolsRoot))
            {
                TryAddCompatToolInstall(tools, seenProtonPaths, toolDirectoryPath, isOfficial: false, diagnostics);
            }
        }

        foreach (string systemCompatToolRoot in systemCompatToolRoots ?? Array.Empty<string>())
        {
            string normalizedSystemCompatToolRoot = NormalizePathForHostLookup(systemCompatToolRoot);
            if (string.IsNullOrWhiteSpace(normalizedSystemCompatToolRoot) || !Directory.Exists(normalizedSystemCompatToolRoot))
            {
                continue;
            }

            diagnostics.Add($"System Steam compat-tool root: {normalizedSystemCompatToolRoot}");
            foreach (string toolDirectoryPath in SafeEnumerateDirectories(normalizedSystemCompatToolRoot))
            {
                TryAddCompatToolInstall(tools, seenProtonPaths, toolDirectoryPath, isOfficial: false, diagnostics);
            }
        }

        return tools;
    }

    internal static List<SteamCompatToolInstall> ResolveCompatToolByName(
        string requestedToolName,
        IEnumerable<SteamCompatToolInstall> installedTools)
    {
        if (string.IsNullOrWhiteSpace(requestedToolName))
        {
            return new List<SteamCompatToolInstall>();
        }

        string normalizedRequestedToolName = NormalizeCompatToolAlias(requestedToolName);

        List<SteamCompatToolInstall> exactMatches = installedTools
            .Where(tool => tool.Aliases.Contains(requestedToolName, StringComparer.OrdinalIgnoreCase))
            .ToList();

        if (exactMatches.Count > 0)
        {
            return exactMatches;
        }

        List<SteamCompatToolInstall> normalizedMatches = installedTools
            .Where(tool => tool.NormalizedAliases.Contains(normalizedRequestedToolName, StringComparer.Ordinal))
            .ToList();

        if (normalizedMatches.Count > 0)
        {
            return normalizedMatches;
        }

        List<SteamCompatToolInstall> heuristicMatches = installedTools
            .Where(tool => ToolMatchesRequestedNameHeuristically(requestedToolName, tool))
            .ToList();

        return heuristicMatches;
    }

    internal static SteamKeyValueNode ParseKeyValueFile(string path)
    {
        string content = File.ReadAllText(path);
        return ParseKeyValueContent(content);
    }

    internal static SteamKeyValueNode ParseKeyValueContent(string content)
    {
        ArgumentNullException.ThrowIfNull(content);
        int index = 0;
        return ParseKeyValueObject(content, ref index, stopOnClosingBrace: false);
    }

    private static SteamAutoPopulateResult CreateResult(
        SteamAutoPopulateFieldState steamAppIdState,
        string steamAppId,
        SteamAutoPopulateFieldState steamCompatDataState,
        string steamCompatDataPath,
        SteamAutoPopulateFieldState steamProtonState,
        string steamProtonPath,
        List<string> diagnostics,
        List<string> manualHints)
    {
        return new SteamAutoPopulateResult
        {
            SteamAppIdState = steamAppIdState,
            SteamAppId = steamAppId ?? string.Empty,
            SteamCompatDataPathState = steamCompatDataState,
            SteamCompatDataPath = steamCompatDataPath ?? string.Empty,
            SteamProtonPathState = steamProtonState,
            SteamProtonPath = steamProtonPath ?? string.Empty,
            Diagnostics = diagnostics.Distinct(StringComparer.Ordinal).ToList(),
            ManualHints = manualHints.Distinct(StringComparer.Ordinal).ToList()
        };
    }

    private static void AddDefaultManualHints(
        List<string> manualHints,
        IEnumerable<string> steamRootCandidates,
        IEnumerable<string> libraryPaths,
        string steamAppId)
    {
        foreach (string steamRootCandidate in steamRootCandidates.Where(value => !string.IsNullOrWhiteSpace(value)))
        {
            manualHints.Add($"Steam root candidate: {steamRootCandidate}");
        }

        foreach (string libraryPath in libraryPaths.Where(value => !string.IsNullOrWhiteSpace(value)))
        {
            manualHints.Add($"Steam library candidate: {libraryPath}");
            if (!string.IsNullOrWhiteSpace(steamAppId))
            {
                manualHints.Add($"Compatdata pattern: {CombineNormalizedPathSegments(libraryPath, "steamapps", "compatdata", steamAppId)}");
            }
        }

        if (!string.IsNullOrWhiteSpace(steamAppId))
        {
            manualHints.Add($"Steam compatdata folders are usually named after the Steam App ID ({steamAppId}).");
        }
    }

    private static void AddDirectoryCandidate(
        HashSet<string> candidates,
        string pathValue,
        List<string> diagnostics,
        string sourceDescription)
    {
        string normalizedPath = NormalizePathForHostLookup(pathValue);
        if (string.IsNullOrWhiteSpace(normalizedPath))
        {
            return;
        }

        if (Directory.Exists(normalizedPath) && candidates.Add(normalizedPath))
        {
            diagnostics.Add($"{sourceDescription}: {normalizedPath}");
        }
    }

    private static void TryAddLibrary(
        List<SteamLibraryInfo> libraries,
        HashSet<string> seenLibraryPaths,
        string libraryPathValue,
        List<string> diagnostics,
        string sourceDescription)
    {
        string normalizedLibraryPath = NormalizePathForHostLookup(libraryPathValue);
        if (string.IsNullOrWhiteSpace(normalizedLibraryPath) || !Directory.Exists(normalizedLibraryPath))
        {
            return;
        }

        string steamAppsPath = CombineNormalizedPathSegments(normalizedLibraryPath, "steamapps");
        if (!Directory.Exists(steamAppsPath))
        {
            return;
        }

        if (!seenLibraryPaths.Add(normalizedLibraryPath))
        {
            return;
        }

        diagnostics.Add($"{sourceDescription} resolved Steam library: {normalizedLibraryPath}");
        libraries.Add(new SteamLibraryInfo(normalizedLibraryPath, steamAppsPath));
    }

    private static void TryAddCompatToolInstall(
        List<SteamCompatToolInstall> tools,
        HashSet<string> seenProtonPaths,
        string toolDirectoryPath,
        bool isOfficial,
        List<string> diagnostics)
    {
        string protonPath = CombineNormalizedPathSegments(toolDirectoryPath, "proton");
        if (!File.Exists(protonPath) || !seenProtonPaths.Add(protonPath))
        {
            return;
        }

        HashSet<string> aliases = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        _ = aliases.Add(Path.GetFileName(toolDirectoryPath));

        string compatibilityToolDefinitionPath = CombineNormalizedPathSegments(toolDirectoryPath, "compatibilitytool.vdf");
        if (File.Exists(compatibilityToolDefinitionPath))
        {
            try
            {
                SteamKeyValueNode toolDefinitionRoot = ParseKeyValueFile(compatibilityToolDefinitionPath);
                SteamKeyValueNode compatToolsNode = FindDescendantByKey(toolDefinitionRoot, "compat_tools");
                if (compatToolsNode is not null)
                {
                    foreach (KeyValuePair<string, SteamKeyValueNode> compatToolEntry in compatToolsNode.Children)
                    {
                        _ = aliases.Add(compatToolEntry.Key);
                        string displayName = compatToolEntry.Value.GetChild("display_name")?.Value ?? string.Empty;
                        if (!string.IsNullOrWhiteSpace(displayName))
                        {
                            _ = aliases.Add(displayName);
                        }
                    }
                }
            }
            catch (Exception ex)
            {
                diagnostics.Add($"Failed to parse compatibility tool metadata '{compatibilityToolDefinitionPath}': {ex.Message}");
            }
        }

        HashSet<string> normalizedAliases = aliases
            .Select(NormalizeCompatToolAlias)
            .Where(value => !string.IsNullOrWhiteSpace(value))
            .ToHashSet(StringComparer.Ordinal);

        tools.Add(new SteamCompatToolInstall(protonPath, isOfficial, aliases.ToList(), normalizedAliases));
    }

    private static SteamKeyValueNode ParseKeyValueObject(string content, ref int index, bool stopOnClosingBrace)
    {
        SteamKeyValueNode node = new SteamKeyValueNode();

        while (true)
        {
            SkipWhitespaceAndComments(content, ref index);
            if (index >= content.Length)
            {
                return node;
            }

            if (stopOnClosingBrace && content[index] == '}')
            {
                index++;
                return node;
            }

            string key = ReadToken(content, ref index);
            if (string.IsNullOrWhiteSpace(key))
            {
                if (index < content.Length && content[index] == '}')
                {
                    index++;
                    return node;
                }

                continue;
            }

            SkipWhitespaceAndComments(content, ref index);
            if (index < content.Length && content[index] == '{')
            {
                index++;
                node.Children[key] = ParseKeyValueObject(content, ref index, stopOnClosingBrace: true);
                continue;
            }

            string value = ReadToken(content, ref index);
            node.Children[key] = new SteamKeyValueNode(value);
        }
    }

    private static void SkipWhitespaceAndComments(string content, ref int index)
    {
        while (index < content.Length)
        {
            if (char.IsWhiteSpace(content[index]))
            {
                index++;
                continue;
            }

            if (content[index] == '/' && index + 1 < content.Length && content[index + 1] == '/')
            {
                index += 2;
                while (index < content.Length && content[index] != '\n')
                {
                    index++;
                }

                continue;
            }

            break;
        }
    }

    private static string ReadToken(string content, ref int index)
    {
        SkipWhitespaceAndComments(content, ref index);
        if (index >= content.Length)
        {
            return string.Empty;
        }

        if (content[index] == '"' )
        {
            index++;
            StringBuilder builder = new StringBuilder();
            while (index < content.Length)
            {
                char character = content[index++];
                if (character == '"' )
                {
                    break;
                }

                if (character == '\\' && index < content.Length)
                {
                    char escapedCharacter = content[index++];
                    builder.Append(escapedCharacter switch
                    {
                        '\\' => '\\',
                        '"' => '"',
                        'n' => '\n',
                        'r' => '\r',
                        't' => '\t',
                        _ => escapedCharacter
                    });
                    continue;
                }

                builder.Append(character);
            }

            return builder.ToString();
        }

        if (content[index] == '{' || content[index] == '}')
        {
            return string.Empty;
        }

        int startIndex = index;
        while (index < content.Length
            && !char.IsWhiteSpace(content[index])
            && content[index] != '{'
            && content[index] != '}')
        {
            index++;
        }

        return content[startIndex..index];
    }

    private static SteamKeyValueNode FindDescendantByKey(SteamKeyValueNode root, string key)
    {
        foreach (KeyValuePair<string, SteamKeyValueNode> child in root.Children)
        {
            if (string.Equals(child.Key, key, StringComparison.OrdinalIgnoreCase))
            {
                return child.Value;
            }

            SteamKeyValueNode match = FindDescendantByKey(child.Value, key);
            if (match is not null)
            {
                return match;
            }
        }

        return null;
    }

    private static string ExtractAppIdFromManifestPath(string manifestPath)
    {
        string fileNameWithoutExtension = Path.GetFileNameWithoutExtension(manifestPath);
        const string prefix = "appmanifest_";
        return fileNameWithoutExtension.StartsWith(prefix, StringComparison.OrdinalIgnoreCase)
            ? fileNameWithoutExtension[prefix.Length..]
            : string.Empty;
    }

    private static IEnumerable<string> SafeEnumerateFiles(string directoryPath, string searchPattern)
    {
        if (!Directory.Exists(directoryPath))
        {
            return Array.Empty<string>();
        }

        try
        {
            return Directory.EnumerateFiles(directoryPath, searchPattern, SearchOption.TopDirectoryOnly).ToArray();
        }
        catch
        {
            return Array.Empty<string>();
        }
    }

    private static IEnumerable<string> SafeEnumerateDirectories(string directoryPath)
    {
        if (!Directory.Exists(directoryPath))
        {
            return Array.Empty<string>();
        }

        try
        {
            return Directory.EnumerateDirectories(directoryPath, "*", SearchOption.TopDirectoryOnly).ToArray();
        }
        catch
        {
            return Array.Empty<string>();
        }
    }

    private static string NormalizePathForHostLookup(string pathValue)
    {
        if (string.IsNullOrWhiteSpace(pathValue))
        {
            return string.Empty;
        }

        string trimmedPath = pathValue.Trim();

        try
        {
            if (trimmedPath.StartsWith("/", StringComparison.Ordinal))
            {
                return NormalizeHostStylePath(trimmedPath);
            }

            if (SteamLaunchService.LooksLikeWindowsPath(trimmedPath))
            {
                return ResolveRemainingDosDevicesPath(NormalizeHostStylePath(SteamLaunchService.NormalizeSteamHostPath(trimmedPath)));
            }

            return ResolveRemainingDosDevicesPath(NormalizeHostStylePath(trimmedPath));
        }
        catch
        {
            return ResolveRemainingDosDevicesPath(NormalizeHostStylePath(trimmedPath));
        }
    }

    private static string CombineNormalizedPathSegments(string rootPath, params string[] segments)
    {
        string normalizedRootPath = NormalizeHostStylePath(rootPath).TrimEnd('/');
        if (string.IsNullOrWhiteSpace(normalizedRootPath))
        {
            return string.Empty;
        }

        StringBuilder builder = new StringBuilder(normalizedRootPath);
        foreach (string segment in segments)
        {
            string normalizedSegment = NormalizeHostStylePath(segment).Trim('/');
            if (string.IsNullOrWhiteSpace(normalizedSegment))
            {
                continue;
            }

            if (builder.Length == 0 || builder[^1] != '/')
            {
                builder.Append('/');
            }

            builder.Append(normalizedSegment);
        }

        return NormalizeHostStylePath(builder.ToString());
    }

    private static bool PathIsSameOrChild(string pathValue, string rootPath)
    {
        string normalizedPath = NormalizeHostStylePath(pathValue).TrimEnd('/');
        string normalizedRoot = NormalizeHostStylePath(rootPath).TrimEnd('/');

        if (string.Equals(normalizedPath, normalizedRoot, StringComparison.Ordinal))
        {
            return true;
        }

        return normalizedPath.StartsWith(normalizedRoot + "/", StringComparison.Ordinal);
    }

    private static string NormalizeCompatToolAlias(string alias)
    {
        if (string.IsNullOrWhiteSpace(alias))
        {
            return string.Empty;
        }

        StringBuilder builder = new StringBuilder(alias.Length);
        foreach (char character in alias)
        {
            if (char.IsLetterOrDigit(character))
            {
                builder.Append(char.ToLowerInvariant(character));
            }
        }

        return builder.ToString();
    }

    private static bool ToolMatchesRequestedNameHeuristically(string requestedToolName, SteamCompatToolInstall installedTool)
    {
        string normalizedRequestedToolName = NormalizeCompatToolAlias(requestedToolName);
        if (string.IsNullOrWhiteSpace(normalizedRequestedToolName))
        {
            return false;
        }

        foreach (string normalizedAlias in installedTool.NormalizedAliases)
        {
            if (normalizedAlias.Contains(normalizedRequestedToolName, StringComparison.Ordinal)
                || normalizedRequestedToolName.Contains(normalizedAlias, StringComparison.Ordinal))
            {
                return true;
            }
        }

        if (normalizedRequestedToolName.StartsWith("proton", StringComparison.Ordinal)
            && int.TryParse(new string(normalizedRequestedToolName.Where(char.IsDigit).ToArray()), out int requestedVersion))
        {
            return installedTool.NormalizedAliases.Any(alias =>
                alias.StartsWith("proton", StringComparison.Ordinal)
                && alias.Contains(requestedVersion.ToString(), StringComparison.Ordinal));
        }

        return false;
    }

    private static string NormalizeHostStylePath(string pathValue)
    {
        string normalizedPath = (pathValue ?? string.Empty).Trim().Replace('\\', '/');
        if (string.IsNullOrWhiteSpace(normalizedPath))
        {
            return string.Empty;
        }

        if (!normalizedPath.StartsWith("/", StringComparison.Ordinal))
        {
            return CollapseRepeatedSeparators(normalizedPath);
        }

        List<string> segments = new List<string>();
        foreach (string segment in normalizedPath.Split('/', StringSplitOptions.RemoveEmptyEntries))
        {
            if (segment == ".")
            {
                continue;
            }

            if (segment == "..")
            {
                if (segments.Count > 0)
                {
                    segments.RemoveAt(segments.Count - 1);
                }

                continue;
            }

            segments.Add(segment);
        }

        return "/" + string.Join("/", segments);
    }

    private static string CollapseRepeatedSeparators(string pathValue)
    {
        StringBuilder builder = new StringBuilder(pathValue.Length);
        bool lastWasSlash = false;

        foreach (char character in pathValue)
        {
            if (character == '/')
            {
                if (lastWasSlash)
                {
                    continue;
                }

                lastWasSlash = true;
                builder.Append(character);
                continue;
            }

            lastWasSlash = false;
            builder.Append(character);
        }

        return builder.ToString();
    }

    private static IReadOnlyList<string> GetSystemCompatToolRoots()
    {
        return
        [
            "/usr/share/steam/compatibilitytools.d",
            "/usr/share/steam/compatibilitytools",
            "/usr/local/share/steam/compatibilitytools.d",
            "/usr/local/share/steam/compatibilitytools"
        ];
    }

    private static string ExtractDosDevicesRestOfPath(string normalizedPath)
    {
        const string marker = "/dosdevices/";
        int markerIndex = normalizedPath.IndexOf(marker, StringComparison.Ordinal);
        if (markerIndex < 0)
        {
            return string.Empty;
        }

        string remainder = normalizedPath[(markerIndex + marker.Length)..];
        int separatorIndex = remainder.IndexOf('/');
        if (separatorIndex < 0)
        {
            return string.Empty;
        }

        string driveSegment = remainder[..separatorIndex];
        if (driveSegment.Length != 2 || driveSegment[1] != ':' || !char.IsLetter(driveSegment[0]))
        {
            return string.Empty;
        }

        return remainder[separatorIndex..];
    }

    private static string GetLastPathSegment(string pathValue)
    {
        string normalizedPath = NormalizeHostStylePath(pathValue).TrimEnd('/');
        if (string.IsNullOrWhiteSpace(normalizedPath))
        {
            return string.Empty;
        }

        int separatorIndex = normalizedPath.LastIndexOf('/');
        return separatorIndex >= 0
            ? normalizedPath[(separatorIndex + 1)..]
            : normalizedPath;
    }

    private static string ResolveRemainingDosDevicesPath(string normalizedPath)
    {
        if (string.IsNullOrWhiteSpace(normalizedPath))
        {
            return string.Empty;
        }

        string restOfPath = ExtractDosDevicesRestOfPath(normalizedPath);
        if (string.IsNullOrWhiteSpace(restOfPath))
        {
            return normalizedPath;
        }

        string scannedHostPath = SteamLaunchService.ResolveMountedHostPathByScanning(
            restOfPath,
            SteamLaunchService.GetMountedHostSearchRoots());

        return string.IsNullOrWhiteSpace(scannedHostPath)
            ? normalizedPath
            : NormalizeHostStylePath(scannedHostPath);
    }
}

internal sealed class SteamLibraryInfo
{
    public SteamLibraryInfo(string libraryPath, string steamAppsPath)
    {
        LibraryPath = libraryPath;
        SteamAppsPath = steamAppsPath;
    }

    public string LibraryPath { get; }

    public string SteamAppsPath { get; }
}

internal sealed class SteamGameMatch
{
    public SteamGameMatch(string steamAppId, string libraryPath, string installDirectoryPath, string manifestPath)
    {
        SteamAppId = steamAppId;
        LibraryPath = libraryPath;
        InstallDirectoryPath = installDirectoryPath;
        ManifestPath = manifestPath;
    }

    public string SteamAppId { get; }

    public string LibraryPath { get; }

    public string InstallDirectoryPath { get; }

    public string ManifestPath { get; }
}

internal sealed class SteamGameMatchSelection
{
    public SteamGameMatchSelection(SteamAutoPopulateFieldState state, SteamGameMatch match)
    {
        State = state;
        Match = match;
    }

    public SteamAutoPopulateFieldState State { get; }

    public SteamGameMatch Match { get; }
}

internal sealed class SteamCompatToolInstall
{
    public SteamCompatToolInstall(
        string protonPath,
        bool isOfficial,
        IReadOnlyList<string> aliases,
        IReadOnlySet<string> normalizedAliases)
    {
        ProtonPath = protonPath;
        IsOfficial = isOfficial;
        Aliases = aliases;
        NormalizedAliases = normalizedAliases;
    }

    public string ProtonPath { get; }

    public bool IsOfficial { get; }

    public IReadOnlyList<string> Aliases { get; }

    public IReadOnlySet<string> NormalizedAliases { get; }
}

internal sealed class ProtonResolution
{
    public ProtonResolution(SteamAutoPopulateFieldState state, string protonPath)
    {
        State = state;
        ProtonPath = protonPath;
    }

    public SteamAutoPopulateFieldState State { get; }

    public string ProtonPath { get; }
}

internal sealed class SteamKeyValueNode
{
    public SteamKeyValueNode()
    {
    }

    public SteamKeyValueNode(string value)
    {
        Value = value ?? string.Empty;
    }

    public string Value { get; } = string.Empty;

    public Dictionary<string, SteamKeyValueNode> Children { get; } = new Dictionary<string, SteamKeyValueNode>(StringComparer.OrdinalIgnoreCase);

    public SteamKeyValueNode GetChild(string key)
    {
        if (string.IsNullOrWhiteSpace(key))
        {
            return null;
        }

        return Children.TryGetValue(key, out SteamKeyValueNode child) ? child : null;
    }
}
