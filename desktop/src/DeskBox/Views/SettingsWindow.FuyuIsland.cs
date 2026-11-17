using System.Runtime.InteropServices.WindowsRuntime;
using DeskBox.Platform;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Input;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Automation;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Input;
using Microsoft.UI.Xaml.Media;
using Microsoft.UI.Xaml.Media.Imaging;
using Windows.UI.Core;
using VirtualKey = Windows.System.VirtualKey;

namespace DeskBox.Views;

public sealed partial class SettingsWindow
{
    private const string IslandSettingsSection = "FuyuIsland";
    private NavigationViewItem? _islandEntry;
    private Grid? _islandPane;
    private Image? _islandImage;
    private TextBlock? _islandPlaceholder;
    private TextBox? _islandEditor;
    private DispatcherQueueTimer? _islandTimer;
    private Func<Task<bool>>? _ensureIslandReady;
    private WriteableBitmap? _islandBitmap;
    private byte[] _islandPixels = [];
    private ulong _islandGeneration;
    private bool _islandActive;
    private bool _islandReady;
    private bool _islandComposing;
    private bool _islandPointerDown;
    private uint _islandPointerButton;

    internal void AddIslandSettingsEntry(Func<Task<bool>> ensureIslandReady)
    {
        _ensureIslandReady = ensureIslandReady;
        _islandEntry = new NavigationViewItem
        {
            Content = "灵动岛",
            Tag = IslandSettingsSection,
            Icon = new SymbolIcon(Symbol.View),
        };
        SettingsNavigationView.MenuItems.Insert(1, _islandEntry);
        _islandPane = new Grid
        {
            Background = new SolidColorBrush(Microsoft.UI.Colors.Transparent),
            Visibility = Visibility.Collapsed,
        };
        _islandImage = new Image { Stretch = Stretch.Fill, IsHitTestVisible = false };
        _islandPlaceholder = new TextBlock
        {
            Text = "正在加载灵动岛设置…",
            TextWrapping = TextWrapping.Wrap,
            Margin = new Thickness(22, 16, 22, 16),
        };
        _islandEditor = new TextBox
        {
            MinWidth = 0,
            MinHeight = 0,
            Opacity = 0.01,
            HorizontalAlignment = HorizontalAlignment.Stretch,
            VerticalAlignment = VerticalAlignment.Stretch,
            IsSpellCheckEnabled = false,
            IsTextPredictionEnabled = false,
        };
        AutomationProperties.SetName(_islandEditor, "灵动岛文字输入");
        _islandPane.Children.Add(_islandImage);
        _islandPane.Children.Add(_islandPlaceholder);
        _islandPane.Children.Add(_islandEditor);
        Grid.SetRow(_islandPane, 1);
        SettingsContentRoot.Children.Add(_islandPane);
        BindIslandInput(_islandPane, _islandEditor);
        _islandTimer = SettingsContentRoot.DispatcherQueue.CreateTimer();
        _islandTimer.Interval = TimeSpan.FromMilliseconds(40);
        _islandTimer.Tick += (_, _) => PresentIslandSettings();
    }

    internal void ShowIslandSettings()
    {
        if (_islandEntry is not null)
        {
            SettingsNavigationView.SelectedItem = _islandEntry;
            EnterIslandSettings();
        }
    }

    private async void EnterIslandSettings()
    {
        if (_islandActive || _islandPane is null || _ensureIslandReady is null)
        {
            return;
        }
        _islandActive = true;
        SettingsNavigationView.IsBackEnabled = true;
        SettingsSearchBox.Visibility = Visibility.Collapsed;
        PageScroller.Visibility = Visibility.Collapsed;
        SettingsBreadcrumbHost.Visibility = Visibility.Collapsed;
        _islandPane.Visibility = Visibility.Visible;
        _islandPlaceholder!.Visibility = Visibility.Visible;
        _islandPlaceholder.Text = "正在加载灵动岛设置…";

        try
        {
            _islandReady = await _ensureIslandReady();
            if (_islandReady && _islandActive && !_isClosed)
            {
                _islandTimer!.Start();
                PresentIslandSettings();
            }
            else if (!_islandReady)
            {
                _islandPlaceholder.Text = "灵动岛设置暂时无法加载。请重启浮屿后重试。";
            }
        }
        catch (Exception error)
        {
            App.Log($"[Fuyu] Embedded settings could not open: {error.Message}");
            _islandPlaceholder.Text = "灵动岛设置暂时无法加载。";
        }
    }

    private void PresentIslandSettings()
    {
        if (!_islandActive || !_islandReady || _isClosed || !IsVisibleToUser ||
            _islandPane?.XamlRoot is not { } root ||
            _islandPane.ActualWidth <= 1 || _islandPane.ActualHeight <= 1)
        {
            return;
        }
        double scale = Math.Max(0.75, Math.Min(root.RasterizationScale,
            Math.Sqrt(4_000_000 / (_islandPane.ActualWidth * _islandPane.ActualHeight))));
        int width = Math.Min(4096, (int)Math.Round(_islandPane.ActualWidth * scale));
        int height = Math.Min(4096, (int)Math.Round(_islandPane.ActualHeight * scale));
        if (_islandBitmap?.PixelWidth != width || _islandBitmap.PixelHeight != height)
        {
            _islandBitmap = new WriteableBitmap(width, height);
            _islandPixels = new byte[checked(width * height * 4)];
        }
        IslandNative.SettingsSurface((uint)width, (uint)height, (float)scale,
            _islandPane.ActualTheme == ElementTheme.Dark ? 1u : 0u);
        ulong generation = IslandNative.SettingsFrame(_islandPixels, (nuint)_islandPixels.Length, _islandGeneration);
        if (generation == 0)
        {
            return;
        }
        _islandGeneration = generation;
        using Stream stream = _islandBitmap.PixelBuffer.AsStream();
        stream.Position = 0;
        stream.Write(_islandPixels);
        _islandBitmap.Invalidate();
        _islandImage!.Source = _islandBitmap;
        _islandPlaceholder!.Visibility = Visibility.Collapsed;
    }

