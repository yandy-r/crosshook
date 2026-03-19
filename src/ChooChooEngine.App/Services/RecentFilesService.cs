using System;
using System.Collections.Generic;
using System.IO;

namespace ChooChooEngine.App.Services
{
    public sealed class RecentFilesService
    {
        private readonly string _settingsPath;

        public RecentFilesService(string startupPath)
        {
            if (startupPath == null)
            {
                throw new ArgumentNullException(nameof(startupPath));
            }

            _settingsPath = Path.Combine(startupPath, "settings.ini");
        }

        public RecentFilesData LoadRecentFiles()
        {
            RecentFilesData recentFiles = new RecentFilesData();

            if (!File.Exists(_settingsPath))
            {
                return recentFiles;
            }

            string[] lines = File.ReadAllLines(_settingsPath);
            string section = string.Empty;

            foreach (string line in lines)
            {
                if (string.IsNullOrWhiteSpace(line) || line.StartsWith(";"))
                {
                    continue;
                }

                if (line.StartsWith("[") && line.EndsWith("]"))
                {
                    section = line.Substring(1, line.Length - 2);
                    continue;
                }

                if (!File.Exists(line))
                {
                    continue;
                }

                switch (section)
                {
                    case "RecentGamePaths":
                        recentFiles.GamePaths.Add(line);
                        break;

                    case "RecentTrainerPaths":
                        recentFiles.TrainerPaths.Add(line);
                        break;

                    case "RecentDllPaths":
                        recentFiles.DllPaths.Add(line);
                        break;
                }
            }

            return recentFiles;
        }

        public void SaveRecentFiles(RecentFilesData recentFiles)
        {
            if (recentFiles == null)
            {
                throw new ArgumentNullException(nameof(recentFiles));
            }

            using (StreamWriter writer = new StreamWriter(_settingsPath))
            {
                writer.WriteLine("[RecentGamePaths]");
                foreach (string path in recentFiles.GamePaths)
                {
                    writer.WriteLine(path);
                }

                writer.WriteLine();
                writer.WriteLine("[RecentTrainerPaths]");
                foreach (string path in recentFiles.TrainerPaths)
                {
                    writer.WriteLine(path);
                }

                writer.WriteLine();
                writer.WriteLine("[RecentDllPaths]");
                foreach (string path in recentFiles.DllPaths)
                {
                    writer.WriteLine(path);
                }
            }
        }
    }

    public sealed class RecentFilesData
    {
        public List<string> GamePaths { get; }

        public List<string> TrainerPaths { get; }

        public List<string> DllPaths { get; }

        public RecentFilesData()
        {
            GamePaths = new List<string>();
            TrainerPaths = new List<string>();
            DllPaths = new List<string>();
        }

        public RecentFilesData(IEnumerable<string> gamePaths, IEnumerable<string> trainerPaths, IEnumerable<string> dllPaths)
            : this()
        {
            AddRange(GamePaths, gamePaths);
            AddRange(TrainerPaths, trainerPaths);
            AddRange(DllPaths, dllPaths);
        }

        private static void AddRange(List<string> target, IEnumerable<string> values)
        {
            if (values == null)
            {
                return;
            }

            foreach (string value in values)
            {
                target.Add(value);
            }
        }
    }
}
