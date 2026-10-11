// Capture only lolPing windows from an explicit preview PID (macOS 14+).
// swiftc -parse-as-library docs/capture-product.swift -o /tmp/lolping-capture
// /tmp/lolping-capture <preview PID> docs/images
import AppKit
import ScreenCaptureKit
import Foundation

@main struct Capture {
    @MainActor static func main() async throws {
        _ = NSApplication.shared
        let pid = pid_t(CommandLine.arguments[1])!
        let directory = URL(fileURLWithPath: CommandLine.arguments[2], isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let content = try await SCShareableContent.excludingDesktopWindows(true, onScreenWindowsOnly: true)
        for window in content.windows where window.owningApplication?.processID == pid {
            print("Window: \(window.windowID) \(window.title ?? "") \(window.frame)")
            let settings = (window.title ?? "").contains("设置")
            let config = SCStreamConfiguration()
            config.showsCursor = false
            config.ignoreShadowsSingleWindow = true
            config.captureResolution = .best
            if settings {
                config.width = Int(window.frame.width * 2)
                config.height = Int(window.frame.height * 2)
            } else if window.frame.width > 1100 && window.frame.origin == .zero {
                config.sourceRect = CGRect(x: window.frame.width / 2 - 240, y: window.frame.height / 2 - 240, width: 480, height: 480)
                config.width = 960
                config.height = 960
            } else { continue }
            let image = try await SCScreenshotManager.captureImage(contentFilter: SCContentFilter(desktopIndependentWindow: window), configuration: config)
            let bitmap = NSBitmapImageRep(cgImage: image)
            let path = directory.appendingPathComponent(settings ? "settings.png" : "wheel.png")
            try bitmap.representation(using: .png, properties: [:])!.write(to: path)
            print(path.path)
        }
    }
}
