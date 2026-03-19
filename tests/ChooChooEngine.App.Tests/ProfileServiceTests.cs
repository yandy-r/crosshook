using ChooChooEngine.App.Services;

namespace ChooChooEngine.App.Tests;

public sealed class ProfileServiceTests
{
    [Fact]
    public void Constructor_ThrowsForNullStartupPath()
    {
        ArgumentNullException exception = Assert.Throws<ArgumentNullException>(() => new ProfileService(null));

        Assert.Equal("startupPath", exception.ParamName);
    }

    [Fact]
    public void SaveProfile_WritesExpectedProfileFormat()
    {
        using TestWorkspace workspace = new TestWorkspace();
        ProfileService service = new ProfileService(workspace.RootPath);

        ProfileData profile = new ProfileData
        {
            GamePath = "/games/hades.exe",
            TrainerPath = "/trainers/hades=godmode.exe",
            Dll1Path = "/mods/first.dll",
            Dll2Path = "/mods/second.dll",
            LaunchInject1 = true,
            LaunchInject2 = false,
            LaunchMethod = "CmdStart"
        };

        service.SaveProfile("deck-run", profile);

        string profilePath = workspace.GetPath("Profiles", "deck-run.profile");

        Assert.Equal(
            new[]
            {
                "GamePath=/games/hades.exe",
                "TrainerPath=/trainers/hades=godmode.exe",
                "Dll1Path=/mods/first.dll",
                "Dll2Path=/mods/second.dll",
                "LaunchInject1=True",
                "LaunchInject2=False",
                "LaunchMethod=CmdStart"
            },
            File.ReadAllLines(profilePath));
    }

    [Fact]
    public void LoadProfile_ParsesStoredValuesAndSkipsMalformedLines()
    {
        using TestWorkspace workspace = new TestWorkspace();
        ProfileService service = new ProfileService(workspace.RootPath);
        string profilePath = workspace.GetPath("Profiles", "deck-run.profile");

        Directory.CreateDirectory(Path.GetDirectoryName(profilePath)!);
        File.WriteAllLines(
            profilePath,
            new[]
            {
                "GamePath=/games/hades.exe",
                "MalformedLine",
                "TrainerPath=/trainers/hades=godmode.exe",
                "Dll1Path=/mods/first.dll",
                "Dll2Path=/mods/second.dll",
                "LaunchInject1=true",
                "LaunchInject2=false",
                "LaunchMethod=ProcessStart",
                "UnknownKey=ignored"
            });

        ProfileData profile = service.LoadProfile("deck-run");

        Assert.Equal("/games/hades.exe", profile.GamePath);
        Assert.Equal("/trainers/hades=godmode.exe", profile.TrainerPath);
        Assert.Equal("/mods/first.dll", profile.Dll1Path);
        Assert.Equal("/mods/second.dll", profile.Dll2Path);
        Assert.True(profile.LaunchInject1);
        Assert.False(profile.LaunchInject2);
        Assert.Equal("ProcessStart", profile.LaunchMethod);
    }

    [Fact]
    public void LoadProfile_ThrowsWhenProfileFileIsMissing()
    {
        using TestWorkspace workspace = new TestWorkspace();
        ProfileService service = new ProfileService(workspace.RootPath);

        FileNotFoundException exception = Assert.Throws<FileNotFoundException>(() => service.LoadProfile("missing-profile"));

        Assert.EndsWith(Path.Combine("Profiles", "missing-profile.profile"), exception.FileName);
    }

    [Fact]
    public void LoadProfile_IgnoresInvalidBooleanValues_InsteadOfThrowing()
    {
        using TestWorkspace workspace = new TestWorkspace();
        ProfileService service = new ProfileService(workspace.RootPath);
        string profilePath = workspace.GetPath("Profiles", "deck-run.profile");

        Directory.CreateDirectory(Path.GetDirectoryName(profilePath)!);
        File.WriteAllLines(
            profilePath,
            new[]
            {
                "GamePath=/games/hades.exe",
                "LaunchInject1=1",
                "LaunchInject2=",
                "LaunchMethod=CreateProcess"
            });

        ProfileData profile = service.LoadProfile("deck-run");

        Assert.Equal("/games/hades.exe", profile.GamePath);
        Assert.False(profile.LaunchInject1);
        Assert.False(profile.LaunchInject2);
        Assert.Equal("CreateProcess", profile.LaunchMethod);
    }

    [Fact]
    public void GetProfileNames_ReturnsEmptyAndCreatesProfilesDirectoryWhenMissing()
    {
        using TestWorkspace workspace = new TestWorkspace();
        ProfileService service = new ProfileService(workspace.RootPath);
        string profilesDirectoryPath = workspace.GetPath("Profiles");

        List<string> profileNames = service.GetProfileNames();

        Assert.Empty(profileNames);
        Assert.True(Directory.Exists(profilesDirectoryPath));
    }

