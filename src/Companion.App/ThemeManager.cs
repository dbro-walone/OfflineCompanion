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
        var resources = Application.Current.Resources;

        SetBrush(resources, "WindowBackground", light ? "#FFF5F6FA" : "#FF1A1D24");
        SetBrush(resources, "PanelBackground", light ? "#FFFFFFFF" : "#FF252830");
        SetBrush(resources, "PanelHoverBackground", light ? "#FFF0F1F5" : "#FF30343E");
        SetBrush(resources, "InputBackground", light ? "#FFFFFFFF" : "#FF20232B");
        SetBrush(resources, "AccentBrush", "#FF6B7DFF");
        SetBrush(resources, "AccentHoverBrush", light ? "#FF596BEA" : "#FF7C8CFF");
        SetBrush(resources, "AccentPressedBrush", "#FF596BEA");
        SetBrush(resources, "TextPrimaryBrush", light ? "#FF1A1D24" : "#FFEAEAF0");
        SetBrush(resources, "TextSecondaryBrush", light ? "#FF667085" : "#FFB0B3BC");
        SetBrush(resources, "ErrorBrush", light ? "#FFD92D20" : "#FFFF6B6B");
        SetBrush(resources, "StrokeBrush", light ? "#FFE0E0E6" : "#FF393D48");
        SetBrush(resources, "ItemHoverBrush", light ? "#99E8E9EF" : "#6630343E");
    }

    public static string Normalize(string? theme) =>
        string.Equals(theme, Light, StringComparison.OrdinalIgnoreCase) ? Light : Dark;

    private static void SetBrush(ResourceDictionary resources, string key, string color) =>
        resources[key] = new SolidColorBrush((Color)ColorConverter.ConvertFromString(color));
}
