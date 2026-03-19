using ChooChooEngine.App.Forms;
using ChooChooEngine.App.Services;

namespace ChooChooEngine.App.Tests;

public sealed class MainFormStartupCoordinatorTests
{
    [Fact]
    public void ResolveAutoLoadProfileName_ReturnsLastUsedProfile_WhenEnabledAndProfileExists()
    {
        CommandLineOptions options = new CommandLineOptions();

        string profileName = MainFormStartupCoordinator.ResolveAutoLoadProfileName(
            autoLoadLastProfile: true,
            lastUsedProfile: "deck-run",
            availableProfiles: new[] { "deck-run", "speedrun" },
            options);

        Assert.Equal("deck-run", profileName);
    }

    [Fact]
    public void ResolveAutoLoadProfileName_ReturnsEmpty_WhenProfileMissing()
    {
        CommandLineOptions options = new CommandLineOptions();

        string profileName = MainFormStartupCoordinator.ResolveAutoLoadProfileName(
            autoLoadLastProfile: true,
            lastUsedProfile: "deck-run",
            availableProfiles: new[] { "speedrun" },
            options);

        Assert.Equal(string.Empty, profileName);
    }

    [Fact]
    public void ResolveAutoLoadProfileName_ReturnsEmpty_WhenCommandLineProfileIsRequested()
    {
        CommandLineOptions options = new CommandLineOptions();
        options.ProfilesToLoad.Add("cli-profile");

        string profileName = MainFormStartupCoordinator.ResolveAutoLoadProfileName(
            autoLoadLastProfile: true,
            lastUsedProfile: "deck-run",
            availableProfiles: new[] { "deck-run", "cli-profile" },
            options);

        Assert.Equal(string.Empty, profileName);
    }
}
