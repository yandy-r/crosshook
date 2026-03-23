using CrossHookEngine.App.Services;

namespace CrossHookEngine.App.Tests;

public sealed class SteamAutoPopulateServiceTests
{
    [Fact]
    public void AttemptAutoPopulate_FindsAppIdCompatdataAndProton_FromGameExecutable()
    {
        using TestWorkspace workspace = new TestWorkspace();
        string steamRootPath = workspace.GetPath("home", "deck", ".steam", "root");
        string steamLibraryPath = workspace.GetPath("mnt", "sdx1", "SteamLibrary");
        string gamePath = workspace.GetPath("mnt", "sdx1", "SteamLibrary", "steamapps", "common", "MGS_TPP", "mgsvtpp.exe");
        string compatDataPath = workspace.GetPath("mnt", "sdx1", "SteamLibrary", "steamapps", "compatdata", "287700");
        string protonPath = workspace.GetPath("home", "deck", ".steam", "root", "steamapps", "common", "Proton 9.0", "proton");

        WriteFile(
            workspace.GetPath("home", "deck", ".steam", "root", "steamapps", "libraryfolders.vdf"),
            $$"""
            "libraryfolders"
            {
                "0"
                {
                    "path" "{{steamLibraryPath.Replace("\\", "/")}}"
                }
            }
            """);

        WriteFile(
            workspace.GetPath("mnt", "sdx1", "SteamLibrary", "steamapps", "appmanifest_287700.acf"),
            """
            "AppState"
            {
                "appid" "287700"
                "installdir" "MGS_TPP"
            }
            """);

        WriteFile(gamePath, "game");
        Directory.CreateDirectory(Path.Combine(compatDataPath, "pfx"));
        WriteFile(protonPath, "#!/bin/bash");
        WriteFile(
            workspace.GetPath("home", "deck", ".steam", "root", "steamapps", "common", "Proton 9.0", "compatibilitytool.vdf"),
            """
            "compatibilitytools"
            {
                "compat_tools"
                {
                    "proton_9"
                    {
                        "display_name" "Proton 9.0"
                    }
                }
            }
            """);

        WriteFile(
            workspace.GetPath("home", "deck", ".steam", "root", "userdata", "100", "config", "localconfig.vdf"),
            """
            "UserLocalConfigStore"
            {
                "Software"
                {
                    "Valve"
                    {
                        "Steam"
                        {
                            "CompatToolMapping"
                            {
                                "287700"
                                {
                                    "name" "proton_9"
                                }
                            }
                        }
                    }
                }
            }
            """);

        SteamAutoPopulateResult result = SteamAutoPopulateService.AttemptAutoPopulate(new SteamAutoPopulateRequest
        {
            GamePath = gamePath,
            SteamClientInstallPath = steamRootPath
        });

        Assert.True(result.HasAnyMatch);
        Assert.Equal(SteamAutoPopulateFieldState.Found, result.SteamAppIdState);
        Assert.Equal("287700", result.SteamAppId);
        Assert.Equal(SteamAutoPopulateFieldState.Found, result.SteamCompatDataPathState);
        Assert.Equal(compatDataPath.Replace('\\', '/'), result.SteamCompatDataPath);
        Assert.Equal(SteamAutoPopulateFieldState.Found, result.SteamProtonPathState);
        Assert.Equal(protonPath.Replace('\\', '/'), result.SteamProtonPath);
        Assert.Contains(
            result.Diagnostics,
            line => line.Contains("appmanifest_287700.acf", StringComparison.Ordinal)
                && !line.Contains("\\", StringComparison.Ordinal));
    }

