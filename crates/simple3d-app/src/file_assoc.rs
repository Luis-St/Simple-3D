//! Telling the desktop that `.simple3d` files belong to this application, and
//! which icon to draw them with (issue 15).
//!
//! On Linux the `.deb` does this: it ships the desktop entry, the MIME type and
//! the icon, and the desktop's own caches pick them up. A portable executable
//! has none of that, and on Windows there is no package at all -- so the
//! association is something the application offers to write for itself, into
//! the current user's part of the registry. Nothing here needs administrator
//! rights, and nothing here touches a machine-wide setting.
//!
//! The registry is written through `reg.exe`, which is part of every Windows
//! install, rather than through a crate: four keys are not worth a dependency.
//! What each key is for is documented on [`association_commands`], and that
//! function is pure so the arguments can be checked by a test on any platform.

// The command builders are compiled everywhere so their arguments can be
// checked by a test on any platform; only Windows ever runs them, which is why
// nothing else calls them there.
#![cfg_attr(not(windows), allow(dead_code))]

use std::path::Path;

/// The ProgID the file type is registered under. Reverse-domain-free and
/// version-free on purpose: Windows expects a stable, application-specific
/// identifier, and a new version of Simple 3D must claim the same one rather
/// than leave a second entry behind.
pub const PROG_ID: &str = "Simple3D.Project";

pub const EXTENSION: &str = ".simple3d";

/// The `reg.exe` invocations that associate `.simple3d` with `exe`, in order.
///
/// * The extension key names the ProgID: what kind of thing a `.simple3d` file
///   is.
/// * The ProgID's default value is the type's name, which Explorer shows in
///   the Type column.
/// * `DefaultIcon` is the executable's own first icon -- index 0, the one
///   `build.rs` compiled in -- because a portable build has no icon file on
///   disk to point at.
/// * `shell\open\command` passes the path as the first argument, which is
///   exactly what `main` already reads.
pub fn association_commands(exe: &Path) -> Vec<Vec<String>> {
    let exe = exe.display().to_string();
    let root = format!(r"HKCU\Software\Classes\{PROG_ID}");
    vec![
        vec![
            "add".into(),
            format!(r"HKCU\Software\Classes\{EXTENSION}"),
            "/ve".into(),
            "/t".into(),
            "REG_SZ".into(),
            "/d".into(),
            PROG_ID.into(),
            "/f".into(),
        ],
        vec![
            "add".into(),
            root.clone(),
            "/ve".into(),
            "/t".into(),
            "REG_SZ".into(),
            "/d".into(),
            "Simple 3D project".into(),
            "/f".into(),
        ],
        vec![
            "add".into(),
            format!(r"{root}\DefaultIcon"),
            "/ve".into(),
            "/t".into(),
            "REG_SZ".into(),
            "/d".into(),
            format!("{exe},0"),
            "/f".into(),
        ],
        vec![
            "add".into(),
            format!(r"{root}\shell\open\command"),
            "/ve".into(),
            "/t".into(),
            "REG_SZ".into(),
            "/d".into(),
            format!("\"{exe}\" \"%1\""),
            "/f".into(),
        ],
    ]
}

/// The `reg.exe` invocations that take the association back out again. The
/// extension key is deleted as well as the ProgID: leaving `.simple3d` pointing
/// at a ProgID that is gone would leave the files with no application at all
/// rather than with whatever had them before.
pub fn removal_commands() -> Vec<Vec<String>> {
    vec![
        vec!["delete".into(), format!(r"HKCU\Software\Classes\{EXTENSION}"), "/f".into()],
        vec!["delete".into(), format!(r"HKCU\Software\Classes\{PROG_ID}"), "/f".into()],
    ]
}

/// Whether this platform can register the association from inside the
/// application. Everywhere else it is the package's job.
pub const fn supported() -> bool {
    cfg!(windows)
}

/// Run one set of `reg.exe` invocations, stopping at the first that fails.
#[cfg(windows)]
fn run(commands: Vec<Vec<String>>) -> Result<(), String> {
    for arguments in commands {
        let output = std::process::Command::new("reg")
            .args(&arguments)
            .output()
            .map_err(|error| format!("could not run reg.exe: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "reg {} failed: {}",
                arguments.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
    }
    Ok(())
}

/// Associate `.simple3d` files with the running executable.
#[cfg(windows)]
pub fn register() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|error| format!("could not find this executable: {error}"))?;
    run(association_commands(&exe))
}

#[cfg(windows)]
pub fn unregister() -> Result<(), String> {
    run(removal_commands())
}

#[cfg(not(windows))]
pub fn register() -> Result<(), String> {
    Err("On Linux the .deb package registers the file type; a portable build does not.".into())
}

#[cfg(not(windows))]
pub fn unregister() -> Result<(), String> {
    Err("On Linux the .deb package registers the file type; a portable build does not.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_association_names_the_extension_the_icon_and_the_command() {
        // Checked here rather than on Windows, which is what makes these
        // arguments reviewable at all: they are built the same on every
        // platform and only ever *run* on one.
        let commands = association_commands(Path::new(r"C:\Tools\simple-3d.exe"));
        let flat: Vec<String> = commands.iter().map(|c| c.join(" ")).collect();
        assert!(flat[0].contains(r"HKCU\Software\Classes\.simple3d"), "{flat:?}");
        assert!(flat[0].contains("Simple3D.Project"), "{flat:?}");
        assert!(flat.iter().any(|c| c.contains(r"DefaultIcon") && c.contains(r"C:\Tools\simple-3d.exe,0")), "{flat:?}");
        assert!(
            flat.iter().any(|c| c.contains(r"shell\open\command") && c.contains(r#""C:\Tools\simple-3d.exe" "%1""#)),
            "{flat:?}"
        );
        // Every one of them overwrites rather than asking, or the command would
        // block on a prompt nobody can see.
        assert!(commands.iter().all(|c| c.contains(&"/f".to_string())), "{flat:?}");
    }

    #[test]
    fn removing_takes_out_both_keys_it_wrote() {
        let commands = removal_commands();
        let flat: Vec<String> = commands.iter().map(|c| c.join(" ")).collect();
        assert!(flat.iter().any(|c| c.contains(r"Classes\.simple3d")), "{flat:?}");
        assert!(flat.iter().any(|c| c.contains(r"Classes\Simple3D.Project")), "{flat:?}");
        assert!(commands.iter().all(|c| c[0] == "delete"), "{flat:?}");
    }
}
