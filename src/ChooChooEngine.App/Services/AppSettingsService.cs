using System;
using System.IO;

namespace ChooChooEngine.App.Services;

public sealed class AppSettingsService
{
    private readonly string _settingsDirectoryPath;
    private readonly string _settingsPath;

    public AppSettingsService(string startupPath)
    {
        ArgumentNullException.ThrowIfNull(startupPath);
        _settingsDirectoryPath = Path.Combine(startupPath, "Settings");
        _settingsPath = Path.Combine(_settingsDirectoryPath, "AppSettings.ini");
    }

    public AppSettingsData LoadAppSettings()
    {
        Directory.CreateDirectory(_settingsDirectoryPath);

        AppSettingsData settings = new AppSettingsData();

        if (!File.Exists(_settingsPath))
        {
            return settings;
        }

        string[] lines = File.ReadAllLines(_settingsPath);

        foreach (string line in lines)
        {
            string[] parts = line.Split(new char[] { '=' }, 2);

            if (parts.Length != 2)
            {
                continue;
            }

            string key = parts[0];
            string value = parts[1];

            switch (key)
            {
                case "AutoLoadLastProfile":
                    settings.AutoLoadLastProfile = bool.Parse(value);
                    break;

                case "LastUsedProfile":
                    settings.LastUsedProfile = value;
                    break;
            }
        }

        return settings;
    }

    public void SaveAppSettings(AppSettingsData settings)
    {
        ArgumentNullException.ThrowIfNull(settings);

        Directory.CreateDirectory(_settingsDirectoryPath);

        using (StreamWriter writer = new StreamWriter(_settingsPath))
        {
            writer.WriteLine($"AutoLoadLastProfile={settings.AutoLoadLastProfile}");
            writer.WriteLine($"LastUsedProfile={settings.LastUsedProfile}");
        }
    }
}

public sealed class AppSettingsData
{
    public bool AutoLoadLastProfile { get; set; }

    public string LastUsedProfile { get; set; } = string.Empty;
}
