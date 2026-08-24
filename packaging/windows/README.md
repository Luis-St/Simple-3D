# Windows packaging

`net.simple3d.Simple3D.ico` is the application icon.
`crates/simple3d-app/build.rs` compiles it into `simple-3d.exe` on a Windows
build, which is what gives the executable its icon in Explorer and gives the
`.simple3d` file association something to point at: the package installs no
separate icon file, so the association names `simple-3d.exe,0` instead.

It is generated from the same drawing the Linux package uses, so the two
platforms cannot drift apart:

```sh
for size in 16 24 32 48 64 128 256; do
    rsvg-convert -w $size -h $size packaging/deb/net.simple3d.Simple3D.svg -o /tmp/icon-$size.png
done
python3 -c "
from PIL import Image
sizes = (16, 24, 32, 48, 64, 128, 256)
images = [Image.open(f'/tmp/icon-{s}.png').convert('RGBA') for s in sizes]
images[-1].save('packaging/windows/net.simple3d.Simple3D.ico', sizes=[(s, s) for s in sizes])
"
```

## The installer

`simple-3d.wxs` is a WiX v4/v5 package. It installs the same executable, with
the install directory chosen on the `WixUI_InstallDir` page rather than fixed
at `Program Files` (issue 56), and offers three optional extras as check boxes
on a page of its own -- a feature tree asks "which components do you want",
which is the wrong question to put to somebody installing one application:

- a Start menu entry (`INSTALLSTARTMENUSHORTCUT`),
- a desktop shortcut (`INSTALLDESKTOPSHORTCUT`), and
- the `.simple3d` file association (`ASSOCIATESIMPLE3DFILES`).

Each property defaults to `1` and each is public, so a silent install can turn
any of them off:

```pwsh
msiexec /i simple-3d-windows-x86_64.msi /qn INSTALLDESKTOPSHORTCUT=0 ASSOCIATESIMPLE3DFILES=0
```

The association is four registry values under `HKLM\Software\Classes`, in a
component of their own so uninstalling takes them back out again -- which is
the part the application could never do for itself, and why writing them is the
installer's job now rather than Help ▸ Associate .simple3d files (issue 54).

The shortcuts point at the installed `simple-3d.exe` and wear the icon above.
The release workflow builds the package on the Windows runner, from the very
executable it uploads as the portable artefact, so the two can never be
different builds:

```pwsh
dotnet tool install --global wix --version 5.0.2
wix extension add -g WixToolset.UI.wixext/5.0.2
wix build -arch x64 -d Version=1.2.3 -d "ExePath=target\x86_64-pc-windows-msvc\release\simple-3d.exe" `
    -ext WixToolset.UI.wixext packaging/windows/simple-3d.wxs -o dist/simple-3d-windows-x86_64.msi
```

WiX runs on Windows only. The same command on Linux does get through compiling
and linking this file, which resolves every dialog, component and property it
names, but it rejects `Directory/@Name` there and cannot bind the `.msi` -- so
the package itself only ever comes out of that job.

`license.rtf` is the repository's licence in the format the installer's licence
page needs. `license-rtf.py` generates it from `LICENSE.md`, so the page shown at
install time and the file in the repository cannot say different things:

```sh
python3 packaging/windows/license-rtf.py
```

The installer is an addition to the portable executable, never a replacement:
the release still ships `simple-3d-windows-x86_64.exe`, which installs nothing
and writes nothing outside its own directory -- and so has no file association
either, that being something only an installed copy can put in the registry and
promise to remove again.
