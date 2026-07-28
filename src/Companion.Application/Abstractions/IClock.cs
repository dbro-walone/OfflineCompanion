namespace Companion.Application.Abstractions;

public interface IClock
{
    DateTimeOffset Now { get; }
}
