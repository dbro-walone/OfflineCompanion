using System.IO;
using System.Windows;
using System.Windows.Controls;
using System.Windows.Media;
using System.Windows.Media.Imaging;
using System.Windows.Threading;

namespace Companion.Presentation.Animation;

public sealed class SpriteAnimationView : Image
{
    private readonly DispatcherTimer _timer;
    private BitmapSource? _atlas;
    private int[] _frames = [0];
    private int _index;

    public SpriteAnimationView()
    {
        Stretch = Stretch.Uniform;
        RenderOptions.SetBitmapScalingMode(this, BitmapScalingMode.HighQuality);
        _timer = new DispatcherTimer(DispatcherPriority.Render);
        _timer.Tick += (_, _) => Advance();
        Loaded += (_, _) => Restart();
        Unloaded += (_, _) => _timer.Stop();
    }

    public static readonly DependencyProperty AtlasPathProperty = DependencyProperty.Register(
        nameof(AtlasPath),
        typeof(string),
        typeof(SpriteAnimationView),
        new PropertyMetadata(null, OnAnimationChanged));

    public static readonly DependencyProperty FrameWidthProperty = DependencyProperty.Register(
        nameof(FrameWidth),
        typeof(int),
        typeof(SpriteAnimationView),
        new PropertyMetadata(384, OnAnimationChanged));

    public static readonly DependencyProperty FrameHeightProperty = DependencyProperty.Register(
        nameof(FrameHeight),
        typeof(int),
        typeof(SpriteAnimationView),
        new PropertyMetadata(512, OnAnimationChanged));

    public static readonly DependencyProperty FramesProperty = DependencyProperty.Register(
        nameof(Frames),
        typeof(string),
        typeof(SpriteAnimationView),
        new PropertyMetadata("0,1,2,3", OnAnimationChanged));

    public static readonly DependencyProperty FpsProperty = DependencyProperty.Register(
        nameof(Fps),
        typeof(int),
        typeof(SpriteAnimationView),
        new PropertyMetadata(8, OnAnimationChanged));

    public string? AtlasPath
    {
        get => (string?)GetValue(AtlasPathProperty);
        set => SetValue(AtlasPathProperty, value);
    }

    public int FrameWidth
    {
        get => (int)GetValue(FrameWidthProperty);
        set => SetValue(FrameWidthProperty, value);
    }

    public int FrameHeight
    {
        get => (int)GetValue(FrameHeightProperty);
        set => SetValue(FrameHeightProperty, value);
    }

    public string Frames
    {
        get => (string)GetValue(FramesProperty);
        set => SetValue(FramesProperty, value);
    }

    public int Fps
    {
        get => (int)GetValue(FpsProperty);
        set => SetValue(FpsProperty, value);
    }

    public void Play(IEnumerable<int> frames, int fps)
    {
        _frames = frames.DefaultIfEmpty(0).ToArray();
        Fps = Math.Clamp(fps, 1, 60);
        _index = 0;
        UpdateFrame();
        _timer.Start();
    }

    private static void OnAnimationChanged(DependencyObject d, DependencyPropertyChangedEventArgs e) =>
        ((SpriteAnimationView)d).Restart();

    private void Restart()
    {
        _timer.Stop();
        _frames = Frames
            .Split(',', StringSplitOptions.TrimEntries | StringSplitOptions.RemoveEmptyEntries)
            .Select(x => int.TryParse(x, out var value) ? value : 0)
            .ToArray();
        if (_frames.Length == 0)
        {
            _frames = [0];
        }

        if (!string.IsNullOrWhiteSpace(AtlasPath) && File.Exists(AtlasPath))
        {
            var bitmap = new BitmapImage();
            bitmap.BeginInit();
            bitmap.CacheOption = BitmapCacheOption.OnLoad;
            bitmap.UriSource = new Uri(Path.GetFullPath(AtlasPath));
            bitmap.EndInit();
            bitmap.Freeze();
            _atlas = bitmap;
        }

        _index = 0;
        _timer.Interval = TimeSpan.FromSeconds(1d / Math.Clamp(Fps, 1, 60));
        UpdateFrame();
        if (IsLoaded)
        {
            _timer.Start();
        }
    }

    private void Advance()
    {
        _index = (_index + 1) % _frames.Length;
        UpdateFrame();
    }

    private void UpdateFrame()
    {
        if (_atlas is null || FrameWidth <= 0 || FrameHeight <= 0)
        {
            Source = null;
            return;
        }

        var columns = Math.Max(1, _atlas.PixelWidth / FrameWidth);
        var frame = Math.Max(0, _frames[_index]);
        var x = frame % columns * FrameWidth;
        var y = frame / columns * FrameHeight;
        if (x + FrameWidth > _atlas.PixelWidth || y + FrameHeight > _atlas.PixelHeight)
        {
            frame = 0;
            x = 0;
            y = 0;
        }

        var cropped = new CroppedBitmap(_atlas, new Int32Rect(x, y, FrameWidth, FrameHeight));
        cropped.Freeze();
        Source = cropped;
    }
}
