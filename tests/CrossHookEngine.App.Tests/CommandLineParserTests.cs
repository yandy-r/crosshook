using CrossHookEngine.App.Services;

namespace CrossHookkEngine.App.Tests;

public sealed class CommandLineParserTests
{
    [Fact]
    public void Parse_ReturnsDefaultsForNullOrEmptyArguments()
    {
        CommandLineParser parser = new CommandLineParser();

        CommandLineOptions nullOptions = parser.Parse(null);
        CommandLineOptions emptyOptions = parser.Parse(Array.Empty<string>());

        Assert.Empty(nullOptions.ProfilesToLoad);
        Assert.False(nullOptions.AutoLaunchRequested);
        Assert.Equal(string.Empty, nullOptions.AutoLaunchPath);

        Assert.Empty(emptyOptions.ProfilesToLoad);
        Assert.False(emptyOptions.AutoLaunchRequested);
        Assert.Equal(string.Empty, emptyOptions.AutoLaunchPath);
    }

    [Fact]
    public void Parse_CollectsProfilesCaseInsensitivelyAndTrimsQuotes()
    {
        CommandLineParser parser = new CommandLineParser();

        CommandLineOptions options = parser.Parse(new[] { "-P", "\"Deck Profile\"", "-p", "Fallback" });

        Assert.Equal(new[] { "Deck Profile", "Fallback" }, options.ProfilesToLoad);
        Assert.False(options.AutoLaunchRequested);
        Assert.Equal(string.Empty, options.AutoLaunchPath);
    }

    [Fact]
    public void Parse_AutoLaunchTrimsSingleArgumentQuotes()
    {
        CommandLineParser parser = new CommandLineParser();

        CommandLineOptions options = parser.Parse(new[] { "-autolaunch", "\"/games/My Game.exe\"" });

        Assert.True(options.AutoLaunchRequested);
        Assert.Equal("/games/My Game.exe", options.AutoLaunchPath);
    }

    [Fact]
    public void Parse_AutoLaunchConsumesTheRemainingArgumentTail()
    {
        CommandLineParser parser = new CommandLineParser();

        CommandLineOptions options = parser.Parse(new[] { "-p", "Deck", "-autolaunch", "game.exe", "--trainer", "modded" });

        Assert.Equal(new[] { "Deck" }, options.ProfilesToLoad);
        Assert.True(options.AutoLaunchRequested);
        Assert.Equal("game.exe --trainer modded", options.AutoLaunchPath);
    }

    [Fact]
    public void Parse_IgnoresDanglingFlagsWithoutValues()
    {
        CommandLineParser parser = new CommandLineParser();

        CommandLineOptions options = parser.Parse(new[] { "-p", "Deck", "-autolaunch" });

        Assert.Equal(new[] { "Deck" }, options.ProfilesToLoad);
        Assert.False(options.AutoLaunchRequested);
        Assert.Equal(string.Empty, options.AutoLaunchPath);
    }

    [Fact]
    public void Parse_IgnoresUnknownFlagsAndContinuesParsingKnownArguments()
    {
        CommandLineParser parser = new CommandLineParser();

        CommandLineOptions options = parser.Parse(
            new[] { "--mystery", "shadow", "-p", "Deck", "--noop", "placeholder", "-autolaunch", "game.exe" });

        Assert.Equal(new[] { "Deck" }, options.ProfilesToLoad);
        Assert.True(options.AutoLaunchRequested);
        Assert.Equal("game.exe", options.AutoLaunchPath);
    }
}
