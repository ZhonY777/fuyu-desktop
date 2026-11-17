using DeskBox.Platform;

namespace DeskBox;

public partial class App
{
    private readonly TaskCompletionSource<bool> _islandReady =
        new(TaskCreationOptions.RunContinuationsAsynchronously);
    private Thread? _islandThread;

    private void StartIsland()
    {
        if (_islandThread is not null || IsShuttingDown)
        {
            return;
        }

        string islandData = Path.Combine(DeskBox.Services.DeskBoxDataPathService.Current.RootPath, "island");
        Directory.CreateDirectory(islandData);
        Environment.SetEnvironmentVariable("DAOZAI_DATA_DIR", islandData);
        _islandThread = new Thread(() =>
        {
            try
            {
                uint exitCode = IslandNative.Run(HandleIslandAction);
                Log($"[Fuyu] Island runtime stopped with code {exitCode}.");
            }
            catch (Exception ex)
            {
                Log($"[Fuyu] Island startup failed: {ex}");
            }
            finally
            {
                _islandReady.TrySetResult(false);
            }
        })
        {
            IsBackground = true,
            Name = "Fuyu Island UI",
        };
        // Winit registers native OLE drop targets on its window thread.
        _islandThread.SetApartmentState(ApartmentState.STA);
        _islandThread.Start();
    }

    private void HandleIslandAction(uint action)
    {
        if (action == 1)
        {
            Log($"[Fuyu] Island ready in desktop process {Environment.ProcessId}.");
            _islandReady.TrySetResult(true);
            return;
        }

        UiDispatcherQueue.TryEnqueue(() =>
        {
            if (IsShuttingDown)
            {
                return;
            }
            switch (action)
            {
                case 2:
                    SafeFireAndForget(RaiseTrayWidgetsAsync);
                    break;
                case 3:
                    ShowSettings("General");
                    break;
                case 4:
                    OpenSettings();
                    _settingsWindow?.ShowIslandSettings();
                    break;
            }
        });
    }

    internal async Task<bool> EnsureIslandReadyAsync()
    {
        StartIsland();
        if (_islandThread is null)
        {
            return false;
        }
        Task winner = await Task.WhenAny(_islandReady.Task, Task.Delay(TimeSpan.FromSeconds(10)));
        return winner == _islandReady.Task && await _islandReady.Task &&
            _islandThread.IsAlive;
    }

    private async Task<bool> SendIslandCommandAsync(uint command) =>
        await EnsureIslandReadyAsync() && IslandNative.Command(command) == 0;

    private void StopIsland()
    {
        if (_islandThread?.IsAlive == true)
        {
            try
            {
                _ = IslandNative.Command(3);
            }
            catch (Exception ex)
            {
                Log($"[Fuyu] Island shutdown failed: {ex.Message}");
            }
        }
    }
}
