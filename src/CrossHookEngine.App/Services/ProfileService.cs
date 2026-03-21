using System;
using System.Collections.Generic;
using System.IO;

namespace CrossHookEngine.App.Services;

public sealed class ProfileService
{
    private readonly string _profilesDirectoryPath;

    public ProfileService(string startupPath)
    {
        ArgumentNullException.ThrowIfNull(startupPath);
        _profilesDirectoryPath = Path.Combine(startupPath, "Profiles");
    }

    public List<string> GetProfileNames()
    {
        if (!Directory.Exists(_profilesDirectoryPath))
        {
            Directory.CreateDirectory(_profilesDirectoryPath);
            return new List<string>();
        }

        string[] profileFiles = Directory.GetFiles(_profilesDirectoryPath, "*.profile");
        List<string> profileNames = new List<string>(profileFiles.Length);

        foreach (string profileFile in profileFiles)
        {
            profileNames.Add(Path.GetFileNameWithoutExtension(profileFile));
        }

        return profileNames;
    }

    public void SaveProfile(string profileName, ProfileData profile)
    {
        ArgumentNullException.ThrowIfNull(profile);

        Directory.CreateDirectory(_profilesDirectoryPath);

        using (StreamWriter writer = new StreamWriter(GetProfilePath(profileName)))
        {
            writer.WriteLine($"GamePath={profile.GamePath}");
            writer.WriteLine($"TrainerPath={profile.TrainerPath}");
            writer.WriteLine($"Dll1Path={profile.Dll1Path}");
            writer.WriteLine($"Dll2Path={profile.Dll2Path}");
            writer.WriteLine($"LaunchInject1={profile.LaunchInject1}");
            writer.WriteLine($"LaunchInject2={profile.LaunchInject2}");
            writer.WriteLine($"LaunchMethod={profile.LaunchMethod}");
            writer.WriteLine($"UseSteamMode={profile.UseSteamMode}");
            writer.WriteLine($"SteamAppId={profile.SteamAppId}");
            writer.WriteLine($"SteamCompatDataPath={profile.SteamCompatDataPath}");
            writer.WriteLine($"SteamProtonPath={profile.SteamProtonPath}");
        }
    }

    public ProfileData LoadProfile(string profileName)
    {
        string profilePath = GetProfilePath(profileName);

        if (!File.Exists(profilePath))
        {
            throw new FileNotFoundException("Profile file not found.", profilePath);
        }

        ProfileData profile = new ProfileData();
        string[] lines = File.ReadAllLines(profilePath);

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
                case "GamePath":
                    profile.GamePath = value;
                    break;

                case "TrainerPath":
                    profile.TrainerPath = value;
                    break;

                case "Dll1Path":
                    profile.Dll1Path = value;
                    break;

                case "Dll2Path":
                    profile.Dll2Path = value;
                    break;

				case "LaunchInject1":
					if (bool.TryParse(value, out bool launchInject1))
					{
						profile.LaunchInject1 = launchInject1;
					}
					break;

				case "LaunchInject2":
					if (bool.TryParse(value, out bool launchInject2))
					{
						profile.LaunchInject2 = launchInject2;
					}
					break;

                case "LaunchMethod":
                    profile.LaunchMethod = value;
                    break;

                case "UseSteamMode":
                    if (bool.TryParse(value, out bool useSteamMode))
                    {
                        profile.UseSteamMode = useSteamMode;
                    }
                    break;

                case "SteamAppId":
                    profile.SteamAppId = value;
                    break;

                case "SteamCompatDataPath":
                    profile.SteamCompatDataPath = value;
                    break;

                case "SteamProtonPath":
                    profile.SteamProtonPath = value;
                    break;
            }
        }

        return profile;
    }

    public void DeleteProfile(string profileName)
    {
        string profilePath = GetProfilePath(profileName);

        if (!File.Exists(profilePath))
        {
            throw new FileNotFoundException("Profile file not found.", profilePath);
        }

        File.Delete(profilePath);
    }

    private string GetProfilePath(string profileName)
    {
        string validatedProfileName = ValidateProfileName(profileName);
        return Path.Combine(_profilesDirectoryPath, $"{validatedProfileName}.profile");
    }

    private static string ValidateProfileName(string profileName)
    {
        const string WindowsReservedPathCharacters = "<>:\"/\\|?*";

        ArgumentNullException.ThrowIfNull(profileName);

        if (string.IsNullOrWhiteSpace(profileName))
        {
            throw new ArgumentException("Profile name cannot be empty or whitespace.", nameof(profileName));
        }

        if (profileName == "." || profileName == "..")
        {
            throw new ArgumentException("Profile name cannot be a relative path segment.", nameof(profileName));
        }

        if (Path.IsPathRooted(profileName)
            || profileName.Contains('/')
            || profileName.Contains('\\')
            || profileName.Contains(':'))
        {
            throw new ArgumentException("Profile name cannot contain path separators or rooted paths.", nameof(profileName));
        }

        if (profileName.IndexOfAny(Path.GetInvalidFileNameChars()) >= 0
            || profileName.IndexOfAny(WindowsReservedPathCharacters.ToCharArray()) >= 0)
        {
            throw new ArgumentException("Profile name contains invalid file name characters.", nameof(profileName));
        }

        return profileName;
    }
}

public sealed class ProfileData
{
    public string GamePath { get; set; } = string.Empty;

    public string TrainerPath { get; set; } = string.Empty;

    public string Dll1Path { get; set; } = string.Empty;

    public string Dll2Path { get; set; } = string.Empty;

    public bool LaunchInject1 { get; set; }

    public bool LaunchInject2 { get; set; }

    public string LaunchMethod { get; set; } = string.Empty;

    public bool UseSteamMode { get; set; }

    public string SteamAppId { get; set; } = string.Empty;

    public string SteamCompatDataPath { get; set; } = string.Empty;

    public string SteamProtonPath { get; set; } = string.Empty;
}
