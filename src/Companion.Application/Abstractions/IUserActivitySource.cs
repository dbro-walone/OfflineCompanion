namespace Companion.Application.Abstractions;

public interface IUserActivitySource
{
    TimeSpan GetIdleDuration();
    bool IsSessionLocked { get; }
}
