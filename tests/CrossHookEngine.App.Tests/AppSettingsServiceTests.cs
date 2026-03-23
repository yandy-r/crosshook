using CrossHookEngine.App.Services;

namespace CrossHookEngine.App.Tests;

public sealed class AppSettingsServiceTests
{
    [Fact]
    public void Constructor_ThrowsForNullStartupPath()
    {
        ArgumentNullException exception = Assert.Throws<ArgumentNullException>(() => new AppSettingsService(null));

        Assert.Equal("startupPath", exception.ParamName);
    }

    [Fact]
    public void SaveAppSettings_WritesExpectedSettingsFormat()
    {
        using TestWorkspace workspace = new TestWorkspace();
        AppSettingsService service = new AppSettingsService(workspace.RootPath);

        service.SaveAppSettings(
            new AppSettingsData
            {
                AutoLoadLastProfile = true,
                LastUsedProfile = "steam-deck=profile"
            });

        string settingsPath = workspace.GetPath("Settings", "AppSettings.ini");

        Assert.Equal(
            new[]
            {
                "AutoLoadLastProfile=True",
                "LastUsedProfile=steam-deck=profile"
            },
            File.ReadAllLines(settingsPath));
    }

    [Fact]
    public void LoadAppSettings_ReturnsDefaultsWhenSettingsFileIsMissing()
    {
        using TestWorkspace workspace = new TestWorkspace();
        AppSettingsService service = new AppSettingsService(workspace.RootPath);

        AppSettingsData settings = service.LoadAppSettings();

        Assert.False(settings.AutoLoadLastProfile);
        Assert.Equal(string.Empty, settings.LastUsedProfile);
        Assert.True(Directory.Exists(workspace.GetPath("Settings")));
    }

    [Fact]
    public void LoadAppSettings_ParsesStoredValuesAndSkipsMalformedLines()
    {
        using TestWorkspace workspace = new TestWorkspace();
        AppSettingsService service = new AppSettingsService(workspace.RootPath);
        string settingsPath = workspace.GetPath("Settings", "AppSettings.ini");

        Directory.CreateDirectory(Path.GetDirectoryName(settingsPath)!);
        File.WriteAllLines(
            settingsPath,
            new[]
            {
                "MalformedLine",
                "AutoLoadLastProfile=true",
                "LastUsedProfile=steam=deck=profile"
            });

        AppSettingsData settings = service.LoadAppSettings();

        Assert.True(settings.AutoLoadLastProfile);
        Assert.Equal("steam=deck=profile", settings.LastUsedProfile);
    }

	[Fact]
	public void LoadAppSettings_IgnoresInvalidBooleanValues_InsteadOfThrowing()
	{
		using TestWorkspace workspace = new TestWorkspace();
		AppSettingsService service = new AppSettingsService(workspace.RootPath);
		string settingsPath = workspace.GetPath("Settings", "AppSettings.ini");

		Directory.CreateDirectory(Path.GetDirectoryName(settingsPath)!);
		File.WriteAllLines(
			settingsPath,
			new[]
			{
				"AutoLoadLastProfile=yes",
				"LastUsedProfile=deck-profile"
			});

		AppSettingsData settings = service.LoadAppSettings();

        Assert.False(settings.AutoLoadLastProfile);
        Assert.Equal("deck-profile", settings.LastUsedProfile);
    }

    [Fact]
    public void SaveAppSettings_ThenLoadAppSettings_RoundTripsPersistedValues()
    {
        using TestWorkspace workspace = new TestWorkspace();
        AppSettingsService service = new AppSettingsService(workspace.RootPath);
        AppSettingsData expectedSettings = new AppSettingsData
        {
            AutoLoadLastProfile = true,
            LastUsedProfile = "deck-profile"
        };

        service.SaveAppSettings(expectedSettings);

        AppSettingsData actualSettings = service.LoadAppSettings();

        Assert.Equal(expectedSettings.AutoLoadLastProfile, actualSettings.AutoLoadLastProfile);
        Assert.Equal(expectedSettings.LastUsedProfile, actualSettings.LastUsedProfile);
    }
}
