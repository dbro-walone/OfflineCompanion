namespace Companion.Packages.Validation;

public sealed record ValidationIssue(string Code, string Message, string? Path = null);

public sealed record PackageValidationResult(
    bool IsValid,
    IReadOnlyList<ValidationIssue> Errors,
    IReadOnlyList<ValidationIssue> Warnings)
{
    public static PackageValidationResult Success(IReadOnlyList<ValidationIssue>? warnings = null) =>
        new(true, [], warnings ?? []);
}
