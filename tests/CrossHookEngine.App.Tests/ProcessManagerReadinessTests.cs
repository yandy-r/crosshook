using System.Diagnostics;
using CrossHookEngine.App.Core;

namespace CrossHookEngine.App.Tests;

public sealed class ProcessManagerReadinessTests
{
    [Fact]
    public void WaitForCurrentProcessReady_ReturnsNotReady_WhenNoProcessIsTracked()
    {
        using ProcessManager manager = new();

        ProcessReadinessResult result = manager.WaitForCurrentProcessReady();

        Assert.False(result.IsReady);
        Assert.Equal("No process is currently tracked.", result.StatusMessage);
    }

    [Fact]
    public void NormalizeProcessReadinessOptions_Throws_WhenTimeoutIsLessThanMinimumLifetime()
    {
        ProcessReadinessOptions options = new()
        {
            TimeoutMs = 1000,
            PollIntervalMs = 50,
            MinimumProcessLifetimeMs = 1500
        };

        ArgumentOutOfRangeException exception = Assert.Throws<ArgumentOutOfRangeException>(() => ProcessManager.NormalizeProcessReadinessOptions(options));

        Assert.Equal("TimeoutMs", exception.ParamName);
    }

    [Fact]
    public void WaitForProcessReady_ReturnsReady_WhenProcessStaysAliveForMinimumLifetime()
    {
        using Process currentProcess = Process.GetCurrentProcess();
        ProcessReadinessOptions options = new()
        {
            TimeoutMs = 200,
            PollIntervalMs = 1,
            MinimumProcessLifetimeMs = 0
        };

        ProcessReadinessResult result = ProcessManager.WaitForProcessReady(currentProcess, options, _ => { });

        Assert.True(result.IsReady);
        Assert.NotEmpty(result.StatusMessage);
    }

    [Fact]
    public void WaitForProcessReady_ReturnsNotReady_WhenProcessHasAlreadyExited()
    {
        ProcessStartInfo startInfo = new()
        {
            FileName = "/usr/bin/env",
            UseShellExecute = false
        };
        startInfo.ArgumentList.Add("true");

        using Process process = Process.Start(startInfo)!;
        process.WaitForExit();

        ProcessReadinessOptions options = new()
        {
            TimeoutMs = 50,
            PollIntervalMs = 1,
            MinimumProcessLifetimeMs = 10
        };

        ProcessReadinessResult result = ProcessManager.WaitForProcessReady(process, options, _ => { });

        Assert.False(result.IsReady);
        Assert.Contains("exited", result.StatusMessage, StringComparison.OrdinalIgnoreCase);
    }
}
