# Windows packaging

`net.simple3d.Simple3D.ico` is the application icon, and the only file here.
`crates/simple3d-app/build.rs` compiles it into `simple-3d.exe` on a Windows
build, which is what gives the executable its icon in Explorer and gives the
`.simple3d` file association something to point at: a portable single binary
has no icon file on disk, so the association names `simple-3d.exe,0` instead.

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

`simple-3d.wxs` is a WiX v4/v5 package that puts the same executable in
`Program Files\Simple 3D` and offers two shortcuts, each its own feature so
either can be turned off in the installer's feature tree before it runs:

- a Start menu entry, and
- a desktop shortcut.

Both point at the installed `simple-3d.exe` and wear the icon above. The
release workflow builds it on the Windows runner, from the very executable it
uploads as the portable artefact, so the two can never be different builds:

```pwsh
dotnet tool install --global wix --version 5.0.2
wix extension add -g WixToolset.UI.wixext/5.0.2
wix build -arch x64 -d Version=1.2.3 -d "ExePath=target\x86_64-pc-windows-msvc\release\simple-3d.exe" `
    -ext WixToolset.UI.wixext packaging/windows/simple-3d.wxs -o dist/simple-3d-windows-x86_64.msi
```

`license.rtf` is the repository's MIT licence in the format the installer's
licence page needs; it is generated from `LICENSE` and says the same thing.

The installer is an addition to the portable executable, never a replacement:
the release still ships `simple-3d-windows-x86_64.exe`, which installs nothing
and writes nothing outside its own directory.

Writing the association itself is the application's own job, from Help ▸
Associate .simple3d files: it writes four keys under
`HKCU\Software\Classes`, so it needs no administrator rights and touches no
machine-wide setting. `crates/simple3d-app/src/file_assoc.rs` documents each
key and has the tests for them.
