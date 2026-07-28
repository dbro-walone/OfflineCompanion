using System.Collections.ObjectModel;
using System.Windows;
using Companion.Infrastructure.Paths;
using Companion.Packages.Installation;
using Companion.Packages.Models;
using Companion.Packages.Validation;
using Microsoft.Win32;

namespace Companion.App;

public partial class PackageManagerWindow
{
    private readonly AppDataPaths _paths;
    private readonly ManifestValidator _validator;
    private readonly SafePackageInstaller _installer;
    private readonly ObservableCollection<PackageManifest> _packages = [];

    public PackageManagerWindow(
        AppDataPaths paths,
        ManifestValidator validator,
        SafePackageInstaller installer)
    {
        InitializeComponent();
        _paths = paths;
        _validator = validator;
        _installer = installer;
        PackageList.ItemsSource = _packages;
        Loaded += (_, _) => Refresh();
    }

    private async void ImportPackage(object sender, RoutedEventArgs e)
    {
        var picker = new OpenFileDialog
        {
            Title = "选择角色包或动作包",
            Filter = "ZIP 扩展包 (*.zip)|*.zip",
            CheckFileExists = true,
            Multiselect = false
        };
        if (picker.ShowDialog(this) != true)
        {
            return;
        }

        try
        {
            var manifest = _installer.Inspect(picker.FileName);
            var root = manifest.PackageType == "character" ? _paths.Characters : _paths.Actions;
            await _installer.InstallAsync(picker.FileName, root);
            StatusText.Text = $"已安装：{manifest.Name} {manifest.Version}";
            Refresh();
        }
        catch (Exception ex) when (ex is IOException or InvalidDataException or UnauthorizedAccessException)
        {
            StatusText.Text = $"安装失败：{ex.Message}";
        }
    }

    private void Refresh()
    {
        _packages.Clear();
        foreach (var root in new[] { _paths.Characters, _paths.Actions })
        {
            if (!Directory.Exists(root))
            {
                continue;
            }

            foreach (var manifestPath in Directory.EnumerateFiles(
                         root,
                         "manifest.json",
                         SearchOption.AllDirectories))
            {
                var directory = Path.GetDirectoryName(manifestPath)!;
                var validation = _validator.ValidateDirectory(directory);
                if (validation.IsValid)
                {
                    _packages.Add(_validator.Load(directory));
                }
            }
        }
    }
}
