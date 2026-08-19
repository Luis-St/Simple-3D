//! Windows resources: the application icon, compiled into the executable.
//!
//! A `.simple3d` file gets the application's icon in Explorer from the icon
//! *inside the executable* -- the file association points at `simple-3d.exe,0`
//! rather than at an icon file, because a portable single binary has no icon
//! file to point at. This is what puts one there.
//!
//! Only ever run when the build host is Windows; on every other platform this
//! is a no-op and `winresource` is not even a dependency.

fn main() {
    println!("cargo:rerun-if-changed=../../packaging/windows/net.simple3d.Simple3D.ico");
    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("../../packaging/windows/net.simple3d.Simple3D.ico");
        resource.set("FileDescription", "Simple 3D");
        resource.set("ProductName", "Simple 3D");
        resource.set("LegalCopyright", "Copyright 2026 Luis Staudt");
        if let Err(error) = resource.compile() {
            // Not fatal: a build without the Windows SDK's resource compiler
            // still produces a working executable, just one wearing the
            // default icon. Saying so beats failing the build.
            println!("cargo:warning=could not embed the Windows icon: {error}");
        }
    }
}
