fn main() {
    println!("cargo:rerun-if-changed=src/macos.m");
    println!("cargo:rerun-if-changed=src/windows.cpp");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("src/macos.m")
            .flag("-fobjc-arc")
            .compile("lolping_macos");
        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rustc-link-lib=framework=AVFoundation");
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        cc::Build::new()
            .cpp(true)
            .file("src/windows.cpp")
            .std("c++20")
            .flag_if_supported("/utf-8")
            .compile("lolping_windows");
        for library in ["user32", "shell32", "comctl32", "shcore", "winmm", "dwmapi"] {
            println!("cargo:rustc-link-lib={library}");
        }
    }
}
