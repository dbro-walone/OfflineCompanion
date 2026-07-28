using Companion.Packages.Models;
using Companion.Packages.Validation;

namespace Companion.IntegrationTests;

public sealed class BundledPackageValidationTests
{
    [Theory]
    [InlineData("characters/shadow-crow-ninja", "character.shadow-crow-ninja")]
    [InlineData("actions/shadow-crow-office", "action.shadow-crow.office")]
    public void BundledPackagePassesStrictValidation(string relativePath, string expectedId)
    {
        var packageRoot = Path.GetFullPath(Path.Combine(
            AppContext.BaseDirectory,
            "..",
            "..",
            "..",
            "..",
            "..",
            "packages",
            relativePath));
        var validator = new ManifestValidator();

        var result = validator.ValidateDirectory(packageRoot);

        Assert.True(
            result.IsValid,
            string.Join(Environment.NewLine, result.Errors.Select(x => $"{x.Code}: {x.Message} {x.Path}")));
        Assert.Equal(expectedId, validator.Load(packageRoot).Id);
    }
}