    [Fact]
    public void AttemptAutoPopulate_UsesDefaultProtonMapping_WhenAppSpecificMappingIsMissing()
    {
        using TestWorkspace workspace = new TestWorkspace();
        string steamRootPath = workspace.GetPath("home", "deck", ".steam", "root");
        string steamLibraryPath = workspace.GetPath("home", "deck", ".local", "share", "Steam");
        string gamePath = workspace.GetPath("home", "deck", ".local", "share", "Steam", "steamapps", "common", "Balatro", "Balatro.exe");
        string protonPath = workspace.GetPath("home", "deck", ".steam", "root", "steamapps", "common", "Proton 8.0", "proton");

        WriteFile(
            workspace.GetPath("home", "deck", ".steam", "root", "steamapps", "libraryfolders.vdf"),
            $$"""
            "libraryfolders"
            {
                "0" "{{steamLibraryPath.Replace("\\", "/")}}"
            }
            """);

        WriteFile(
            workspace.GetPath("home", "deck", ".local", "share", "Steam", "steamapps", "appmanifest_2379780.acf"),
            """
            "AppState"
            {
                "appid" "2379780"
                "installdir" "Balatro"
            }
            """);

        WriteFile(gamePath, "game");
        WriteFile(protonPath, "#!/bin/bash");
        WriteFile(
            workspace.GetPath("home", "deck", ".steam", "root", "steamapps", "common", "Proton 8.0", "compatibilitytool.vdf"),
            """
            "compatibilitytools"
            {
                "compat_tools"
                {
                    "proton_8"
                    {
                        "display_name" "Proton 8.0"
                    }
                }
            }
            """);

        WriteFile(
            workspace.GetPath("home", "deck", ".steam", "root", "userdata", "100", "config", "localconfig.vdf"),
            """
            "UserLocalConfigStore"
            {
                "Software"
                {
                    "Valve"
                    {
                        "Steam"
                        {
                            "CompatToolMapping"
                            {
                                "0"
                                {
                                    "name" "proton_8"
                                }
                            }
                        }
                    }
                }
            }
            """);

        SteamAutoPopulateResult result = SteamAutoPopulateService.AttemptAutoPopulate(new SteamAutoPopulateRequest
        {
            GamePath = gamePath,
            SteamClientInstallPath = steamRootPath
        });

        Assert.Equal(SteamAutoPopulateFieldState.Found, result.SteamAppIdState);
        Assert.Equal("2379780", result.SteamAppId);
        Assert.Equal(SteamAutoPopulateFieldState.NotFound, result.SteamCompatDataPathState);
        Assert.Equal(SteamAutoPopulateFieldState.Found, result.SteamProtonPathState);
        Assert.Equal(protonPath.Replace('\\', '/'), result.SteamProtonPath);
    }

