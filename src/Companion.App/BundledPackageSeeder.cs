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

        await CopyEmbeddedResourcesAsync(
            packageResources.Where(n => n.Contains(".characters.", StringComparison.OrdinalIgnoreCase)),
            paths.Characters,
            cancellationToken);
        await CopyEmbeddedResourcesAsync(
            packageResources.Where(n => n.Contains(".actions.", StringComparison.OrdinalIgnoreCase)),
            paths.Actions,
            cancellationToken);
    }

    private async Task CopyEmbeddedResourcesAsync(
        IEnumerable<string> resources,
        string destination,
        CancellationToken cancellationToken)
    {
        var assembly = Assembly.GetExecutingAssembly();
        foreach (var resourceName in resources)
        {
            cancellationToken.ThrowIfCancellationRequested();

            // ResourcePrefix + "characters\shadow-crow-ninja\animations\idle.json"
            var relativePath = resourceName[ResourcePrefix.Length..];

            // Normalise: the LogicalName uses Windows-style backslashes from %(RecursiveDir)
            // which end up embedded literally. Replace with OS-appropriate separator.
            relativePath = relativePath.Replace('\\', Path.DirectorySeparatorChar);

            var target = Path.Combine(destination, relativePath);
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
