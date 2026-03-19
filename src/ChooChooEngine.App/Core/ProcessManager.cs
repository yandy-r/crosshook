using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.ComponentModel;
using System.IO;
using System.Threading;
using ChooChooEngine.App.Interop;

namespace ChooChooEngine.App.Core
{
    public partial class ProcessManager : IDisposable
    {
        #region Win32 API

	[LibraryImport("kernel32.dll", SetLastError = true)]
	private static partial IntPtr OpenThread(int dwDesiredAccess, [MarshalAs(UnmanagedType.Bool)] bool bInheritHandle, uint dwThreadId);

	[LibraryImport("kernel32.dll", SetLastError = true)]
	private static partial uint SuspendThread(IntPtr hThread);

	[LibraryImport("kernel32.dll", SetLastError = true)]
	private static partial uint ResumeThread(IntPtr hThread);

        [LibraryImport("kernel32.dll", SetLastError = true, EntryPoint = "CreateProcessW", StringMarshalling = StringMarshalling.Utf16)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static partial bool CreateProcess(string lpApplicationName, string lpCommandLine, 
            IntPtr lpProcessAttributes, IntPtr lpThreadAttributes, [MarshalAs(UnmanagedType.Bool)] bool bInheritHandles, uint dwCreationFlags, 
            IntPtr lpEnvironment, string lpCurrentDirectory, ref STARTUPINFO lpStartupInfo, 
            out PROCESS_INFORMATION lpProcessInformation);
        
        [LibraryImport("Dbghelp.dll", SetLastError = true)]
        [return: MarshalAs(UnmanagedType.Bool)]
        private static partial bool MiniDumpWriteDump(IntPtr hProcess, int ProcessId, IntPtr hFile, 
            int DumpType, IntPtr ExceptionParam, IntPtr UserStreamParam, IntPtr CallbackParam);

        [StructLayout(LayoutKind.Sequential)]
        private struct STARTUPINFO
        {
            public int cb;
            public IntPtr lpReserved;
            public IntPtr lpDesktop;
            public IntPtr lpTitle;
            public int dwX;
            public int dwY;
            public int dwXSize;
            public int dwYSize;
            public int dwXCountChars;
            public int dwYCountChars;
            public int dwFillAttribute;
            public int dwFlags;
            public short wShowWindow;
            public short cbReserved2;
            public IntPtr lpReserved2;
            public IntPtr hStdInput;
            public IntPtr hStdOutput;
            public IntPtr hStdError;
        }

        [StructLayout(LayoutKind.Sequential)]
        private struct PROCESS_INFORMATION
        {
            public IntPtr hProcess;
            public IntPtr hThread;
            public int dwProcessId;
            public int dwThreadId;
        }

        // Access rights
        private const int PROCESS_CREATE_THREAD = 0x0002;
        private const int PROCESS_QUERY_INFORMATION = 0x0400;
        private const int PROCESS_VM_OPERATION = 0x0008;
        private const int PROCESS_VM_WRITE = 0x0020;
        private const int PROCESS_VM_READ = 0x0010;
        private const int PROCESS_ALL_ACCESS = 0x1F0FFF;

        // Thread access rights
        private const int THREAD_SUSPEND_RESUME = 0x0002;
        private const int THREAD_GET_CONTEXT = 0x0008;
        private const int THREAD_SET_CONTEXT = 0x0010;
        private const int THREAD_ALL_ACCESS = 0x1F03FF;

        // Memory allocation
        private const uint MEM_COMMIT = 0x1000;
        private const uint MEM_RESERVE = 0x2000;
        private const uint MEM_RELEASE = 0x8000;
        private const uint PAGE_READWRITE = 0x04;
        private const uint CREATE_SUSPENDED = 0x00000004;

        // MiniDump types
        private const int MiniDumpNormal = 0x00000000;
        private const int MiniDumpWithFullMemory = 0x00000002;

        #endregion

        private Process _process;
        private IntPtr _processHandle;
        private bool _processHandleOpen = false;
	private bool _disposed;

        public event EventHandler<ProcessEventArgs> ProcessStarted;
        public event EventHandler<ProcessEventArgs> ProcessStopped;
        public event EventHandler<ProcessEventArgs> ProcessAttached;
        public event EventHandler<ProcessEventArgs> ProcessDetached;

        public Process CurrentProcess => _process;
        public bool IsProcessRunning => _process != null && !_process.HasExited;
        public int ProcessId => _process?.Id ?? -1;

        public ProcessManager()
        {
        }

        public bool LaunchProcess(string exePath, string workingDir = null, LaunchMethod method = LaunchMethod.CreateProcess)
        {
            if (string.IsNullOrEmpty(exePath) || !File.Exists(exePath))
                return false;

            if (string.IsNullOrEmpty(workingDir))
                workingDir = Path.GetDirectoryName(exePath);

            try
            {
                switch (method)
                {
                    case LaunchMethod.CreateProcess:
                        return LaunchWithCreateProcess(exePath, workingDir);
                    case LaunchMethod.CmdStart:
                        return LaunchWithCmd(exePath, workingDir);
                    case LaunchMethod.CreateThreadInjection:
                        return LaunchWithCreateThreadInjection(exePath, workingDir);
                    case LaunchMethod.RemoteThreadInjection:
                        return LaunchWithRemoteThreadInjection(exePath, workingDir);
                    case LaunchMethod.ShellExecute:
                        return LaunchWithShellExecute(exePath, workingDir);
                    case LaunchMethod.ProcessStart:
                        return LaunchWithProcessStart(exePath, workingDir);
                    default:
                        return false;
                }
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"Error launching process: {ex.Message}");
                return false;
            }
        }