    private bool LeaveIslandSettings()
    {
        if (!_islandActive)
        {
            return false;
        }
        _islandActive = false;
        _islandPointerDown = false;
        _islandComposing = false;
        PauseIslandPresenter();
        _islandEditor!.ReleasePointerCaptures();
        _islandPane!.Visibility = Visibility.Collapsed;
        _islandImage!.Source = null;
        _islandEditor!.Text = "";
        SettingsSearchBox.Visibility = Visibility.Visible;
        PageScroller.Visibility = Visibility.Visible;
        return true;
    }

    private void PauseIslandPresenter()
    {
        _islandTimer?.Stop();
        if (_islandReady)
        {
            IslandNative.SettingsSurface(0, 0, 1, 0);
        }
    }

    private void ResumeIslandPresenter()
    {
        if (_islandActive && _islandReady)
        {
            _islandTimer?.Start();
        }
    }

    private static uint IslandModifiers()
    {
        static bool IsDown(VirtualKey key) =>
            InputKeyboardSource.GetKeyStateForCurrentThread(key).HasFlag(CoreVirtualKeyStates.Down);
        return (IsDown(VirtualKey.Shift) ? 1u : 0u) |
            (IsDown(VirtualKey.Control) ? 2u : 0u) |
            (IsDown(VirtualKey.Menu) ? 4u : 0u);
    }

    private void BindIslandInput(Grid pane, TextBox editor)
    {
        void Pointer(uint kind, PointerRoutedEventArgs e, uint value = 0)
        {
            var point = e.GetCurrentPoint(pane).Position;
            IslandNative.SettingsInput(kind, (float)point.X, (float)point.Y, value, IslandModifiers());
            e.Handled = true;
        }
        pane.AddHandler(UIElement.PointerMovedEvent, new PointerEventHandler((_, e) => Pointer(0, e)), true);
        pane.AddHandler(UIElement.PointerPressedEvent, new PointerEventHandler((_, e) =>
        {
            _islandPointerButton = e.GetCurrentPoint(pane).Properties.IsRightButtonPressed ? 1u : 0u;
            _islandPointerDown = true;
            editor.CapturePointer(e.Pointer);
            Pointer(1, e, _islandPointerButton);
        }), true);
        pane.AddHandler(UIElement.PointerReleasedEvent, new PointerEventHandler((_, e) =>
        {
            _islandPointerDown = false;
            Pointer(2, e, _islandPointerButton);
            editor.ReleasePointerCapture(e.Pointer);
            // Wait until WinUI has finished its own pointer focus transfer.
            _ = pane.DispatcherQueue.TryEnqueue(DispatcherQueuePriority.Low, () =>
            {
                if (_islandActive && _islandPane?.Visibility == Visibility.Visible)
                {
                    editor.Focus(FocusState.Keyboard);
                }
            });
        }), true);
        pane.PointerCaptureLost += (_, e) =>
        {
            if (_islandPointerDown)
            {
                _islandPointerDown = false;
                Pointer(2, e, _islandPointerButton);
            }
        };
        pane.PointerExited += (_, _) =>
        {
            if (!_islandPointerDown)
            {
                IslandNative.SettingsInput(5, 0, 0, 0, 0);
            }
        };
        pane.AddHandler(UIElement.PointerWheelChangedEvent, new PointerEventHandler((_, e) =>
        {
            var properties = e.GetCurrentPoint(pane).Properties;
            float delta = properties.MouseWheelDelta;
            IslandNative.SettingsInput(3, properties.IsHorizontalMouseWheel ? delta : 0,
                properties.IsHorizontalMouseWheel ? 0 : delta, 0, IslandModifiers());
            e.Handled = true;
        }), true);
        editor.TextCompositionStarted += (_, _) => _islandComposing = true;
        editor.TextCompositionEnded += (_, _) =>
        {
            _islandComposing = false;
            _ = pane.DispatcherQueue.TryEnqueue(SendIslandText);
        };
        editor.TextChanged += (_, _) => SendIslandText();
        editor.PreviewKeyDown += (_, e) =>
        {
            uint key = (uint)e.Key;
            uint mods = IslandModifiers();
            if (_islandComposing ||
                !(key is 8 or 9 or 13 or 27 or >= 33 and <= 40 or 46 ||
                  ((mods & 2) != 0 && key is 65 or 67 or 86 or 88 or 89 or 90)))
            {
                return;
            }
            IslandNative.SettingsInput(4, 0, 0, key, mods);
            e.Handled = true;
        };
    }

    private void SendIslandText()
    {
        if (!_islandActive || _islandComposing || _islandEditor is not { Text.Length: > 0 } editor)
        {
            return;
        }
        string text = editor.Text;
        editor.Text = "";
        IslandNative.SettingsText(text, (nuint)text.Length);
    }
}