    [Fact]
    public void AttemptAutoPopulate_LeavesProtonBlank_WhenMappingsConflict()
    {
        using TestWorkspace workspace = new TestWorkspace();
        string steamRootPath = workspace.GetPath("home", "deck", ".steam", "root");
        string steamLibraryPath = workspace.GetPath("mnt", "sdx1", "SteamLibrary");
        string gamePath = workspace.GetPath("mnt", "sdx1", "SteamLibrary", "steamapps", "common", "MGS_TPP", "mgsvtpp.exe");
        string compatDataPath = workspace.GetPath("mnt", "sdx1", "SteamLibrary", "steamapps", "compatdata", "287700");

        WriteFile(
            workspace.GetPath("home", "deck", ".steam", "root", "steamapps", "libraryfolders.vdf"),
            $$"""
            "libraryfolders"
            {
                "0"
                {
                    "path" "{{steamLibraryPath.Replace("\\", "/")}}"
                }
            }
            """);

        WriteFile(
            workspace.GetPath("mnt", "sdx1", "SteamLibrary", "steamapps", "appmanifest_287700.acf"),
            """
            "AppState"
            {
                "appid" "287700"
                "installdir" "MGS_TPP"
            }
            """);

        WriteFile(gamePath, "game");
        Directory.CreateDirectory(Path.Combine(compatDataPath, "pfx"));
        WriteFile(workspace.GetPath("home", "deck", ".steam", "root", "steamapps", "common", "Proton 9.0", "proton"), "#!/bin/bash");
        WriteFile(workspace.GetPath("home", "deck", ".steam", "root", "compatibilitytools.d", "GE-Proton9-2", "proton"), "#!/bin/bash");

        WriteFile(
            workspace.GetPath("home", "deck", ".steam", "root", "userdata", "100", "config", "localconfig.vdf"),
            """
            "UserLocalConfigStore"
            {
                "Software"
                {
                    "Valve"
                    {
                        "Steam"
                        {
                            "CompatToolMapping"
                            {
                                "287700"
                                {
                                    "name" "Proton 9.0"
                                }
                            }
                        }
                    }
                }
            }
            """);

        WriteFile(
            workspace.GetPath("home", "deck", ".steam", "root", "userdata", "200", "config", "localconfig.vdf"),
            """
            "UserLocalConfigStore"
            {
                "Software"
                {
                    "Valve"
                    {
                        "Steam"
                        {
                            "CompatToolMapping"
                            {
                                "287700"
                                {
                                    "name" "GE-Proton9-2"
                                }
                            }
                        }
                    }
                }
            }
            """);

        SteamAutoPopulateResult result = SteamAutoPopulateService.AttemptAutoPopulate(new SteamAutoPopulateRequest
        {
            GamePath = gamePath,
            SteamClientInstallPath = steamRootPath
        });

        Assert.Equal(SteamAutoPopulateFieldState.Found, result.SteamAppIdState);
        Assert.Equal(SteamAutoPopulateFieldState.Found, result.SteamCompatDataPathState);
        Assert.Equal(SteamAutoPopulateFieldState.Ambiguous, result.SteamProtonPathState);
        Assert.Equal(string.Empty, result.SteamProtonPath);
        Assert.Contains(result.Diagnostics, line => line.Contains("Multiple app-specific Proton mappings", StringComparison.Ordinal));
    }

    [Fact]
    public void AttemptAutoPopulate_ReturnsNotFound_WhenGameDoesNotMatchAnySteamManifest()
    {
        using TestWorkspace workspace = new TestWorkspace();
        string steamRootPath = workspace.GetPath("home", "deck", ".steam", "root");
        string steamLibraryPath = workspace.GetPath("mnt", "sdx1", "SteamLibrary");
        string gamePath = workspace.GetPath("games", "Standalone", "othergame.exe");

        WriteFile(
            workspace.GetPath("home", "deck", ".steam", "root", "steamapps", "libraryfolders.vdf"),
            $$"""
            "libraryfolders"
            {
                "0"
                {
                    "path" "{{steamLibraryPath.Replace("\\", "/")}}"
                }
            }
            """);

        WriteFile(
            workspace.GetPath("mnt", "sdx1", "SteamLibrary", "steamapps", "appmanifest_287700.acf"),
            """
            "AppState"
            {
                "appid" "287700"
                "installdir" "MGS_TPP"
            }
            """);

        WriteFile(gamePath, "game");

        SteamAutoPopulateResult result = SteamAutoPopulateService.AttemptAutoPopulate(new SteamAutoPopulateRequest
        {
            GamePath = gamePath,
            SteamClientInstallPath = steamRootPath
        });

        Assert.False(result.HasAnyMatch);
        Assert.Equal(SteamAutoPopulateFieldState.NotFound, result.SteamAppIdState);
        Assert.Equal(SteamAutoPopulateFieldState.NotFound, result.SteamCompatDataPathState);
        Assert.Equal(SteamAutoPopulateFieldState.NotFound, result.SteamProtonPathState);
        Assert.Contains(result.Diagnostics, line => line.Contains("No Steam app manifest matched", StringComparison.Ordinal));
    }

