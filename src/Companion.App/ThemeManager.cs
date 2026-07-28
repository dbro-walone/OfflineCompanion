using System.Windows;
using System.Windows.Media;

namespace Companion.App;

public static class ThemeManager
{
    public const string Dark = "dark";
    public const string Light = "light";

    public static void Apply(string? theme)
    {
        var light = string.Equals(theme, Light, StringComparison.OrdinalIgnoreCase);
        var resources = System.Windows.Application.Current.Resources;

        SetBrush(resources, "WindowBackground", light ? "#FFF8F6FA" : "#FF20212B");
        SetBrush(resources, "PanelBackground", light ? "#FFFFFCFF" : "#FF2A2C38");
        SetBrush(resources, "PanelHoverBackground", light ? "#FFF1EDF5" : "#FF353847");
        SetBrush(resources, "InputBackground", light ? "#FFFFFCFF" : "#FF252733");
        SetBrush(resources, "AccentBrush", "#FF8582E8");
        SetBrush(resources, "AccentHoverBrush", light ? "#FF706DD2" : "#FF9B98F3");
        SetBrush(resources, "AccentPressedBrush", "#FF706DD2");
        SetBrush(resources, "TextPrimaryBrush", light ? "#FF272331" : "#FFF0EDF4");
        SetBrush(resources, "TextSecondaryBrush", light ? "#FF706979" : "#FFB8B3C2");
        SetBrush(resources, "ErrorBrush", light ? "#FFD92D20" : "#FFFF6B6B");
        SetBrush(resources, "StrokeBrush", light ? "#FFE5DFE9" : "#FF424554");
        SetBrush(resources, "ItemHoverBrush", light ? "#99EEE9F2" : "#66353847");
    }

    public static string Normalize(string? theme) =>
        string.Equals(theme, Light, StringComparison.OrdinalIgnoreCase) ? Light : Dark;

    private static void SetBrush(ResourceDictionary resources, string key, string color) =>
        resources[key] = new SolidColorBrush((Color)ColorConverter.ConvertFromString(color));
}
