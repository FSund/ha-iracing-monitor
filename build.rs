extern crate winresource;

fn main() {
    // Please note: Using #[cfg(target_os = "windows")] in build.rs may not work
    // as expected because build.rs is executed on the host. This means that
    // target_os is always equal to host_os when compiling build.rs. E.g. if we
    // use rustc on Linux and want to cross-compile binaries that run on
    // Windows, target_os in build.rs is "linux". However, CARGO_CFG_TARGET_OS
    // should always be defined and contains the actual target operating system,
    // though it can only be checked at runtime of the build script.
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap() == "windows" {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("resources/icon.ico");
        res.compile().unwrap();
    }
}
