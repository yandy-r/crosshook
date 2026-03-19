using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Runtime.InteropServices.Marshalling;
using System.Text;
using System.Threading;
using System.Timers;
using ChooChooEngine.App.Core;
using ChooChooEngine.App.Diagnostics;
using ChooChooEngine.App.Interop;

namespace ChooChooEngine.App.Injection
{
    public partial class InjectionManager : IDisposable
    {
        #region Win32 API

        [LibraryImport("kernel32.dll", StringMarshalling = StringMarshalling.Utf8)]
        private static partial IntPtr GetProcAddress(IntPtr hModule, string procName);

        [LibraryImport("kernel32.dll", EntryPoint = "GetModuleHandleW", StringMarshalling = StringMarshalling.Utf16)]
        private static partial IntPtr GetModuleHandle(string lpModuleName);

        [LibraryImport("kernel32.dll", EntryPoint = "LoadLibraryW", SetLastError = true, StringMarshalling = StringMarshalling.Utf16)]
        private static partial IntPtr LoadLibrary(string lpFileName);

        [LibraryImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static partial bool FreeLibrary(IntPtr hModule);

        // Access rights
        private const int PROCESS_CREATE_THREAD = 0x0002;
        private const int PROCESS_QUERY_INFORMATION = 0x0400;
        private const int PROCESS_VM_OPERATION = 0x0008;
        private const int PROCESS_VM_WRITE = 0x0020;
        private const int PROCESS_VM_READ = 0x0010;
        private const int PROCESS_ALL_ACCESS = 0x1F0FFF;

        // Memory allocation
        private const uint MEM_COMMIT = 0x1000;
        private const uint MEM_RESERVE = 0x2000;
        private const uint MEM_RELEASE = 0x8000;
        private const uint PAGE_READWRITE = 0x04;
        private const uint PAGE_EXECUTE_READWRITE = 0x40;

	// Wait / PE parsing constants
	private const uint WAIT_OBJECT_0 = 0x00000000;
	private const uint WAIT_ABANDONED = 0x00000080;
	private const uint WAIT_TIMEOUT = 0x00000102;
	private const uint WAIT_FAILED = 0xFFFFFFFF;
	private const uint STILL_ACTIVE = 259;
	private const ushort IMAGE_NT_OPTIONAL_HDR32_MAGIC = 0x10B;
	private const ushort IMAGE_NT_OPTIONAL_HDR64_MAGIC = 0x20B;

        #endregion

        private ProcessManager _processManager;
        private System.Timers.Timer _monitoringTimer;
        private Dictionary<string, bool> _validatedDlls = new Dictionary<string, bool>();
        private readonly object _injectionLock = new object();
	private bool _disposed;
        
        public event EventHandler<InjectionEventArgs> InjectionSucceeded;
        public event EventHandler<InjectionEventArgs> InjectionFailed;
        
        public string MainDllPath { get; set; }
        public List<string> AdditionalDllPaths { get; } = new List<string>(4);
        public bool AutoInject { get; set; } = false;
        public int MonitoringInterval { get; set; } = 1000;
        public bool IsMonitoring { get; private set; } = false;
        public InjectionMethod InjectionMethod { get; set; } = InjectionMethod.StandardInjection;

        public InjectionManager(ProcessManager processManager)
        {
            ArgumentNullException.ThrowIfNull(processManager);

            _processManager = processManager;
            _monitoringTimer = new System.Timers.Timer(MonitoringInterval);
            _monitoringTimer.Elapsed += OnMonitoringTimerElapsed;
        }

        public void StartMonitoring()
        {
            if (!IsMonitoring)
            {
                _monitoringTimer.Interval = MonitoringInterval;
                _monitoringTimer.Start();
                IsMonitoring = true;
            }
        }

        public void StopMonitoring()
        {
            if (IsMonitoring)
            {
                _monitoringTimer.Stop();
                IsMonitoring = false;
            }
        }

	public void Dispose()
	{
		if (_disposed)
			return;

		StopMonitoring();

		if (_monitoringTimer != null)
		{
			_monitoringTimer.Elapsed -= OnMonitoringTimerElapsed;
			_monitoringTimer.Dispose();
			_monitoringTimer = null;
		}

		_disposed = true;
	}

        public bool InjectDll(string dllPath)
        {
            if (string.IsNullOrEmpty(dllPath) || !File.Exists(dllPath))
                return false;

            if (!ValidateDll(dllPath))
            {
                OnInjectionFailed(new InjectionEventArgs(dllPath, "DLL validation failed"));
                return false;
            }

            IntPtr processHandle = _processManager.GetProcessHandle();
            if (processHandle == IntPtr.Zero)
            {
                OnInjectionFailed(new InjectionEventArgs(dllPath, "Process handle is invalid"));
                return false;
            }

            try
            {
                lock (_injectionLock)
                {
                    switch (InjectionMethod)
                    {
                        case InjectionMethod.StandardInjection:
                            return InjectDllStandard(processHandle, dllPath);
						case InjectionMethod.ManualMapping:
							return InjectDllManualMapping(processHandle, dllPath);
						default:
							OnInjectionFailed(new InjectionEventArgs(dllPath, GetUnsupportedInjectionMethodMessage(InjectionMethod)));
							return false;
					}
				}
			}
            catch (Exception ex)
            {
                OnInjectionFailed(new InjectionEventArgs(dllPath, $"Injection failed: {ex.Message}"));
                return false;
            }
        }

        public void InjectAllDlls()
        {
            if (!string.IsNullOrEmpty(MainDllPath))
            {
                InjectDll(MainDllPath);
            }

            foreach (string dllPath in AdditionalDllPaths)
            {
                if (!string.IsNullOrEmpty(dllPath))
                {
                    InjectDll(dllPath);
                }
            }
        }

        public bool ValidateDll(string dllPath)
        {
            if (string.IsNullOrEmpty(dllPath) || !File.Exists(dllPath))
                return false;

            // Check if we've already validated this DLL
            if (_validatedDlls.TryGetValue(dllPath, out bool isValid))
                return isValid;

            try
            {
                // Try to load the DLL in the current process
                IntPtr moduleHandle = LoadLibrary(dllPath);
                if (moduleHandle == IntPtr.Zero)
                {
                    _validatedDlls[dllPath] = false;
                    return false;
                }

                // Check architecture compatibility (32-bit vs 64-bit)
                bool isProcess64Bit = Environment.Is64BitProcess;
                bool isDll64Bit = IsDll64Bit(dllPath);

                // Unload the DLL
                FreeLibrary(moduleHandle);

                // Check if architectures match
                bool architecturesMatch = isProcess64Bit == isDll64Bit;
                _validatedDlls[dllPath] = architecturesMatch;
                
                return architecturesMatch;
            }
            catch (Exception ex)
            {
                AppDiagnostics.LogError($"Error validating DLL: {ex}");
                _validatedDlls[dllPath] = false;
                return false;
            }
        }

        private bool IsDll64Bit(string dllPath)
        {
            try
            {
                // Read the PE header to determine if the DLL is 32-bit or 64-bit
                using (FileStream fs = new FileStream(dllPath, FileMode.Open, FileAccess.Read))
                {
			bool? is64Bit = TryReadIsDll64Bit(fs);
			if (!is64Bit.HasValue)
				return false;

			return is64Bit.Value;
                }
            }
            catch (Exception ex)
            {
                AppDiagnostics.LogError($"Error determining DLL architecture: {ex}");
                return false;
            }
        }

	internal static bool? TryReadIsDll64Bit(Stream dllStream)
	{
		using (BinaryReader br = new BinaryReader(dllStream, Encoding.UTF8, leaveOpen: true))
		{
			// DOS header
			dllStream.Position = 0x3C;
			uint peHeaderOffset = br.ReadUInt32();

			// PE header signature
			dllStream.Position = peHeaderOffset;
			uint peHeaderSignature = br.ReadUInt32();
			if (peHeaderSignature != 0x00004550) // "PE\0\0"
				return null;

			// Skip the 20-byte COFF header and read the Optional Header magic intentionally.
			dllStream.Position += 20;
			ushort magic = br.ReadUInt16();

			return magic switch
			{
				IMAGE_NT_OPTIONAL_HDR32_MAGIC => false,
				IMAGE_NT_OPTIONAL_HDR64_MAGIC => true,
				_ => null
			};
		}
	}

	internal static string GetRemoteThreadFailureMessage(uint waitResult, bool gotExitCode, uint exitCode, Func<int> getLastError)
	{
		switch (waitResult)
		{
			case WAIT_OBJECT_0:
				break;
			case WAIT_TIMEOUT:
				return "LoadLibraryA thread timed out after 5000 ms";
			case WAIT_ABANDONED:
				return "WaitForSingleObject returned WAIT_ABANDONED for the LoadLibraryA thread";
			case WAIT_FAILED:
				return Win32ErrorHelper.FormatError("WaitForSingleObject", getLastError());
			default:
				return $"WaitForSingleObject returned unexpected result {waitResult}";
		}

		if (!gotExitCode)
			return Win32ErrorHelper.FormatError("GetExitCodeThread", getLastError());

		if (exitCode == STILL_ACTIVE)
			return "LoadLibraryA thread is still active after wait completion";

		if (exitCode == 0)
			return "LoadLibraryA returned 0";

		return null;
	}

	internal static string GetUnsupportedInjectionMethodMessage(InjectionMethod injectionMethod)
	{
		return injectionMethod switch
		{
			InjectionMethod.ManualMapping => "Manual mapping is not implemented. Refusing to fall back to standard injection.",
			_ => $"Injection method '{injectionMethod}' is not supported."
		};
	}

        private bool InjectDllStandard(IntPtr processHandle, string dllPath)
        {
            // Keep the remote thread pointed at LoadLibraryA so the ASCII-path injection contract stays unchanged.
            IntPtr loadLibraryAddr = GetProcAddress(GetModuleHandle("kernel32.dll"), "LoadLibraryA");
            if (loadLibraryAddr == IntPtr.Zero)
                return false;

            // Allocate memory in the target process for the DLL path
            byte[] dllPathBytes = Encoding.ASCII.GetBytes(dllPath);
            uint allocSize = (uint)((dllPathBytes.Length + 1) * Marshal.SizeOf(typeof(char)));

            IntPtr remoteMemory = Kernel32Interop.VirtualAllocEx(processHandle, IntPtr.Zero, allocSize, 
                MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
            if (remoteMemory == IntPtr.Zero)
            {
                int errorCode = Marshal.GetLastWin32Error();
                OnInjectionFailed(new InjectionEventArgs(dllPath, Win32ErrorHelper.FormatError("VirtualAllocEx", errorCode)));
                return false;
            }

            try
            {
                // Write the DLL path to the allocated memory
		// Pass exact buffer length - VirtualAllocEx zero-initialized the extra byte for the null terminator
                UIntPtr bytesWritten;
                bool writeResult = Kernel32Interop.WriteProcessMemory(processHandle, remoteMemory, dllPathBytes,
                    (uint)dllPathBytes.Length, out bytesWritten);
                if (!writeResult)
                {
                    int errorCode = Marshal.GetLastWin32Error();
                    OnInjectionFailed(new InjectionEventArgs(dllPath, Win32ErrorHelper.FormatError("WriteProcessMemory", errorCode)));
                    return false;
                }

                if (bytesWritten.ToUInt64() != (ulong)dllPathBytes.Length)
                {
                    OnInjectionFailed(new InjectionEventArgs(dllPath,
                        $"WriteProcessMemory returned {bytesWritten.ToUInt64()} bytes, expected {dllPathBytes.Length}"));
                    return false;
                }

                // Create a remote thread that calls LoadLibraryA with the DLL path as argument
                IntPtr threadHandle = Kernel32Interop.CreateRemoteThread(processHandle, IntPtr.Zero, 0, 
                    loadLibraryAddr, remoteMemory, 0, IntPtr.Zero);
                if (threadHandle == IntPtr.Zero)
                {
                    int errorCode = Marshal.GetLastWin32Error();
                    OnInjectionFailed(new InjectionEventArgs(dllPath, Win32ErrorHelper.FormatError("CreateRemoteThread", errorCode)));
                    return false;
                }

                // Wait for the thread to finish
		uint waitResult = WaitForSingleObject(threadHandle, 5000);

                // Get the thread exit code (should be the handle to the loaded DLL)
                uint exitCode;
		bool gotExitCode = GetExitCodeThread(threadHandle, out exitCode);

                // Clean up
                Kernel32Interop.CloseHandle(threadHandle);

		string failureMessage = GetRemoteThreadFailureMessage(waitResult, gotExitCode, exitCode, Marshal.GetLastWin32Error);
		if (!string.IsNullOrEmpty(failureMessage))
                {
                    OnInjectionFailed(new InjectionEventArgs(dllPath, failureMessage));
                    return false;
                }

                OnInjectionSucceeded(new InjectionEventArgs(dllPath));
                return true;
            }
            finally
            {
                // Free the allocated memory
                if (!Kernel32Interop.VirtualFreeEx(processHandle, remoteMemory, 0, MEM_RELEASE))
                {
                    int errorCode = Marshal.GetLastWin32Error();
                    AppDiagnostics.LogError(Win32ErrorHelper.FormatError("VirtualFreeEx", errorCode));
                }
            }
        }

        private bool InjectDllManualMapping(IntPtr processHandle, string dllPath)
        {
			_ = processHandle;
			OnInjectionFailed(new InjectionEventArgs(dllPath, GetUnsupportedInjectionMethodMessage(InjectionMethod.ManualMapping)));
			return false;
        }

        private void OnMonitoringTimerElapsed(object sender, ElapsedEventArgs e)
        {
            if (AutoInject && _processManager.IsProcessRunning)
            {
                InjectAllDlls();
            }
        }

        #region Additional P/Invoke for thread handling

        [LibraryImport("kernel32.dll", SetLastError = true)]
        private static partial uint WaitForSingleObject(IntPtr hHandle, uint dwMilliseconds);

        [LibraryImport("kernel32.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static partial bool GetExitCodeThread(IntPtr hThread, out uint lpExitCode);

        #endregion

        #region Event Methods

        protected virtual void OnInjectionSucceeded(InjectionEventArgs e)
        {
            InjectionSucceeded?.Invoke(this, e);
        }

        protected virtual void OnInjectionFailed(InjectionEventArgs e)
        {
            InjectionFailed?.Invoke(this, e);
        }

        #endregion
    }

    public enum InjectionMethod
    {
        StandardInjection,
        ManualMapping
    }

    public class InjectionEventArgs : EventArgs
    {
        public string DllPath { get; }
        public string Message { get; }

        public InjectionEventArgs(string dllPath, string message = null)
        {
            DllPath = dllPath;
            Message = message;
        }
    }
}
