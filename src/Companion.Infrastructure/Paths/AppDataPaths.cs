namespace Companion.Infrastructure.Paths;

public sealed class AppDataPaths
{
    public AppDataPaths(string? root = null)
    {
        Root = root ?? Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "OfflineCompanion");
    }

    public string Root { get; }
    public string Data => Path.Combine(Root, "data");
    public string Config => Path.Combine(Root, "config");
    public string Packages => Path.Combine(Root, "packages");
    public string Characters => Path.Combine(Packages, "characters");
    public string Actions => Path.Combine(Packages, "actions");
    public string Cache => Path.Combine(Root, "cache", "atlases");
    public string Logs => Path.Combine(Root, "logs");
    public string Backups => Path.Combine(Root, "backups");
    public string Database => Path.Combine(Data, "companion.db");
    public string Settings => Path.Combine(Config, "settings.json");

    public void EnsureCreated()
    {
        foreach (var path in new[]
                 {
                     Root, Data, Config, Packages, Characters, Actions, Cache, Logs, Backups
                 })
        {
            Directory.CreateDirectory(path);
        }
    }
}
