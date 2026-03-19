using System.Text;
using CrossHookEngine.App.Injection;

namespace CrossHookkEngine.App.Tests;

public class InjectionManagerTests
{
    [Fact]
    public void TryReadIsDll64Bit_ReturnsFalse_ForPe32Magic()
    {
        using MemoryStream stream = CreatePeStream(0x10B);

        bool? is64Bit = InjectionManager.TryReadIsDll64Bit(stream);

        Assert.True(is64Bit.HasValue);
        Assert.False(is64Bit.Value);
    }

    [Fact]
    public void TryReadIsDll64Bit_ReturnsTrue_ForPe32PlusMagic()
    {
        using MemoryStream stream = CreatePeStream(0x20B);

        bool? is64Bit = InjectionManager.TryReadIsDll64Bit(stream);

        Assert.True(is64Bit.HasValue);
        Assert.True(is64Bit.Value);
    }

    [Fact]
    public void TryReadIsDll64Bit_ReturnsNull_ForUnknownMagic()
    {
        using MemoryStream stream = CreatePeStream(0x999);

        bool? is64Bit = InjectionManager.TryReadIsDll64Bit(stream);

        Assert.Null(is64Bit);
    }

    [Theory]
    [InlineData(0x102, true, 259, "timed out")]
    [InlineData(0x80, true, 259, "WAIT_ABANDONED")]
    [InlineData(0xFFFFFFFF, true, 259, "Win32 error 5")]
    [InlineData(0x1234, true, 259, "unexpected result")]
    public void GetRemoteThreadFailureMessage_ReturnsWaitFailure(uint waitResult, bool gotExitCode, uint exitCode, string expected)
    {
        string failureMessage = InjectionManager.GetRemoteThreadFailureMessage(waitResult, gotExitCode, exitCode, () => 5);

        Assert.Contains(expected, failureMessage);
    }

    [Fact]
    public void GetRemoteThreadFailureMessage_ReturnsExitCodeFailure_WhenGetExitCodeThreadFails()
    {
        string failureMessage = InjectionManager.GetRemoteThreadFailureMessage(0, false, 0, () => 6);

        Assert.Contains("GetExitCodeThread", failureMessage);
        Assert.Contains("Win32 error 6", failureMessage);
    }

    [Fact]
    public void GetRemoteThreadFailureMessage_ReturnsStillActiveFailure_WhenThreadDidNotExit()
    {
        string failureMessage = InjectionManager.GetRemoteThreadFailureMessage(0, true, 259, () => 0);

        Assert.Contains("still active", failureMessage);
    }

    [Fact]
    public void GetRemoteThreadFailureMessage_ReturnsNull_WhenWaitAndExitCodeSucceed()
    {
        string failureMessage = InjectionManager.GetRemoteThreadFailureMessage(0, true, 0x100000, () => 0);

        Assert.Null(failureMessage);
    }

    private static MemoryStream CreatePeStream(ushort optionalHeaderMagic)
    {
        byte[] bytes = new byte[512];

        using (MemoryStream stream = new MemoryStream(bytes, writable: true))
        using (BinaryWriter writer = new BinaryWriter(stream, Encoding.UTF8, leaveOpen: true))
        {
            stream.Position = 0x3C;
            writer.Write((uint)0x80);

            stream.Position = 0x80;
            writer.Write(0x00004550u);

            stream.Position = 0x80 + 4 + 20;
            writer.Write(optionalHeaderMagic);
        }

        return new MemoryStream(bytes, writable: false);
    }
}
