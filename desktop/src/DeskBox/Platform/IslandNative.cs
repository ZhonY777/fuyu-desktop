using System.Runtime.InteropServices;

namespace DeskBox.Platform;

internal static class IslandNative
{
    [UnmanagedFunctionPointer(CallingConvention.Winapi)]
    public delegate void NotifyHost(uint action);

    [DllImport("FuyuIsland.dll", EntryPoint = "fuyu_island_run", ExactSpelling = true)]
    [DefaultDllImportSearchPaths(DllImportSearchPath.ApplicationDirectory | DllImportSearchPath.System32)]
    public static extern uint Run(NotifyHost notify);

    [DllImport("FuyuIsland.dll", EntryPoint = "fuyu_island_command", ExactSpelling = true)]
    [DefaultDllImportSearchPaths(DllImportSearchPath.ApplicationDirectory | DllImportSearchPath.System32)]
    public static extern uint Command(uint action);

    [DllImport("FuyuIsland.dll", EntryPoint = "fuyu_island_settings_surface", ExactSpelling = true)]
    [DefaultDllImportSearchPaths(DllImportSearchPath.ApplicationDirectory | DllImportSearchPath.System32)]
    public static extern void SettingsSurface(uint width, uint height, float scale, uint dark);

    [DllImport("FuyuIsland.dll", EntryPoint = "fuyu_island_settings_input", ExactSpelling = true)]
    [DefaultDllImportSearchPaths(DllImportSearchPath.ApplicationDirectory | DllImportSearchPath.System32)]
    public static extern void SettingsInput(uint kind, float x, float y, uint value, uint modifiers);

    [DllImport("FuyuIsland.dll", EntryPoint = "fuyu_island_settings_text", ExactSpelling = true, CharSet = CharSet.Unicode)]
    [DefaultDllImportSearchPaths(DllImportSearchPath.ApplicationDirectory | DllImportSearchPath.System32)]
    public static extern void SettingsText(string text, nuint length);

    [DllImport("FuyuIsland.dll", EntryPoint = "fuyu_island_settings_frame", ExactSpelling = true)]
    [DefaultDllImportSearchPaths(DllImportSearchPath.ApplicationDirectory | DllImportSearchPath.System32)]
    public static extern ulong SettingsFrame([Out] byte[] buffer, nuint length, ulong generation);
}
