using System.ComponentModel;

namespace CrossHookEngine.App.Interop
{
    internal static class Win32ErrorHelper
    {
        internal static string FormatError(string operation, int errorCode)
        {
            string errorDescription = errorCode == 0
                ? "Unknown error"
                : new Win32Exception(errorCode).Message;

            return $"{operation} failed with Win32 error {errorCode}: {errorDescription}";
        }
    }
}
