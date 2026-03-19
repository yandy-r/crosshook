namespace ChooChooEngine.App.Tests;

public sealed class TestWorkspace : IDisposable
{
    public TestWorkspace()
    {
        RootPath = Path.Combine(Path.GetTempPath(), "choochoo-loader-tests", Guid.NewGuid().ToString("N"));
    }

    public string RootPath { get; }

    public string GetPath(params string[] segments)
    {
        string path = RootPath;

        foreach (string segment in segments)
        {
            path = Path.Combine(path, segment);
        }

        return path;
    }

    public string CreateFile(params string[] segments)
    {
        string path = GetPath(segments);
        string directoryPath = Path.GetDirectoryName(path) ?? throw new InvalidOperationException("File path must have a directory.");

        Directory.CreateDirectory(directoryPath);
        File.WriteAllText(path, string.Join("/", segments));

        return path;
    }

    public void Dispose()
    {
        if (Directory.Exists(RootPath))
        {
            Directory.Delete(RootPath, recursive: true);
        }
    }
}
