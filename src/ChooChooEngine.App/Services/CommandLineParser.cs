using System;
using System.Collections.Generic;

namespace ChooChooEngine.App.Services
{
    public sealed class CommandLineParser
    {
        public CommandLineOptions Parse(string[] args)
        {
            CommandLineOptions options = new CommandLineOptions();

            if (args == null || args.Length == 0)
            {
                return options;
            }

            for (int i = 0; i < args.Length; i++)
            {
                string arg = args[i];

                if (string.Equals(arg, "-p", StringComparison.OrdinalIgnoreCase) && i + 1 < args.Length)
                {
                    options.ProfilesToLoad.Add(args[++i].Trim('"'));
                    continue;
                }

                if (string.Equals(arg, "-autolaunch", StringComparison.OrdinalIgnoreCase) && i + 1 < args.Length)
                {
                    List<string> commandParts = new List<string>();

                    for (int j = i + 1; j < args.Length; j++)
                    {
                        commandParts.Add(args[j]);
                    }

                    options.AutoLaunchPath = string.Join(" ", commandParts).Trim('"', '\'');
                    options.AutoLaunchRequested = true;
                    break;
                }
            }

            return options;
        }
    }

    public sealed class CommandLineOptions
    {
        public List<string> ProfilesToLoad { get; } = new List<string>();

        public string AutoLaunchPath { get; set; } = string.Empty;

        public bool AutoLaunchRequested { get; set; }
    }
}
