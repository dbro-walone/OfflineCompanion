using System.Text.Json;

namespace Companion.Infrastructure.Config;

public sealed class JsonConfigStore(string path)
{
    private static readonly JsonSerializerOptions Options = new(JsonSerializerDefaults.Web)
    {
        WriteIndented = true
    };

    public async Task<AppSettings> LoadAsync(CancellationToken cancellationToken = default)
    {
        if (!File.Exists(path))
        {
            return new AppSettings();
        }

        try
        {
            await using var stream = File.OpenRead(path);
            return await JsonSerializer.DeserializeAsync<AppSettings>(
                       stream,
                       Options,
                       cancellationToken) ??
                   new AppSettings();
        }
        catch (JsonException)
        {
            var backup = path + ".bak";
            if (!File.Exists(backup))
            {
                throw;
            }

            await using var stream = File.OpenRead(backup);
            return await JsonSerializer.DeserializeAsync<AppSettings>(
                       stream,
                       Options,
                       cancellationToken) ??
                   new AppSettings();
        }
    }

    public async Task SaveAsync(AppSettings settings, CancellationToken cancellationToken = default)
    {
        Directory.CreateDirectory(Path.GetDirectoryName(path)!);
        var temporary = path + $".{Guid.NewGuid():N}.tmp";
        var backup = path + ".bak";
        try
        {
            await using (var stream = new FileStream(
                             temporary,
                             FileMode.CreateNew,
                             FileAccess.Write,
                             FileShare.None,
                             81920,
                             FileOptions.Asynchronous | FileOptions.WriteThrough))
            {
                await JsonSerializer.SerializeAsync(stream, settings, Options, cancellationToken);
                await stream.FlushAsync(cancellationToken);
            }

            if (File.Exists(path))
            {
                File.Copy(path, backup, overwrite: true);
            }

            File.Move(temporary, path, overwrite: true);
        }
        finally
        {
            if (File.Exists(temporary))
            {
                File.Delete(temporary);
            }
        }
    }
}
