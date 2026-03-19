using System.Collections.Generic;
using CrossHookEngine.App.Services;

namespace CrossHookEngine.App.Forms;

internal static class MainFormStartupCoordinator
{
    public static string ResolveAutoLoadProfileName(
        bool autoLoadLastProfile,
        string lastUsedProfile,
        IEnumerable<string> availableProfiles,
        CommandLineOptions options)
    {
        if (!autoLoadLastProfile || string.IsNullOrWhiteSpace(lastUsedProfile))
        {
            return string.Empty;
        }

        if (options is not null && options.ProfilesToLoad.Count > 0)
        {
            return string.Empty;
        }

        foreach (string profileName in availableProfiles)
        {
            if (string.Equals(profileName, lastUsedProfile, System.StringComparison.Ordinal))
            {
                return lastUsedProfile;
            }
        }

        return string.Empty;
    }
}
