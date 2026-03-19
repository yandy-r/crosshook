using System.Reflection;
using System.Runtime.InteropServices;
using ChooChooEngine.App.Core;
using ChooChooEngine.App.Interop;
using ChooChooEngine.App.Memory;

namespace ChooChooEngine.App.Tests;

public class InteropLibraryImportTests
{
    [Theory]
    [InlineData(typeof(Kernel32Interop), "OpenProcess")]
    [InlineData(typeof(Kernel32Interop), "CreateRemoteThread")]
    [InlineData(typeof(Kernel32Interop), "WriteProcessMemory")]
    [InlineData(typeof(Kernel32Interop), "VirtualAllocEx")]
    [InlineData(typeof(Kernel32Interop), "VirtualFreeEx")]
    [InlineData(typeof(ProcessManager), "OpenThread")]
    [InlineData(typeof(ProcessManager), "SuspendThread")]
    [InlineData(typeof(ProcessManager), "ResumeThread")]
    [InlineData(typeof(MemoryManager), "ReadProcessMemory")]
    [InlineData(typeof(MemoryManager), "WriteProcessMemory")]
    [InlineData(typeof(MemoryManager), "VirtualQueryEx")]
    public void LibraryImportDeclarationsThatNeedLastError_EnableSetLastError(Type declaringType, string methodName)
    {
        MethodInfo method = declaringType.GetMethod(methodName, BindingFlags.Static | BindingFlags.NonPublic);

        Assert.NotNull(method);

        LibraryImportAttribute attribute = method.GetCustomAttribute<LibraryImportAttribute>();

        Assert.NotNull(attribute);
        Assert.True(attribute.SetLastError, $"{declaringType.FullName}.{methodName} should enable SetLastError.");
    }
}
