#define UNICODE
#define _UNICODE
#define NOMINMAX
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <commctrl.h>
#include <dwmapi.h>
#include <shellapi.h>
#include <shellscalingapi.h>
#include <mmsystem.h>

#include <algorithm>
#include <cstdint>
#include <cstring>
#include <deque>
#include <filesystem>
#include <fstream>
#include <memory>
#include <vector>

// All entry points and window callbacks run on GPUI's main thread.
namespace {
constexpr UINT kTrayMessage = WM_APP + 30;
constexpr UINT kConfigureOverlay = WM_APP + 31;
HWND trayWindow = nullptr;
UINT taskbarCreated = 0;
bool enabled = true;
bool trayInstalled = false;
std::deque<int> actions;

NOTIFYICONDATAW trayData() {
    NOTIFYICONDATAW data{};
    data.cbSize = sizeof(data);
    data.hWnd = trayWindow;
    data.uID = 1;
    data.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
    data.uCallbackMessage = kTrayMessage;
    data.hIcon = LoadIconW(nullptr, IDI_APPLICATION);
    lstrcpynW(data.szTip, enabled ? L"lolPing Native · 运行中" : L"lolPing Native · 已暂停", 128);
    return data;
}

void addTray() {
    auto data = trayData();
    trayInstalled = Shell_NotifyIconW(NIM_ADD, &data) != FALSE;
}

LRESULT CALLBACK trayProc(HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam) {
    if (taskbarCreated != 0 && message == taskbarCreated) {
        addTray();
        return 0;
    }
    if (message == kTrayMessage) {
        if (lparam == WM_LBUTTONUP || lparam == WM_LBUTTONDBLCLK) actions.push_back(1);
        if (lparam == WM_RBUTTONUP || lparam == WM_CONTEXTMENU) {
            HMENU menu = CreatePopupMenu();
            AppendMenuW(menu, MF_STRING, 1, L"打开 lolPing 设置");
            AppendMenuW(menu, MF_STRING, 2, enabled ? L"暂停" : L"恢复");
            AppendMenuW(menu, MF_SEPARATOR, 0, nullptr);
            AppendMenuW(menu, MF_STRING, 3, L"退出 lolPing");
            POINT point{};
            GetCursorPos(&point);
            SetForegroundWindow(hwnd);
            int selected = TrackPopupMenu(menu, TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
                point.x, point.y, 0, hwnd, nullptr);
            if (selected != 0) actions.push_back(selected);
            DestroyMenu(menu);
            PostMessageW(hwnd, WM_NULL, 0, 0);
        }
        return 0;
    }
    return DefWindowProcW(hwnd, message, wparam, lparam);
}

LRESULT CALLBACK overlayProc(HWND hwnd, UINT message, WPARAM wparam, LPARAM lparam,
                             UINT_PTR id, DWORD_PTR data) {
    if (message == kConfigureOverlay) {
        const auto* bounds = reinterpret_cast<const RECT*>(data);
        auto style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        style = (style | WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW)
            & ~static_cast<LONG_PTR>(WS_EX_APPWINDOW);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style);
        SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);
        SetWindowPos(hwnd, HWND_TOPMOST, bounds->left, bounds->top,
            bounds->right - bounds->left, bounds->bottom - bounds->top,
            SWP_NOACTIVATE | SWP_FRAMECHANGED | SWP_SHOWWINDOW);
        return 0;
    }
    if (message == WM_NCHITTEST) return HTTRANSPARENT;
    if (message == WM_MOUSEACTIVATE) return MA_NOACTIVATE;
    if (message == WM_NCDESTROY) {
        RemoveWindowSubclass(hwnd, overlayProc, id);
        delete reinterpret_cast<RECT*>(data);
    }
    return DefSubclassProc(hwnd, message, wparam, lparam);
}

BOOL CALLBACK collectMonitor(HMONITOR monitor, HDC, LPRECT, LPARAM data) {
    reinterpret_cast<std::vector<HMONITOR>*>(data)->push_back(monitor);
    return TRUE;
}

// Scale PCM samples per playback, without changing the user's device or session volume.
struct Sound {
    HWAVEOUT device = nullptr;
    WAVEHDR header{};
    std::vector<char> samples;
    ~Sound() {
        if (device) {
            waveOutReset(device);
            if (header.dwFlags & WHDR_PREPARED) waveOutUnprepareHeader(device, &header, sizeof(header));
            waveOutClose(device);
        }
    }
};
std::vector<std::unique_ptr<Sound>> sounds;

void reapSounds() {
    std::erase_if(sounds, [](const auto& sound) { return (sound->header.dwFlags & WHDR_DONE) != 0; });
}
} // namespace

