#[cfg(windows)]
extern crate winres;

#[cfg(windows)]
fn main() {
    // Set the icon for the Windows executable
    let mut res = winres::WindowsResource::new();
    res.set_icon("resources/icon.ico");
    res.compile().unwrap();
}

#[cfg(unix)]
fn main() {}
