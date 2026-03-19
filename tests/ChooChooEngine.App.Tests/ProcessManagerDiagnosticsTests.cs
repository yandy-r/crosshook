using System.Diagnostics;
using ChooChooEngine.App.Core;

namespace ChooChooEngine.App.Tests;

public sealed class ProcessManagerDiagnosticsTests
{
    [Fact]
    public void GetMiniDumpFailureMessage_ReturnsNull_WhenDumpSucceeds()
    {
        string message = ProcessManager.GetMiniDumpFailureMessage(writeDumpResult: true, () => 5);

        Assert.Null(message);
    }

    [Fact]
    public void GetMiniDumpFailureMessage_ReturnsWin32Error_WhenDumpFails()
    {
        string message = ProcessManager.GetMiniDumpFailureMessage(writeDumpResult: false, () => 5);

        Assert.Contains("MiniDumpWriteDump", message);
        Assert.Contains("Win32 error 5", message);
    }

    [Fact]
    public void CreateProcessSnapshot_ReturnsNull_WhenSourceIsNull()
    {
        Process snapshot = ProcessManager.CreateProcessSnapshot(null);

        Assert.Null(snapshot);
    }

    [Fact]
    public void CreateProcessSnapshot_ReturnsDistinctRunningProcessInstance()
    {
        using Process currentProcess = Process.GetCurrentProcess();
        using Process snapshot = ProcessManager.CreateProcessSnapshot(currentProcess);

        Assert.NotNull(snapshot);
        Assert.NotSame(currentProcess, snapshot);
        Assert.Equal(currentProcess.Id, snapshot.Id);
    }
}
