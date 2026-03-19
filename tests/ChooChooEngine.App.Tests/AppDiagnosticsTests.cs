using ChooChooEngine.App.Diagnostics;

namespace ChooChooEngine.App.Tests;

[Collection("Trace diagnostics")]
public sealed class AppDiagnosticsTests : IDisposable
{
    public void Dispose()
    {
        AppDiagnostics.ShutdownTraceLogging();
    }

    [Fact]
    public void InitializeTraceLogging_CreatesLogFile_AndPersistsMessages()
    {
        using TestWorkspace workspace = new();

        string logFilePath = AppDiagnostics.InitializeTraceLogging(workspace.RootPath);
        AppDiagnostics.LogError("release-safe diagnostic message");
        AppDiagnostics.ShutdownTraceLogging();

        Assert.True(File.Exists(logFilePath));

        string contents = File.ReadAllText(logFilePath);
        Assert.Contains("Trace logging initialized", contents);
        Assert.Contains("release-safe diagnostic message", contents);
    }

    [Fact]
    public void NormalizeUnhandledException_WrapsNonExceptionObjects()
    {
        Exception exception = AppDiagnostics.NormalizeUnhandledException("boom");

        Assert.IsType<InvalidOperationException>(exception);
        Assert.Contains("boom", exception.Message);
    }

    [Fact]
    public void CreateUnhandledExceptionLogMessage_IncludesSourceAndTerminationState()
    {
        InvalidOperationException exception = new("crash");

        string message = AppDiagnostics.CreateUnhandledExceptionLogMessage("UI thread", exception, isTerminating: true);

        Assert.Contains("UI thread", message);
        Assert.Contains("terminating=True", message);
        Assert.Contains("crash", message);
    }
}
