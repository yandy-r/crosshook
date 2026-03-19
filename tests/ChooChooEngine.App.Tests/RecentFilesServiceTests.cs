using ChooChooEngine.App.Services;

namespace ChooChooEngine.App.Tests;

public sealed class RecentFilesServiceTests
{
    [Fact]
    public void SaveRecentFiles_WritesSectionedSettingsFormat()
    {
        using TestWorkspace workspace = new TestWorkspace();
        Directory.CreateDirectory(workspace.RootPath);
        RecentFilesService service = new RecentFilesService(workspace.RootPath);

        RecentFilesData recentFiles = new RecentFilesData(
            new[] { "/games/hades.exe" },
            new[] { "/trainers/hades.exe" },
            new[] { "/mods/first.dll", "/mods/second.dll" });

        service.SaveRecentFiles(recentFiles);

        string settingsPath = workspace.GetPath("settings.ini");

        Assert.Equal(
            new[]
            {
                "[RecentGamePaths]",
                "/games/hades.exe",
                string.Empty,
                "[RecentTrainerPaths]",
                "/trainers/hades.exe",
                string.Empty,
                "[RecentDllPaths]",
                "/mods/first.dll",
                "/mods/second.dll"
            },
            File.ReadAllLines(settingsPath));
    }

    [Fact]
    public void LoadRecentFiles_OnlyReturnsExistingPathsInKnownSections()
    {
        using TestWorkspace workspace = new TestWorkspace();
        RecentFilesService service = new RecentFilesService(workspace.RootPath);

        string gamePath = workspace.CreateFile("games", "hades.exe");
        string trainerPath = workspace.CreateFile("trainers", "hades-trainer.exe");
        string dllPath = workspace.CreateFile("mods", "first.dll");
        string ignoredPath = workspace.CreateFile("other", "ignored.dll");
        string missingDllPath = workspace.GetPath("mods", "missing.dll");

        File.WriteAllLines(
            workspace.GetPath("settings.ini"),
            new[]
            {
                "; comment",
                string.Empty,
                "[RecentGamePaths]",
                gamePath,
                workspace.GetPath("games", "missing.exe"),
                string.Empty,
                "[RecentTrainerPaths]",
                trainerPath,
                string.Empty,
                "[OtherSection]",
                ignoredPath,
                string.Empty,
                "[RecentDllPaths]",
                dllPath,
                missingDllPath
            });

        RecentFilesData recentFiles = service.LoadRecentFiles();

        Assert.Equal(new[] { gamePath }, recentFiles.GamePaths);
        Assert.Equal(new[] { trainerPath }, recentFiles.TrainerPaths);
        Assert.Equal(new[] { dllPath }, recentFiles.DllPaths);
    }
}
