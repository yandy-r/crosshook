using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.ComponentModel;
using System.IO;
using System.Threading;
using CrossHookEngine.App.Diagnostics;
using CrossHookEngine.App.Interop;

namespace CrossHookEngine.App.Core
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

        public Process CurrentProcess => CreateProcessSnapshot(_process);
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
                AppDiagnostics.LogError($"Error launching process: {ex}");
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
                AppDiagnostics.LogError($"Error attaching to process: {ex}");
                return false;
            }
        }

        public ProcessReadinessResult WaitForCurrentProcessReady(ProcessReadinessOptions options = null)
        {
            if (_process is null)
            {
                return new ProcessReadinessResult(
                    isReady: false,
                    elapsedMs: 0,
                    modulesAccessible: false,
                    hasMainWindow: false,
                    statusMessage: "No process is currently tracked.",
                    processExitedBeforeReady: false);
            }

            return WaitForProcessReady(_process, options ?? new ProcessReadinessOptions(), milliseconds => Thread.Sleep(milliseconds));
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
                AppDiagnostics.LogError($"Error killing process: {ex}");
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
						AppDiagnostics.LogError))
                    {
				success = false;
                    }
                }

		return success;
            }
            catch (Exception ex)
            {
                AppDiagnostics.LogError($"Error suspending process: {ex}");
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
						AppDiagnostics.LogError))
                    {
				success = false;
                    }
                }

		return success;
            }
            catch (Exception ex)
            {
                AppDiagnostics.LogError($"Error resuming process: {ex}");
                return false;
            }
        }

        public bool CreateMiniDump(string outputPath, bool fullMemory = false)
        {
            if (_process == null || _process.HasExited)
                return false;

            try
            {
                IntPtr processHandle = GetProcessHandle();
                if (processHandle == IntPtr.Zero)
                    return false;

                using (FileStream fs = new FileStream(outputPath, FileMode.Create, FileAccess.ReadWrite, FileShare.Write))
                {
                    int dumpType = fullMemory ? MiniDumpWithFullMemory : MiniDumpNormal;
                    bool writeDumpResult = MiniDumpWriteDump(processHandle, _process.Id, fs.SafeFileHandle.DangerousGetHandle(),
                        dumpType, IntPtr.Zero, IntPtr.Zero, IntPtr.Zero);

                    string failureMessage = GetMiniDumpFailureMessage(writeDumpResult, Marshal.GetLastWin32Error);
                    if (!string.IsNullOrEmpty(failureMessage))
                    {
                        AppDiagnostics.LogError(failureMessage);
                        fs.Close();
                        File.Delete(outputPath);
                        return false;
                    }
                }

                return true;
            }
            catch (Exception ex)
            {
                AppDiagnostics.LogError($"Error creating minidump: {ex}");
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
                AppDiagnostics.LogError($"Error getting process modules: {ex}");
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
                AppDiagnostics.LogError($"Error getting process threads: {ex}");
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
				AppDiagnostics.LogError(Win32ErrorHelper.FormatError($"OpenProcess for process {_process.Id}", errorCode));
			}

                return _processHandleOpen;
            }
            catch (Exception ex)
            {
                AppDiagnostics.LogError($"Error opening process handle: {ex}");
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

	internal static string GetMiniDumpFailureMessage(bool writeDumpResult, Func<int> getLastError)
	{
		if (writeDumpResult)
			return null;

		return Win32ErrorHelper.FormatError("MiniDumpWriteDump", getLastError());
	}

	internal static Process CreateProcessSnapshot(Process process)
	{
		if (process is null)
			return null;

		try
		{
			if (process.HasExited)
				return null;

			return Process.GetProcessById(process.Id);
		}
		catch (ArgumentException)
		{
			return null;
		}
		catch (InvalidOperationException)
		{
			return null;
		}
		catch (Win32Exception)
		{
			return null;
		}
	}

	internal static ProcessReadinessOptions NormalizeProcessReadinessOptions(ProcessReadinessOptions options)
	{
		if (options is null)
			throw new ArgumentNullException(nameof(options));

		if (options.TimeoutMs <= 0)
			throw new ArgumentOutOfRangeException(nameof(options.TimeoutMs), "Timeout must be greater than zero.");

		if (options.PollIntervalMs <= 0)
			throw new ArgumentOutOfRangeException(nameof(options.PollIntervalMs), "Poll interval must be greater than zero.");

		if (options.MinimumProcessLifetimeMs < 0)
			throw new ArgumentOutOfRangeException(nameof(options.MinimumProcessLifetimeMs), "Minimum process lifetime cannot be negative.");

		if (options.TimeoutMs < options.MinimumProcessLifetimeMs)
			throw new ArgumentOutOfRangeException(nameof(options.TimeoutMs), "Timeout must be greater than or equal to the minimum process lifetime.");

		return new ProcessReadinessOptions
		{
			TimeoutMs = options.TimeoutMs,
			PollIntervalMs = options.PollIntervalMs,
			MinimumProcessLifetimeMs = options.MinimumProcessLifetimeMs,
			RequireMainWindow = options.RequireMainWindow
		};
	}

	internal static ProcessReadinessResult WaitForProcessReady(
		Process process,
		ProcessReadinessOptions options,
		Action<int> delay)
	{
		if (process is null)
			throw new ArgumentNullException(nameof(process));

		if (delay is null)
			throw new ArgumentNullException(nameof(delay));

		ProcessReadinessOptions normalizedOptions = NormalizeProcessReadinessOptions(options);
		Stopwatch stopwatch = Stopwatch.StartNew();

		while (true)
		{
			using Process snapshot = CreateProcessSnapshot(process);
			if (snapshot is null)
			{
				return new ProcessReadinessResult(
					isReady: false,
					elapsedMs: (int)stopwatch.ElapsedMilliseconds,
					modulesAccessible: false,
					hasMainWindow: false,
					statusMessage: "Process exited before it became ready.",
					processExitedBeforeReady: true);
			}

			snapshot.Refresh();

			bool modulesAccessible = CanInspectProcessModules(snapshot);
			bool hasMainWindow = TryHasMainWindow(snapshot);
			bool lifetimeSatisfied = stopwatch.ElapsedMilliseconds >= normalizedOptions.MinimumProcessLifetimeMs;
			bool mainWindowSatisfied = !normalizedOptions.RequireMainWindow || hasMainWindow;

			if (lifetimeSatisfied && mainWindowSatisfied)
			{
				string readinessSource = hasMainWindow
					? "main window"
					: modulesAccessible
						? "module enumeration"
						: "stabilization window";

				string statusMessage = hasMainWindow || modulesAccessible
					? $"Process readiness confirmed via {readinessSource}."
					: "Process remained alive for the stabilization window; proceeding without an additional readiness signal.";

				return new ProcessReadinessResult(
					isReady: true,
					elapsedMs: (int)stopwatch.ElapsedMilliseconds,
					modulesAccessible: modulesAccessible,
					hasMainWindow: hasMainWindow,
					statusMessage: statusMessage);
			}

			if (stopwatch.ElapsedMilliseconds >= normalizedOptions.TimeoutMs)
			{
				string timeoutMessage = normalizedOptions.RequireMainWindow
					? "Timed out waiting for the process main window."
					: "Timed out waiting for the process to stabilize.";

				return new ProcessReadinessResult(
					isReady: false,
					elapsedMs: (int)stopwatch.ElapsedMilliseconds,
					modulesAccessible: modulesAccessible,
					hasMainWindow: hasMainWindow,
					statusMessage: timeoutMessage);
			}

			delay(normalizedOptions.PollIntervalMs);
		}
	}

	internal static bool CanInspectProcessModules(Process process)
	{
		if (process is null)
			return false;

		try
		{
			if (process.HasExited)
				return false;

			_ = process.Modules.Count;
			return true;
		}
		catch (ArgumentException)
		{
			return false;
		}
		catch (InvalidOperationException)
		{
			return false;
		}
		catch (Win32Exception)
		{
			return false;
		}
		catch (NotSupportedException)
		{
			return false;
		}
	}

	internal static bool TryHasMainWindow(Process process)
	{
		if (process is null)
			return false;

		try
		{
			if (process.HasExited)
				return false;

			process.Refresh();
			return process.MainWindowHandle != IntPtr.Zero;
		}
		catch (ArgumentException)
		{
			return false;
		}
		catch (InvalidOperationException)
		{
			return false;
		}
		catch (Win32Exception)
		{
			return false;
		}
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

			if (!TryRequireStartedProcess(Process.Start(startInfo), "Process.Start for cmd.exe launcher", AppDiagnostics.LogError, out Process cmdProcess))
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
				AppDiagnostics.LogError(GetUnsupportedLaunchMethodMessage(LaunchMethod.CreateThreadInjection));
				return false;
        }

        private bool LaunchWithRemoteThreadInjection(string exePath, string workingDir)
        {
				_ = exePath;
				_ = workingDir;
				AppDiagnostics.LogError(GetUnsupportedLaunchMethodMessage(LaunchMethod.RemoteThreadInjection));
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

			if (!TryRequireStartedProcess(Process.Start(startInfo), "Process.Start for shell execute launch", AppDiagnostics.LogError, out Process process))
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

			if (!TryRequireStartedProcess(Process.Start(startInfo), "Process.Start for direct launch", AppDiagnostics.LogError, out Process process))
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

    public sealed class ProcessReadinessOptions
    {
        public int TimeoutMs { get; set; } = 15000;
        public int PollIntervalMs { get; set; } = 250;
        public int MinimumProcessLifetimeMs { get; set; } = 2000;
        public bool RequireMainWindow { get; set; } = false;
    }

    public sealed class ProcessReadinessResult
    {
        public bool IsReady { get; }
        public int ElapsedMs { get; }
        public bool ModulesAccessible { get; }
        public bool HasMainWindow { get; }
        public string StatusMessage { get; }

        public bool ProcessExitedBeforeReady { get; }

        public ProcessReadinessResult(bool isReady, int elapsedMs, bool modulesAccessible, bool hasMainWindow, string statusMessage, bool processExitedBeforeReady = false)
        {
            IsReady = isReady;
            ElapsedMs = elapsedMs;
            ModulesAccessible = modulesAccessible;
            HasMainWindow = hasMainWindow;
            StatusMessage = statusMessage ?? string.Empty;
            ProcessExitedBeforeReady = processExitedBeforeReady;
        }
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
