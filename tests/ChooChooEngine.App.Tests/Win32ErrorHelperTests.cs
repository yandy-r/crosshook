using ChooChooEngine.App.Interop;

namespace ChooChooEngine.App.Tests;

public class Win32ErrorHelperTests
{
    [Theory]
    [InlineData("OpenProcess", 5)]
    [InlineData("WriteProcessMemory", 0)]
    public void FormatError_IncludesOperationAndErrorCode(string operation, int errorCode)
    {
        string message = Win32ErrorHelper.FormatError(operation, errorCode);

        Assert.Contains(operation, message);
        Assert.Contains($"Win32 error {errorCode}", message);
    }
}
