using System.Reflection;
using Companion.Infrastructure.Paths;

namespace Companion.App;

public sealed class BundledPackageSeeder(AppDataPaths paths)
{
    private const string ResourcePrefix = "OfflineCompanion.packages.";

    public async Task SeedAsync(CancellationToken cancellationToken = default)
    {
        var assembly = Assembly.GetExecutingAssembly();
        var resourceNames = assembly.GetManifestResourceNames();
        var packageResources = resourceNames
            .Where(n => n.StartsWith(ResourcePrefix, StringComparison.Ordinal))
            .ToList();

        if (packageResources.Count == 0)
        {
            return;
        }

        foreach (var resourceName in packageResources)
        {
            cancellationToken.ThrowIfCancellationRequested();

            // Resource name: "OfflineCompanion.packages.characters\shadow-crow-ninja\animations\idle.json"
            // Strip prefix, normalise separators
            var relativePath = resourceName[ResourcePrefix.Length..]
                .Replace('\\', Path.DirectorySeparatorChar)
                .Replace('/', Path.DirectorySeparatorChar);

            // Route to the correct destination (characters/ or actions/)
            string target;
            if (relativePath.StartsWith("characters" + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase))
            {
                target = Path.Combine(paths.Characters, relativePath["characters".Length..].TrimStart(Path.DirectorySeparatorChar));
            }
            else if (relativePath.StartsWith("actions" + Path.DirectorySeparatorChar, StringComparison.OrdinalIgnoreCase))
            {
                target = Path.Combine(paths.Actions, relativePath["actions".Length..].TrimStart(Path.DirectorySeparatorChar));
            }
            else
            {
                continue;
            }

            if (File.Exists(target))
            {
                continue;
            }

            Directory.CreateDirectory(Path.GetDirectoryName(target)!);
            await using var input = assembly.GetManifestResourceStream(resourceName);
            if (input is null)
            {
                continue;
            }
            await using var output = File.Create(target);
            await input.CopyToAsync(output, cancellationToken);
        }
    }
}
