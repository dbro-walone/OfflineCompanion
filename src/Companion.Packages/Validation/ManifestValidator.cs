using System.Text.Json;
using System.Text.Json.Serialization;
using Companion.Packages.Models;

namespace Companion.Packages.Validation;

public sealed class ManifestValidator
{
    private static readonly HashSet<string> AllowedExtensions =
        new(StringComparer.OrdinalIgnoreCase)
        {
            ".json", ".png", ".webp", ".wav", ".ogg", ".txt"
        };

    private readonly JsonSerializerOptions _jsonOptions = new(JsonSerializerDefaults.Web)
    {
        PropertyNameCaseInsensitive = true,
        ReadCommentHandling = JsonCommentHandling.Disallow,
        UnmappedMemberHandling = JsonUnmappedMemberHandling.Disallow,
        Converters = { new JsonStringEnumConverter(JsonNamingPolicy.KebabCaseLower) }
    };

    public PackageManifest Load(string packageDirectory)
    {
        var manifestPath = Path.Combine(packageDirectory, "manifest.json");
        using var stream = File.OpenRead(manifestPath);
        return Load(stream);
    }

    public PackageManifest Load(Stream stream)
    {
        using var document = JsonDocument.Parse(stream);
        var type = document.RootElement.GetProperty("packageType").GetString();
        var raw = document.RootElement.GetRawText();
        return type switch
        {
            "character" => JsonSerializer.Deserialize<CharacterManifest>(raw, _jsonOptions)
                ?? throw new JsonException("角色 Manifest 为空。"),
            "action" => JsonSerializer.Deserialize<ActionPackManifest>(raw, _jsonOptions)
                ?? throw new JsonException("动作 Manifest 为空。"),
            _ => throw new JsonException($"不支持的 packageType：{type}")
        };
    }

    public PackageValidationResult ValidateDirectory(string packageDirectory)
    {
        var errors = new List<ValidationIssue>();
        var warnings = new List<ValidationIssue>();
        var root = Path.GetFullPath(packageDirectory);

        if (!File.Exists(Path.Combine(root, "manifest.json")))
        {
            errors.Add(new("PKG_MANIFEST_MISSING", "缺少 manifest.json。"));
            return new(false, errors, warnings);
        }

        PackageManifest manifest;
        try
        {
            manifest = Load(root);
        }
        catch (Exception ex) when (ex is IOException or JsonException or UnauthorizedAccessException)
        {
            errors.Add(new("PKG_MANIFEST_INVALID", ex.Message, "manifest.json"));
            return new(false, errors, warnings);
        }

        if (manifest.SchemaVersion != 1)
        {
            errors.Add(new("PKG_SCHEMA_UNSUPPORTED", "仅支持 schemaVersion=1。"));
        }

        if (!IsSafeIdentifier(manifest.Id))
        {
            errors.Add(new("PKG_ID_INVALID", "包 ID 只能包含字母、数字、点、下划线和连字符。"));
        }

        foreach (var file in Directory.EnumerateFiles(root, "*", SearchOption.AllDirectories))
        {
            var relative = Path.GetRelativePath(root, file);
            if (!AllowedExtensions.Contains(Path.GetExtension(file)))
            {
                errors.Add(new("PKG_FILE_TYPE_BLOCKED", "扩展包包含禁止的文件类型。", relative));
            }

            if (!IsInside(root, file))
            {
                errors.Add(new("PKG_PATH_ESCAPE", "资源路径超出包目录。", relative));
            }
        }

        switch (manifest)
        {
            case CharacterManifest character:
                ValidateCharacter(root, character, errors, warnings);
                break;
            case ActionPackManifest actions:
                ValidateActionPack(root, actions, errors);
                break;
        }

        return new(errors.Count == 0, errors, warnings);
    }

    private void ValidateCharacter(
        string root,
        CharacterManifest manifest,
        List<ValidationIssue> errors,
        List<ValidationIssue> warnings)
    {
        if (!manifest.Actions.TryGetValue("idle", out var idlePath))
        {
            errors.Add(new("CHAR_IDLE_MISSING", "角色包必须声明 idle 动作。"));
            return;
        }

        ValidateResource(root, idlePath, errors);
        ValidateResource(root, manifest.Preview, errors);
        foreach (var (actionId, path) in manifest.Actions)
        {
            ValidateResource(root, path, errors, actionId);
        }

        foreach (var suggested in new[] { "clicked", "dragged", "reminder.default", "celebrate" })
        {
            if (!manifest.Actions.ContainsKey(suggested))
            {
                warnings.Add(new("CHAR_ACTION_RECOMMENDED", $"建议提供动作 {suggested}。"));
            }
        }

        if (manifest.Frame.Width <= 0 || manifest.Frame.Height <= 0)
        {
            errors.Add(new("CHAR_FRAME_INVALID", "frame 尺寸必须大于 0。"));
        }

        if (manifest.DefaultScale < manifest.ScaleRange.Min ||
            manifest.DefaultScale > manifest.ScaleRange.Max ||
            manifest.ScaleRange.Min < 0.75 ||
            manifest.ScaleRange.Max > 1.4)
        {
            errors.Add(new("CHAR_SCALE_INVALID", "缩放范围必须位于 0.75～1.40，默认缩放必须在范围内。"));
        }
    }

    private void ValidateActionPack(
        string root,
        ActionPackManifest manifest,
        List<ValidationIssue> errors)
    {
        if (manifest.CompatibleCharacters.Length == 0)
        {
            errors.Add(new("ACTION_COMPATIBILITY_MISSING", "动作包必须声明 compatibleCharacters。"));
        }

        foreach (var action in manifest.Actions)
        {
            ValidateResource(root, action.Animation, errors, action.Id);
            if (action.Weight <= 0)
            {
                errors.Add(new("ACTION_WEIGHT_INVALID", "动作权重必须大于 0。", action.Id));
            }
        }
    }

    private static void ValidateResource(
        string root,
        string relativePath,
        List<ValidationIssue> errors,
        string? context = null)
    {
        if (Path.IsPathRooted(relativePath))
        {
            errors.Add(new("PKG_ABSOLUTE_PATH", "资源路径不能是绝对路径。", context ?? relativePath));
            return;
        }

        var fullPath = Path.GetFullPath(Path.Combine(root, relativePath));
        if (!IsInside(root, fullPath))
        {
            errors.Add(new("PKG_PATH_ESCAPE", "资源路径超出包目录。", context ?? relativePath));
        }
        else if (!File.Exists(fullPath))
        {
            errors.Add(new("PKG_RESOURCE_MISSING", "资源文件不存在。", context ?? relativePath));
        }
    }

    private static bool IsSafeIdentifier(string value) =>
        value.Length is > 0 and <= 128 &&
        value.All(x => char.IsAsciiLetterOrDigit(x) || x is '.' or '_' or '-');

    private static bool IsInside(string root, string path)
    {
        var normalizedRoot = Path.TrimEndingDirectorySeparator(Path.GetFullPath(root)) +
                             Path.DirectorySeparatorChar;
        var normalizedPath = Path.GetFullPath(path);
        return normalizedPath.StartsWith(normalizedRoot, StringComparison.OrdinalIgnoreCase);
    }
}