    [Fact]
    public void DiscoverCompatTools_IncludesSystemCompatToolRoots()
    {
        using TestWorkspace workspace = new TestWorkspace();
        string systemCompatToolRoot = workspace.GetPath("usr", "share", "steam", "compatibilitytools.d");
        string protonPath = workspace.GetPath("usr", "share", "steam", "compatibilitytools.d", "proton-cachyos-slr", "proton");
        List<string> diagnostics = new List<string>();

        WriteFile(protonPath, "#!/bin/bash");

        List<SteamCompatToolInstall> tools = SteamAutoPopulateService.DiscoverCompatTools(
            Array.Empty<string>(),
            new[] { systemCompatToolRoot.Replace('\\', '/') },
            diagnostics);

        SteamCompatToolInstall tool = Assert.Single(tools);
        Assert.Equal(protonPath.Replace('\\', '/'), tool.ProtonPath);

        List<SteamCompatToolInstall> resolvedTools = SteamAutoPopulateService.ResolveCompatToolByName("proton-cachyos-slr", tools);
        Assert.Single(resolvedTools);
        Assert.Contains(diagnostics, line => line.Contains("System Steam compat-tool root", StringComparison.Ordinal));
    }

    [Fact]
    public void AttemptAutoPopulate_ResolvesDosDevicesGamePath_ByScanningMountedHostRoots()
    {
        using TestWorkspace workspace = new TestWorkspace();
        string steamRootPath = workspace.GetPath("home", "deck", ".local", "share", "Steam");
        string steamLibraryPath = workspace.GetPath("mnt", "sdb", "SteamLibrary");
        string compatDataPath = workspace.GetPath("mnt", "sdb", "SteamLibrary", "steamapps", "compatdata", "2231380");
        string unresolvedGamePath = workspace.GetPath(
            "home",
            "deck",
            ".local",
            "share",
            "Steam",
            "steamapps",
            "compatdata",
            "3109640506",
            "pfx",
            "dosdevices",
            "d:",
            "SteamLibrary",
            "steamapps",
            "common",
            "Ghost Recon Breakpoint",
            "GRB_vulkan.exe").Replace('\\', '/');
        string resolvedGamePath = workspace.GetPath("mnt", "sdb", "SteamLibrary", "steamapps", "common", "Ghost Recon Breakpoint", "GRB_vulkan.exe");

        WriteFile(
            workspace.GetPath("home", "deck", ".local", "share", "Steam", "steamapps", "libraryfolders.vdf"),
            $$"""
            "libraryfolders"
            {
                "2"
                {
                    "path" "{{steamLibraryPath.Replace("\\", "/")}}"
                }
            }
            """);

        WriteFile(
            workspace.GetPath("mnt", "sdb", "SteamLibrary", "steamapps", "appmanifest_2231380.acf"),
            """
            "AppState"
            {
                "appid" "2231380"
                "installdir" "Ghost Recon Breakpoint"
            }
            """);

        WriteFile(resolvedGamePath, "game");
        Directory.CreateDirectory(Path.Combine(compatDataPath, "pfx"));

        SteamAutoPopulateResult result = SteamAutoPopulateService.AttemptAutoPopulate(new SteamAutoPopulateRequest
        {
            GamePath = unresolvedGamePath,
            SteamClientInstallPath = steamRootPath
        });

        Assert.True(
            result.SteamAppIdState == SteamAutoPopulateFieldState.Found,
            string.Join(Environment.NewLine, result.Diagnostics));
        Assert.Equal("2231380", result.SteamAppId);
        Assert.Equal(SteamAutoPopulateFieldState.Found, result.SteamCompatDataPathState);
        Assert.Equal(compatDataPath.Replace('\\', '/'), result.SteamCompatDataPath);
        Assert.Contains(result.Diagnostics, line => line.Contains(resolvedGamePath.Replace('\\', '/'), StringComparison.Ordinal));
    }

    private static void WriteFile(string path, string content)
    {
        string directoryPath = Path.GetDirectoryName(path) ?? throw new InvalidOperationException("File path must have a directory.");
        Directory.CreateDirectory(directoryPath);
        File.WriteAllText(path, content);
    }
}
