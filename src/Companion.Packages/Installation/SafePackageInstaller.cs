using System.IO.Compression;
using Companion.Packages.Models;
using Companion.Packages.Validation;

namespace Companion.Packages.Installation;

public sealed class SafePackageInstaller(ManifestValidator validator)
{
    public const long MaximumArchiveBytes = 200L * 1024 * 1024;
    public const long MaximumEntryBytes = 100L * 1024 * 1024;
    public const long MaximumExpandedBytes = 220L * 1024 * 1024;

    public PackageManifest Inspect(string archivePath)
    {
        using var archive = ZipFile.OpenRead(archivePath);
        ValidateArchive(archive);
        var manifest = archive.Entries.SingleOrDefault(x =>
            string.Equals(x.FullName, "manifest.json", StringComparison.OrdinalIgnoreCase));
        if (manifest is null || manifest.Length > 1024 * 1024)
        {
            throw new InvalidDataException("ZIP 根目录缺少有效的 manifest.json。");
        }

        using var stream = manifest.Open();
        return validator.Load(stream);
    }

    public async Task<PackageManifest> InstallAsync(
        string archivePath,
        string destinationRoot,
        CancellationToken cancellationToken = default)
    {
        var archiveInfo = new FileInfo(archivePath);
        if (!archiveInfo.Exists || archiveInfo.Length > MaximumArchiveBytes)
        {
            throw new InvalidDataException("扩展包不存在或超过 200 MB。");
        }

        Directory.CreateDirectory(destinationRoot);
        var staging = Path.Combine(destinationRoot, $".staging-{Guid.NewGuid():N}");
        Directory.CreateDirectory(staging);

        try
        {
            using var archive = ZipFile.OpenRead(archivePath);
            ValidateArchive(archive);
            foreach (var entry in archive.Entries)
            {
                cancellationToken.ThrowIfCancellationRequested();
                var target = GetSafeTarget(staging, entry.FullName);
                if (entry.FullName.EndsWith("/", StringComparison.Ordinal))
                {
                    Directory.CreateDirectory(target);
                    continue;
                }

                Directory.CreateDirectory(Path.GetDirectoryName(target)!);
                await using var source = entry.Open();
                await using var output = new FileStream(
                    target,
                    FileMode.CreateNew,
                    FileAccess.Write,
                    FileShare.None,
                    81920,
                    FileOptions.Asynchronous);
                await source.CopyToAsync(output, cancellationToken);
            }

            var validation = validator.ValidateDirectory(staging);
            if (!validation.IsValid)
            {
                throw new InvalidDataException(string.Join(
                    Environment.NewLine,
                    validation.Errors.Select(x => $"{x.Code}: {x.Message} {x.Path}")));
            }

            var manifest = validator.Load(staging);
            var final = Path.Combine(destinationRoot, manifest.Id, manifest.Version);
            if (Directory.Exists(final))
            {
                throw new IOException($"包 {manifest.Id} {manifest.Version} 已安装。");
            }

            Directory.CreateDirectory(Path.GetDirectoryName(final)!);
            Directory.Move(staging, final);
            return manifest;
        }
        catch
        {
            if (Directory.Exists(staging))
            {
                Directory.Delete(staging, recursive: true);
            }

            throw;
        }
    }

    private static void ValidateArchive(ZipArchive archive)
    {
        long expanded = 0;
        foreach (var entry in archive.Entries)
        {
            if (Path.IsPathRooted(entry.FullName) || entry.FullName.Contains('\0'))
            {
                throw new InvalidDataException($"ZIP 包含非法路径：{entry.FullName}");
            }

            if (entry.Length > MaximumEntryBytes)
            {
                throw new InvalidDataException($"ZIP 文件项超过 100 MB：{entry.FullName}");
            }

            expanded = checked(expanded + entry.Length);
            if (expanded > MaximumExpandedBytes)
            {
                throw new InvalidDataException("ZIP 解压总量超过 220 MB。");
            }

            var unixMode = (entry.ExternalAttributes >> 16) & 0xF000;
            if (unixMode == 0xA000)
            {
                throw new InvalidDataException($"ZIP 不允许符号链接：{entry.FullName}");
            }
        }
    }

    private static string GetSafeTarget(string root, string entryName)
    {
        var target = Path.GetFullPath(Path.Combine(root, entryName));
        var prefix = Path.TrimEndingDirectorySeparator(Path.GetFullPath(root)) +
                     Path.DirectorySeparatorChar;
        if (!target.StartsWith(prefix, StringComparison.OrdinalIgnoreCase))
        {
            throw new InvalidDataException($"ZIP 路径穿越：{entryName}");
        }

        return target;
    }
}
