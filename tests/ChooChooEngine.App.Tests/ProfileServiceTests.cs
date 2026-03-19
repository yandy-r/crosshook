using ChooChooEngine.App.Services;

namespace ChooChooEngine.App.Tests;

public sealed class ProfileServiceTests
{
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