    [Fact]
    public void GetProfileNames_ReturnsOnlyProfileBasenames()
    {
        using TestWorkspace workspace = new TestWorkspace();
        ProfileService service = new ProfileService(workspace.RootPath);
        string profilesDirectoryPath = workspace.GetPath("Profiles");

        Directory.CreateDirectory(profilesDirectoryPath);
        File.WriteAllText(Path.Combine(profilesDirectoryPath, "alpha.profile"), string.Empty);
        File.WriteAllText(Path.Combine(profilesDirectoryPath, "beta.profile"), string.Empty);
        File.WriteAllText(Path.Combine(profilesDirectoryPath, "ignore.txt"), string.Empty);

        List<string> profileNames = service.GetProfileNames();

        Assert.Equal(new[] { "alpha", "beta" }, profileNames.OrderBy(name => name));
    }

    [Fact]
    public void DeleteProfile_RemovesExistingProfileFile()
    {
        using TestWorkspace workspace = new TestWorkspace();
        ProfileService service = new ProfileService(workspace.RootPath);

        service.SaveProfile("deck-run", new ProfileData { GamePath = "/games/hades.exe" });
        string profilePath = workspace.GetPath("Profiles", "deck-run.profile");

        service.DeleteProfile("deck-run");

        Assert.False(File.Exists(profilePath));
    }

    [Fact]
    public void DeleteProfile_ThrowsWhenProfileFileIsMissing()
    {
        using TestWorkspace workspace = new TestWorkspace();
        ProfileService service = new ProfileService(workspace.RootPath);

        FileNotFoundException exception = Assert.Throws<FileNotFoundException>(() => service.DeleteProfile("missing-profile"));

        Assert.EndsWith(Path.Combine("Profiles", "missing-profile.profile"), exception.FileName);
    }

    [Fact]
    public void SaveProfile_ThenLoadProfile_RoundTripsPersistedValues()
    {
        using TestWorkspace workspace = new TestWorkspace();
        ProfileService service = new ProfileService(workspace.RootPath);
        ProfileData expectedProfile = new ProfileData
        {
            GamePath = "/games/hades.exe",
            TrainerPath = "/trainers/hades.exe",
            Dll1Path = "/mods/first.dll",
            Dll2Path = "/mods/second.dll",
            LaunchInject1 = true,
            LaunchInject2 = true,
            LaunchMethod = "CreateProcess"
        };

        service.SaveProfile("deck-run", expectedProfile);

        ProfileData actualProfile = service.LoadProfile("deck-run");

        Assert.Equal(expectedProfile.GamePath, actualProfile.GamePath);
        Assert.Equal(expectedProfile.TrainerPath, actualProfile.TrainerPath);
        Assert.Equal(expectedProfile.Dll1Path, actualProfile.Dll1Path);
        Assert.Equal(expectedProfile.Dll2Path, actualProfile.Dll2Path);
        Assert.Equal(expectedProfile.LaunchInject1, actualProfile.LaunchInject1);
        Assert.Equal(expectedProfile.LaunchInject2, actualProfile.LaunchInject2);
        Assert.Equal(expectedProfile.LaunchMethod, actualProfile.LaunchMethod);
    }

    [Theory]
    [InlineData("../outside")]
    [InlineData("nested/profile")]
    [InlineData("..\\outside")]
    public void SaveProfile_ThrowsForPathTraversalProfileNames(string profileName)
    {
        using TestWorkspace workspace = new TestWorkspace();
        ProfileService service = new ProfileService(workspace.RootPath);

        ArgumentException exception = Assert.Throws<ArgumentException>(() => service.SaveProfile(profileName, new ProfileData()));

        Assert.Equal("profileName", exception.ParamName);
    }

    [Theory]
    [InlineData("../outside")]
    [InlineData("nested/profile")]
    [InlineData("..\\outside")]
    public void LoadProfile_ThrowsForPathTraversalProfileNames(string profileName)
    {
        using TestWorkspace workspace = new TestWorkspace();
        ProfileService service = new ProfileService(workspace.RootPath);

        ArgumentException exception = Assert.Throws<ArgumentException>(() => service.LoadProfile(profileName));

        Assert.Equal("profileName", exception.ParamName);
    }

    [Fact]
    public void SaveProfile_AllowsSafeNamesWithDotsAndSpaces()
    {
        using TestWorkspace workspace = new TestWorkspace();
        ProfileService service = new ProfileService(workspace.RootPath);
        ProfileData profile = new ProfileData { GamePath = "/games/hades.exe" };

        service.SaveProfile("Steam Deck v1.0", profile);

        Assert.True(File.Exists(workspace.GetPath("Profiles", "Steam Deck v1.0.profile")));
    }
}
