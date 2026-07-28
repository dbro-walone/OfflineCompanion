namespace Companion.Packages.Models;

public abstract record PackageManifest
{
    public required int SchemaVersion { get; init; }
    public required string PackageType { get; init; }
    public required string Id { get; init; }
    public required string Name { get; init; }
    public required string Version { get; init; }
    public required string EngineVersion { get; init; }
}

public sealed record ScaleRange
{
    public double Min { get; init; } = 0.75;
    public double Max { get; init; } = 1.4;
}

public sealed record FrameSize
{
    public int Width { get; init; }
    public int Height { get; init; }
}

public sealed record CharacterManifest : PackageManifest
{
    public required string Author { get; init; }
    public required string License { get; init; }
    public required string Preview { get; init; }
    public required string DefaultAction { get; init; }
    public double DefaultScale { get; init; } = 1;
    public ScaleRange ScaleRange { get; init; } = new();
    public required FrameSize Frame { get; init; }
    public required Dictionary<string, string> Actions { get; init; }
}

public sealed record CharacterCompatibility
{
    public required string Id { get; init; }
    public required string Version { get; init; }
}

public sealed record ActionEntry
{
    public required string Id { get; init; }
    public required string Trigger { get; init; }
    public int Weight { get; init; } = 1;
    public int CooldownSeconds { get; init; }
    public required string Animation { get; init; }
}

public sealed record ActionPackManifest : PackageManifest
{
    public required CharacterCompatibility[] CompatibleCharacters { get; init; }
    public int Priority { get; init; }
    public required ActionEntry[] Actions { get; init; }
}

public enum PlayMode
{
    Once,
    Loop,
    PingPong,
    HoldLast,
    ReverseReturn
}

public sealed record AnimationSegment
{
    public int Start { get; init; }
    public int End { get; init; }
    public int Repeat { get; init; } = 1;
}

public sealed record AnimationSegments
{
    public AnimationSegment? Entry { get; init; }
    public AnimationSegment? Loop { get; init; }
    public AnimationSegment? Exit { get; init; }
}

public sealed record AnimationDefinition
{
    public required string Id { get; init; }
    public required string Atlas { get; init; }
    public int Fps { get; init; } = 30;
    public PlayMode PlayMode { get; init; } = PlayMode.Once;
    public bool Interruptible { get; init; } = true;
    public string ResumePolicy { get; init; } = "restart";
    public bool Mirrorable { get; init; }
    public required AnimationSegments Segments { get; init; }
}
