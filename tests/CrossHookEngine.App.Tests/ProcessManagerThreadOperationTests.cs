using CrossHookEngine.App.Core;

namespace CrossHookkEngine.App.Tests;

public class ProcessManagerThreadOperationTests
{
    [Fact]
    public void TryExecuteThreadOperation_ReturnsFalse_WhenOpenThreadFails()
    {
        List<string> failures = new();
        bool closed = false;

        bool result = ProcessManager.TryExecuteThreadOperation(
            42,
            "SuspendThread",
            () => IntPtr.Zero,
            _ => 0,
            _ => closed = true,
            () => 5,
            failures.Add);

        Assert.False(result);
        Assert.False(closed);
        Assert.Single(failures);
        Assert.Contains("OpenThread for SuspendThread on thread 42 failed with Win32 error 5", failures[0]);
    }

    [Fact]
    public void TryExecuteThreadOperation_ReturnsFalse_WhenThreadOperationFails()
    {
        List<string> failures = new();
        IntPtr closedHandle = IntPtr.Zero;
        IntPtr openedHandle = new(1234);

        bool result = ProcessManager.TryExecuteThreadOperation(
            7,
            "ResumeThread",
            () => openedHandle,
            _ => uint.MaxValue,
            handle => closedHandle = handle,
            () => 6,
            failures.Add);

        Assert.False(result);
        Assert.Equal(openedHandle, closedHandle);
        Assert.Single(failures);
        Assert.Contains("ResumeThread on thread 7 failed with Win32 error 6", failures[0]);
    }

    [Fact]
    public void TryExecuteThreadOperation_ReturnsTrue_WhenOperationSucceeds()
    {
        List<string> failures = new();
        IntPtr closedHandle = IntPtr.Zero;
        IntPtr openedHandle = new(5678);

        bool result = ProcessManager.TryExecuteThreadOperation(
            9,
            "SuspendThread",
            () => openedHandle,
            _ => 0,
            handle => closedHandle = handle,
            () => 0,
            failures.Add);

        Assert.True(result);
        Assert.Equal(openedHandle, closedHandle);
        Assert.Empty(failures);
    }
}
