using Companion.Infrastructure.Paths;

namespace Companion.Infrastructure.Logging;

public sealed class LocalLogService(AppDataPaths paths)
{
    private readonly SemaphoreSlim _gate = new(1, 1);

    public async Task WriteAsync(
        string level,
        string eventId,
        string message,
        Exception? exception = null,
        CancellationToken cancellationToken = default)
    {
        paths.EnsureCreated();
        var file = Path.Combine(paths.Logs, $"{DateTime.Today:yyyy-MM-dd}.log");
        var sanitized = message.ReplaceLineEndings(" ");
        var line = $"{DateTimeOffset.Now:O}\t{level}\t{eventId}\t{sanitized}";
        if (exception is not null)
        {
            line += $"\t{exception.GetType().Name}: {exception.Message.ReplaceLineEndings(" ")}";
        }

        await _gate.WaitAsync(cancellationToken);
        try
        {
            await File.AppendAllTextAsync(file, line + Environment.NewLine, cancellationToken);
        }
        finally
        {
            _gate.Release();
        }
    }
}
