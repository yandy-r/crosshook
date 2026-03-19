using System;
using System.Threading;
using System.Windows.Forms;
using CrossHookEngine.App.Diagnostics;
using CrossHookEngine.App.Forms;

namespace CrossHookEngine.App
{
    static class Program
    {
        private static Mutex _mutex = null;
        private const string MutexName = "CrossHookEngineInjectorSingleInstance";
        private static string _logFilePath;

        /// <summary>
        /// The main entry point for the application.
        /// </summary>
        [STAThread]
        static void Main(string[] args)
        {
            _logFilePath = AppDiagnostics.InitializeTraceLogging();
            AppDomain.CurrentDomain.UnhandledException += OnUnhandledException;

            bool createdNew;
            _mutex = new Mutex(true, MutexName, out createdNew);

            if (!createdNew)
            {
                // Another instance is already running
                MessageBox.Show("CrossHook Injection Engine is already running!", 
                    "Already Running", 
                    MessageBoxButtons.OK, 
                    MessageBoxIcon.Information);
                return;
            }

            try
            {
                Application.SetUnhandledExceptionMode(UnhandledExceptionMode.CatchException);
                Application.ThreadException += OnThreadException;
                Application.EnableVisualStyles();
                Application.SetCompatibleTextRenderingDefault(false);
                Application.Run(new MainForm(args));
            }
            finally
            {
                Application.ThreadException -= OnThreadException;
                AppDomain.CurrentDomain.UnhandledException -= OnUnhandledException;

                // Release the mutex
                _mutex.ReleaseMutex();
                AppDiagnostics.ShutdownTraceLogging();
            }
        }

        private static void OnThreadException(object sender, ThreadExceptionEventArgs e)
        {
            HandleUnhandledException("UI thread", e.Exception, isTerminating: false);
        }

        private static void OnUnhandledException(object sender, UnhandledExceptionEventArgs e)
        {
            Exception exception = AppDiagnostics.NormalizeUnhandledException(e.ExceptionObject);
            HandleUnhandledException("AppDomain", exception, e.IsTerminating);
        }

        private static void HandleUnhandledException(string source, Exception exception, bool isTerminating)
        {
            AppDiagnostics.LogUnhandledException(source, exception, isTerminating);

            MessageBox.Show(
                AppDiagnostics.CreateUserFacingCrashMessage(_logFilePath),
                "Unexpected Error",
                MessageBoxButtons.OK,
                MessageBoxIcon.Error);
        }
    }
} 
