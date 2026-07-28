using Companion.Infrastructure.Paths;

namespace Companion.App;

public sealed class BundledPackageSeeder(AppDataPaths paths)
{
    public async Task SeedAsync(CancellationToken cancellationToken = default)
    {
        var source = Path.Combine(AppContext.BaseDirectory, "packages");
        if (!Directory.Exists(source))
        {
            return;
        }

        await CopyTreeAsync(
            Path.Combine(source, "characters"),
            paths.Characters,
            cancellationToken);
        await CopyTreeAsync(
            Path.Combine(source, "actions"),
            paths.Actions,
            cancellationToken);
    }

    private static async Task CopyTreeAsync(
        string source,
        string destination,
        CancellationToken cancellationToken)
    {
        if (!Directory.Exists(source))
        {
            return;
        }

        foreach (var directory in Directory.EnumerateDirectories(source, "*", SearchOption.AllDirectories))
        {
            Directory.CreateDirectory(Path.Combine(destination, Path.GetRelativePath(source, directory)));
        }

        foreach (var file in Directory.EnumerateFiles(source, "*", SearchOption.AllDirectories))
        {
            cancellationToken.ThrowIfCancellationRequested();
            var target = Path.Combine(destination, Path.GetRelativePath(source, file));
            if (File.Exists(target))
            {
                continue;
            }

            Directory.CreateDirectory(Path.GetDirectoryName(target)!);
            await using var input = File.OpenRead(file);
            await using var output = File.Create(target);
            await input.CopyToAsync(output, cancellationToken);
        }
    }
}