extern "C" {
bool lp_init_status() {
    const auto instance = GetModuleHandleW(nullptr);
    WNDCLASSW klass{};
    klass.lpfnWndProc = trayProc;
    klass.hInstance = instance;
    klass.lpszClassName = L"LolPingNativeTray";
    if (!RegisterClassW(&klass) && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) return false;
    taskbarCreated = RegisterWindowMessageW(L"TaskbarCreated");
    // A hidden top-level window receives Explorer's TaskbarCreated broadcast.
    trayWindow = CreateWindowExW(WS_EX_TOOLWINDOW, klass.lpszClassName, L"lolPing Native",
        WS_POPUP, 0, 0, 0, 0, nullptr, nullptr, instance, nullptr);
    if (!trayWindow) return false;
    addTray();
    return trayInstalled;
}

void lp_shutdown() {
    sounds.clear();
    if (trayWindow) {
        auto data = trayData();
        Shell_NotifyIconW(NIM_DELETE, &data);
        DestroyWindow(trayWindow);
        trayWindow = nullptr;
    }
}

int lp_poll_action() {
    reapSounds();
    if (actions.empty()) return 0;
    int action = actions.front();
    actions.pop_front();
    return action;
}

void lp_status_enabled(bool value) {
    enabled = value;
    auto data = trayData();
    if (trayInstalled) Shell_NotifyIconW(NIM_MODIFY, &data);
}

void lp_configure_settings(void* handle) {
    const BOOL dark = TRUE;
    DwmSetWindowAttribute(static_cast<HWND>(handle), 20 /* immersive dark mode */, &dark, sizeof(dark));
}

void lp_configure_overlay(void* handle, double x, double y, double width, double height) {
    const auto hwnd = static_cast<HWND>(handle);
    auto bounds = std::make_unique<RECT>(RECT{static_cast<LONG>(x), static_cast<LONG>(y),
        static_cast<LONG>(x + width), static_cast<LONG>(y + height)});
    if (SetWindowSubclass(hwnd, overlayProc, 1, reinterpret_cast<DWORD_PTR>(bounds.get()))) {
        bounds.release();
        // Resizing sends synchronous messages. Defer until GPUI releases its App borrow.
        PostMessageW(hwnd, kConfigureOverlay, 0, 0);
    }
}

void lp_hide_window(void* handle) { ShowWindowAsync(static_cast<HWND>(handle), SW_HIDE); }
void lp_show_window(void* handle) { ShowWindowAsync(static_cast<HWND>(handle), SW_RESTORE); }

bool lp_overlay_valid(void* handle) {
    const auto hwnd = static_cast<HWND>(handle);
    const auto style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
    constexpr LONG_PTR required = WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOPMOST;
    return (style & required) == required && GetForegroundWindow() != hwnd && IsWindowVisible(hwnd);
}

void lp_display_bounds(uint32_t display, double* out) {
    std::vector<HMONITOR> monitors;
    // GPUI 0.2.2 assigns display IDs in this same EnumDisplayMonitors order.
    EnumDisplayMonitors(nullptr, nullptr, collectMonitor, reinterpret_cast<LPARAM>(&monitors));
    if (display >= monitors.size()) return;
    MONITORINFO info{};
    info.cbSize = sizeof(info);
    if (!GetMonitorInfoW(monitors[display], &info)) return;
    out[0] = info.rcMonitor.left;
    out[1] = info.rcMonitor.top;
    out[2] = info.rcMonitor.right - info.rcMonitor.left;
    out[3] = info.rcMonitor.bottom - info.rcMonitor.top;
    UINT dx = 96, dy = 96;
    GetDpiForMonitor(monitors[display], MDT_EFFECTIVE_DPI, &dx, &dy);
    out[4] = dx / 96.0;
}

bool lp_play_sound(const wchar_t* path, float volume) {
    reapSounds();
    if (sounds.size() >= 16) sounds.erase(sounds.begin());
    std::ifstream file(std::filesystem::path(path), std::ios::binary);
    char riff[12]{};
    if (!file.read(riff, sizeof(riff)) || std::memcmp(riff, "RIFF", 4) || std::memcmp(riff + 8, "WAVE", 4)) return false;
    WAVEFORMATEX format{};
    auto sound = std::make_unique<Sound>();
    char chunk[4];
    uint32_t length;
    while (file.read(chunk, 4) && file.read(reinterpret_cast<char*>(&length), 4)) {
        if (length > 16 * 1024 * 1024) return false;
        if (std::memcmp(chunk, "fmt ", 4) == 0) {
            if (length < 16) return false;
            if (!file.read(reinterpret_cast<char*>(&format), 16)) return false;
            file.seekg(length - 16, std::ios::cur);
        } else if (std::memcmp(chunk, "data", 4) == 0) {
            sound->samples.resize(length);
            if (!file.read(sound->samples.data(), length)) return false;
        } else {
            file.seekg(length, std::ios::cur);
        }
        if (length & 1) file.seekg(1, std::ios::cur);
    }
    if (format.wFormatTag != WAVE_FORMAT_PCM || format.wBitsPerSample != 16 ||
        format.nChannels == 0 || format.nBlockAlign != format.nChannels * 2 ||
        sound->samples.empty() || sound->samples.size() % format.nBlockAlign != 0) return false;
    for (size_t i = 0; i < sound->samples.size(); i += 2) {
        int16_t sample;
        std::memcpy(&sample, sound->samples.data() + i, 2);
        sample = static_cast<int16_t>(sample * std::clamp(volume, 0.0f, 1.0f));
        std::memcpy(sound->samples.data() + i, &sample, 2);
    }
    if (waveOutOpen(&sound->device, WAVE_MAPPER, &format, 0, 0, CALLBACK_NULL) != MMSYSERR_NOERROR) return false;
    sound->header.lpData = sound->samples.data();
    sound->header.dwBufferLength = static_cast<DWORD>(sound->samples.size());
    if (waveOutPrepareHeader(sound->device, &sound->header, sizeof(WAVEHDR)) != MMSYSERR_NOERROR ||
        waveOutWrite(sound->device, &sound->header, sizeof(WAVEHDR)) != MMSYSERR_NOERROR) return false;
    sounds.push_back(std::move(sound));
    return true;
}
} // extern "C"
