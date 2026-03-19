using System;
using System.Diagnostics;
using System.IO;

namespace ChooChooEngine.App.Diagnostics
{
    internal static class AppDiagnostics
    {
        private static readonly object SyncRoot = new object();
        private static TextWriterTraceListener _fileListener;
        private static string _logFilePath;

        internal static string InitializeTraceLogging(string appDataRoot = null)
        {
            string logFilePath = GetLogFilePath(appDataRoot);

            lock (SyncRoot)
            {
                if (_fileListener != null && string.Equals(_logFilePath, logFilePath, StringComparison.OrdinalIgnoreCase))
                {
                    return _logFilePath;
                }

                ShutdownTraceLoggingCore();

                string directoryPath = Path.GetDirectoryName(logFilePath) ?? throw new InvalidOperationException("Log file path must include a directory.");
                Directory.CreateDirectory(directoryPath);

                _fileListener = new TextWriterTraceListener(logFilePath);
                _logFilePath = logFilePath;

                Trace.AutoFlush = true;
                Trace.Listeners.Add(_fileListener);
            }

            LogInfo($"Trace logging initialized at '{logFilePath}'.");
            return logFilePath;
        }

        internal static void ShutdownTraceLogging()
        {
            lock (SyncRoot)
            {
                ShutdownTraceLoggingCore();
            }
        }

        internal static string GetLogFilePath(string appDataRoot = null)
        {
            string baseDirectory = string.IsNullOrWhiteSpace(appDataRoot)
                ? Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData)
                : appDataRoot;

            if (string.IsNullOrWhiteSpace(baseDirectory))
            {
                throw new InvalidOperationException("Unable to resolve the application data directory for diagnostics logging.");
            }

            return Path.Combine(baseDirectory, "ChooChooEngine", "logs", "choochoo.log");
        }

        internal static void LogInfo(string message)
        {
            Trace.WriteLine(FormatLogEntry("INFO", message));
        }

        internal static void LogError(string message)
        {
            Trace.WriteLine(FormatLogEntry("ERROR", message));
        }

        internal static void LogUnhandledException(string source, Exception exception, bool isTerminating)
        {
            LogError(CreateUnhandledExceptionLogMessage(source, exception, isTerminating));
        }

        internal static Exception NormalizeUnhandledException(object exceptionObject)
        {
            if (exceptionObject is Exception exception)
            {
                return exception;
            }

            string valueDescription = exceptionObject?.ToString() ?? "<null>";
            return new InvalidOperationException($"Unhandled exception object was not an Exception. Value: {valueDescription}");
        }

        internal static string CreateUnhandledExceptionLogMessage(string source, Exception exception, bool isTerminating)
        {
            if (exception is null)
            {
                throw new ArgumentNullException(nameof(exception));
            }

            return $"{source} unhandled exception (terminating={isTerminating}): {exception}";
        }

        internal static string CreateUserFacingCrashMessage(string logFilePath)
        {
            return "ChooChoo encountered an unexpected error and needs to close." + Environment.NewLine
                + Environment.NewLine
                + $"A diagnostic log was written to:{Environment.NewLine}{logFilePath}";
        }

        private static string FormatLogEntry(string level, string message)
        {
            return $"[{DateTime.UtcNow:O}] {level}: {message}";
        }

        private static void ShutdownTraceLoggingCore()
        {
            if (_fileListener is null)
            {
                _logFilePath = null;
                return;
            }

            Trace.Listeners.Remove(_fileListener);
            _fileListener.Flush();
            _fileListener.Close();
            _fileListener.Dispose();
            _fileListener = null;
            _logFilePath = null;
        }
    }
}