        public bool AttachToProcess(int processId)
        {
            try
            {
                _process = Process.GetProcessById(processId);
                OpenProcessHandle();
                OnProcessAttached(new ProcessEventArgs(_process));
                return true;
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"Error attaching to process: {ex.Message}");
                return false;
            }
        }

        public bool DetachFromProcess()
        {
            if (_process == null)
                return false;

            CloseProcessHandle();
            OnProcessDetached(new ProcessEventArgs(_process));
	    _process.Dispose();
            _process = null;
            return true;
        }

        public bool KillProcess()
        {
            if (_process == null || _process.HasExited)
                return false;

            try
            {
                _process.Kill();
		CloseProcessHandle();
                OnProcessStopped(new ProcessEventArgs(_process));
		_process.Dispose();
                _process = null;
                return true;
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"Error killing process: {ex.Message}");
                return false;
            }
        }

        public bool SuspendProcess()
        {
            if (_process == null || _process.HasExited)
                return false;

            try
            {
		bool success = true;

                foreach (ProcessThread thread in _process.Threads)
                {
			if (!TryExecuteThreadOperation(
					thread.Id,
					"SuspendThread",
					() => OpenThread(THREAD_SUSPEND_RESUME, false, (uint)thread.Id),
					SuspendThread,
					handle => _ = Kernel32Interop.CloseHandle(handle),
					Marshal.GetLastWin32Error,
					message => Debug.WriteLine(message)))
                    {
				success = false;
                    }
                }

		return success;
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"Error suspending process: {ex.Message}");
                return false;
            }
        }

        public bool ResumeProcess()
        {
            if (_process == null || _process.HasExited)
                return false;

            try
            {
		bool success = true;

                foreach (ProcessThread thread in _process.Threads)
                {
			if (!TryExecuteThreadOperation(
					thread.Id,
					"ResumeThread",
					() => OpenThread(THREAD_SUSPEND_RESUME, false, (uint)thread.Id),
					ResumeThread,
					handle => _ = Kernel32Interop.CloseHandle(handle),
					Marshal.GetLastWin32Error,
					message => Debug.WriteLine(message)))
                    {
				success = false;
                    }
                }

		return success;
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"Error resuming process: {ex.Message}");
                return false;
            }
        }

        public bool CreateMiniDump(string outputPath, bool fullMemory = false)
        {
            if (_process == null || _process.HasExited)
                return false;

            try
            {
                using (FileStream fs = new FileStream(outputPath, FileMode.Create, FileAccess.ReadWrite, FileShare.Write))
                {
                    int dumpType = fullMemory ? MiniDumpWithFullMemory : MiniDumpNormal;
                    MiniDumpWriteDump(_processHandle, _process.Id, fs.SafeFileHandle.DangerousGetHandle(), 
                        dumpType, IntPtr.Zero, IntPtr.Zero, IntPtr.Zero);
                }
                return true;
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"Error creating minidump: {ex.Message}");
                return false;
            }
        }

        public List<ProcessModule> GetProcessModules()
        {
            if (_process == null || _process.HasExited)
                return new List<ProcessModule>();

            try
            {
                List<ProcessModule> modules = new List<ProcessModule>();
                foreach (ProcessModule module in _process.Modules)
                {
                    modules.Add(module);
                }
                return modules;
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"Error getting process modules: {ex.Message}");
                return new List<ProcessModule>();
            }
        }

        public List<ProcessThread> GetProcessThreads()
        {
            if (_process == null || _process.HasExited)
                return new List<ProcessThread>();

            try
            {
                List<ProcessThread> threads = new List<ProcessThread>();
                foreach (ProcessThread thread in _process.Threads)
                {
                    threads.Add(thread);
                }
                return threads;
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"Error getting process threads: {ex.Message}");
                return new List<ProcessThread>();
            }
        }

        public IntPtr GetProcessHandle()
        {
            if (_process == null || _process.HasExited)
                return IntPtr.Zero;

            if (!_processHandleOpen)
                OpenProcessHandle();

            return _processHandle;
        }

        private bool OpenProcessHandle()
        {
            if (_process == null || _process.HasExited)
                return false;

            try
            {
		CloseProcessHandle();
                _processHandle = Kernel32Interop.OpenProcess(PROCESS_ALL_ACCESS, false, _process.Id);
                _processHandleOpen = _processHandle != IntPtr.Zero;

		if (!_processHandleOpen)
		{
			int errorCode = Marshal.GetLastWin32Error();
			Debug.WriteLine(Win32ErrorHelper.FormatError($"OpenProcess for process {_process.Id}", errorCode));
		}

                return _processHandleOpen;
            }
            catch (Exception ex)
            {
                Debug.WriteLine($"Error opening process handle: {ex.Message}");
                return false;
            }
        }

        private void CloseProcessHandle()
        {
            if (_processHandleOpen && _processHandle != IntPtr.Zero)
            {
                Kernel32Interop.CloseHandle(_processHandle);
                _processHandle = IntPtr.Zero;
                _processHandleOpen = false;
            }
        }

	public void Dispose()
	{
		Dispose(true);
		GC.SuppressFinalize(this);
	}

	protected virtual void Dispose(bool disposing)
	{
		if (_disposed)
			return;

		CloseProcessHandle();

		if (disposing && _process != null)
		{
			_process.Dispose();
			_process = null;
		}

		_disposed = true;
	}

	internal static bool TryExecuteThreadOperation(
		int threadId,
		string operationName,
		Func<IntPtr> openThread,
		Func<IntPtr, uint> threadOperation,
		Action<IntPtr> closeHandle,
		Func<int> getLastError,
		Action<string> logFailure)
	{
		IntPtr threadHandle = openThread();
		if (threadHandle == IntPtr.Zero)
		{
			logFailure(Win32ErrorHelper.FormatError($"OpenThread for {operationName} on thread {threadId}", getLastError()));
			return false;
		}

		try
		{
			uint result = threadOperation(threadHandle);
			if (result == uint.MaxValue)
			{
				logFailure(Win32ErrorHelper.FormatError($"{operationName} on thread {threadId}", getLastError()));
				return false;
			}

			return true;
		}
		finally
		{
			closeHandle(threadHandle);
		}
	}

	internal static string GetUnsupportedLaunchMethodMessage(LaunchMethod method)
	{
		return method switch
		{
			LaunchMethod.CreateThreadInjection => "CreateThreadInjection launch is not implemented. Refusing to fall back to CreateProcess.",
			LaunchMethod.RemoteThreadInjection => "RemoteThreadInjection launch is not implemented. Refusing to fall back to CreateProcess.",
			_ => $"Launch method '{method}' is not supported."
		};
	}

	internal static bool TryRequireStartedProcess(
		Process process,
		string operationName,
		Action<string> logFailure,
		out Process startedProcess)
	{
		if (process is null)
		{
			logFailure($"{operationName} returned null. The target process may have been reused by the shell.");
			startedProcess = null;
			return false;
		}

		startedProcess = process;
		return true;
	}

        #region Launch Methods

        private bool LaunchWithCreateProcess(string exePath, string workingDir)
        {
            STARTUPINFO startupInfo = new STARTUPINFO();
            startupInfo.cb = Marshal.SizeOf(startupInfo);
            PROCESS_INFORMATION processInfo = new PROCESS_INFORMATION();

            bool result = CreateProcess(exePath, null, IntPtr.Zero, IntPtr.Zero, 
                false, 0, IntPtr.Zero, workingDir, ref startupInfo, out processInfo);

            if (result)
            {
                // Close the thread handle immediately — we only need hProcess
                Kernel32Interop.CloseHandle(processInfo.hThread);

                _process = Process.GetProcessById(processInfo.dwProcessId);
                _processHandle = processInfo.hProcess;
                _processHandleOpen = true;
                OnProcessStarted(new ProcessEventArgs(_process));
                return true;
            }

            return false;
        }

        private bool LaunchWithCmd(string exePath, string workingDir)
        {
            ProcessStartInfo startInfo = new ProcessStartInfo
            {
                FileName = "cmd.exe",
                Arguments = $"/c start \"\" \"{exePath}\"",
                WorkingDirectory = workingDir,
                CreateNoWindow = true,
                UseShellExecute = false
            };

			if (!TryRequireStartedProcess(Process.Start(startInfo), "Process.Start for cmd.exe launcher", message => Debug.WriteLine(message), out Process cmdProcess))
				return false;

			cmdProcess.WaitForExit();

            // We need to find our newly created process
            // This is a simplistic approach and may not work in all cases
            Process[] processes = Process.GetProcessesByName(Path.GetFileNameWithoutExtension(exePath));
            if (processes.Length > 0)
            {
                _process = processes[0];
                OpenProcessHandle();
                OnProcessStarted(new ProcessEventArgs(_process));
                return true;
            }

            return false;
        }

        private bool LaunchWithCreateThreadInjection(string exePath, string workingDir)
        {
			_ = exePath;
			_ = workingDir;
			Debug.WriteLine(GetUnsupportedLaunchMethodMessage(LaunchMethod.CreateThreadInjection));
			return false;
        }

        private bool LaunchWithRemoteThreadInjection(string exePath, string workingDir)
        {
			_ = exePath;
			_ = workingDir;
			Debug.WriteLine(GetUnsupportedLaunchMethodMessage(LaunchMethod.RemoteThreadInjection));
			return false;
        }

        private bool LaunchWithShellExecute(string exePath, string workingDir)
        {
            ProcessStartInfo startInfo = new ProcessStartInfo
            {
                FileName = exePath,
                WorkingDirectory = workingDir,
                UseShellExecute = true
            };

			if (!TryRequireStartedProcess(Process.Start(startInfo), "Process.Start for shell execute launch", message => Debug.WriteLine(message), out Process process))
				return false;

			_process = process;
            OpenProcessHandle();
            OnProcessStarted(new ProcessEventArgs(_process));
            return true;
        }

        private bool LaunchWithProcessStart(string exePath, string workingDir)
        {
            ProcessStartInfo startInfo = new ProcessStartInfo
            {
                FileName = exePath,
                WorkingDirectory = workingDir,
                UseShellExecute = false
            };

			if (!TryRequireStartedProcess(Process.Start(startInfo), "Process.Start for direct launch", message => Debug.WriteLine(message), out Process process))
				return false;

			_process = process;
            OpenProcessHandle();
            OnProcessStarted(new ProcessEventArgs(_process));
            return true;
        }

        #endregion

        #region Event Methods

        protected virtual void OnProcessStarted(ProcessEventArgs e)
        {
            ProcessStarted?.Invoke(this, e);
        }

        protected virtual void OnProcessStopped(ProcessEventArgs e)
        {
            ProcessStopped?.Invoke(this, e);
        }

        protected virtual void OnProcessAttached(ProcessEventArgs e)
        {
            ProcessAttached?.Invoke(this, e);
        }

        protected virtual void OnProcessDetached(ProcessEventArgs e)
        {
            ProcessDetached?.Invoke(this, e);
        }

        #endregion
    }

    public enum LaunchMethod
    {
        CreateProcess,
        CmdStart,
        CreateThreadInjection,
        RemoteThreadInjection,
        ShellExecute,
        ProcessStart
    }

    public class ProcessEventArgs : EventArgs
    {
        public Process Process { get; }

        public ProcessEventArgs(Process process)
        {
            Process = process;
        }
    }
}
