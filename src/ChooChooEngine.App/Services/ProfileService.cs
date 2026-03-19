using System;
using System.Collections.Generic;
using System.IO;

namespace ChooChooEngine.App.Services;

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
        ArgumentNullException.ThrowIfNull(profileName);
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
        }
    }

    public ProfileData LoadProfile(string profileName)
    {
        ArgumentNullException.ThrowIfNull(profileName);

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
                    profile.LaunchInject1 = bool.Parse(value);
                    break;

                case "LaunchInject2":
                    profile.LaunchInject2 = bool.Parse(value);
                    break;

                case "LaunchMethod":
                    profile.LaunchMethod = value;
                    break;
            }
        }

        return profile;
    }

    public void DeleteProfile(string profileName)
    {
        ArgumentNullException.ThrowIfNull(profileName);

        string profilePath = GetProfilePath(profileName);

        if (!File.Exists(profilePath))
        {
            throw new FileNotFoundException("Profile file not found.", profilePath);
        }

        File.Delete(profilePath);
    }

    private string GetProfilePath(string profileName)
    {
        return Path.Combine(_profilesDirectoryPath, $"{profileName}.profile");
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
}
