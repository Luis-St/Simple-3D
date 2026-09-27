//! Windows resources: the application icon, compiled into the executable. The `.simple3d` file
//! association points at `simple-3d.exe,0`, since a portable binary has no icon file. No-op elsewhere.

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
            // Not fatal: without the SDK's resource compiler the executable just wears the default icon.
            println!("cargo:warning=could not embed the Windows icon: {error}");
        }
    }
}
