using System.IO.Compression;
using Companion.Packages.Installation;
using Companion.Packages.Validation;

namespace Companion.IntegrationTests;

public sealed class SafePackageInstallerTests : IDisposable
{
    private readonly string _root = Path.Combine(
        Path.GetTempPath(),
        $"offline-companion-package-tests-{Guid.NewGuid():N}");

    [Fact]
    public async Task InstallerRejectsPathTraversal()
    {
        Directory.CreateDirectory(_root);
        var archivePath = Path.Combine(_root, "evil.zip");
        using (var archive = ZipFile.Open(archivePath, ZipArchiveMode.Create))
        {
            var entry = archive.CreateEntry("../evil.txt");
            await using var stream = new StreamWriter(entry.Open());
            await stream.WriteAsync("blocked");
        }

        var installer = new SafePackageInstaller(new ManifestValidator());

        await Assert.ThrowsAsync<InvalidDataException>(() =>
            installer.InstallAsync(archivePath, Path.Combine(_root, "packages")));
        Assert.False(File.Exists(Path.Combine(_root, "evil.txt")));
    }

    public void Dispose()
    {
        if (Directory.Exists(_root))
        {
            Directory.Delete(_root, recursive: true);
        }
    }
}
