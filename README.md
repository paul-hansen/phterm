# Paul Hansen's terminal

My personal fork of [wezterm](https://wezterm.org/)

I don't plan on upstreaming this, mainly because of effort required and I know Wez has been away from his project for awhile so there's low chance of features being merged. This is more about getting the features/fixes I want added in a way I can reuse them across my machines.

Feel free to upstream anything you want though.

Sponsor Wez at https://wezterm.org/sponsor.html

## Changes

- Kitty protocol image placeholder support. Allows rendering images correctly in Claude Code.
  This implements the [Unicode placeholders](https://sw.kovidgoyal.net/kitty/graphics-protocol/#unicode-placeholders)
  part of the kitty graphics protocol (`U=1` virtual placements), so images are drawn by
  printing placeholder characters and move with the text. This is what makes images work
  in TUIs that use it, inside tmux, and with `kitten icat --unicode-placeholder`.

Everything else is upstream wezterm, so the [upstream docs](https://wezterm.org/) apply
and your existing `~/.wezterm.lua` works unchanged.

## Things to know before switching

- **It replaces upstream wezterm.** The binaries, package name (`wezterm`), app id and
  config location are the same as upstream, so install one or the other, not both.
  Installing this over an upstream install replaces it.
- **Mux client and server must both be from this fork.** The image placeholder change
  bumps the mux protocol version, so this build can't connect to an upstream
  `wezterm-mux-server` (SSH/unix/TLS domains), or the other way round. Install the
  fork on the remote host too.
- **No update notifications.** wezterm's built-in update checker looks at upstream
  releases, not this fork's. To hear about new versions, watch this repo on GitHub
  (Watch → Custom → Releases).
- **Unsigned builds.** The macOS and Windows builds aren't code signed; see the
  install steps below for getting past the OS warnings.
- **Upstream changes are pulled in periodically** by rebasing these changes onto
  upstream wezterm, so releases also include whatever upstream has fixed since.

## Installing

Download from the [latest release](https://github.com/paul-hansen/phterm/releases/latest).
Releases are tagged by build date (`YYYYMMDD-HHMMSS-<commit>`), and every file has a
matching `.sha256` you can check with `sha256sum -c <file>.sha256`.

If you have the [GitHub CLI](https://cli.github.com/), you can download the latest
release's file from the command line instead, e.g.:

```console
$ gh release download -R paul-hansen/phterm -p '*.AppImage'
```

### Linux

There are builds for Ubuntu 22.04 and 26.04. The 22.04 builds link against an older
glibc, so they're the ones to use on other distros or anything older than 26.04.

**AppImage** (any distro, no install):

```console
$ chmod +x WezTerm-*.AppImage
$ ./WezTerm-*.AppImage
```

The AppImage needs FUSE 2 (`libfuse2`, or `fuse2` on Arch). It's built on Ubuntu 26.04,
so it needs a recent glibc; on older systems use the 22.04 `.deb` or `.tar.xz`.

**Debian / Ubuntu** (`.deb`):

```console
$ sudo apt install ./wezterm-*.Ubuntu22.04.deb
```

Use the `Ubuntu26.04` package on Ubuntu 26.04 and newer. Remove with `sudo apt remove wezterm`.

**Other distros** (`.tar.xz`, e.g. Arch, Fedora): the archive contains a
`wezterm/usr/...` tree. Copy it into `/usr/local`:

```console
$ tar -xJf wezterm-*.Ubuntu22.04.tar.xz
$ sudo cp -r wezterm/usr/. /usr/local/
```

The tarball doesn't install dependencies; you need fontconfig, libxkbcommon, OpenSSL 3,
and Wayland/X11 libraries, which most desktop installs already have.

### macOS

The zip contains a universal `WezTerm.app` (Apple Silicon and Intel).

```console
$ unzip WezTerm-macos-*.zip
$ mv WezTerm-macos-*/WezTerm.app /Applications/
$ xattr -dr com.apple.quarantine /Applications/WezTerm.app
```

The `xattr` step is needed because the app isn't signed; without it macOS refuses to
open it (usually saying it's "damaged" or from an unidentified developer). To use the `wezterm` CLI from a shell, add
`/Applications/WezTerm.app/Contents/MacOS` to your `PATH`.

### Windows

- **Installer**: run `WezTerm-*-setup.exe`. Windows SmartScreen will warn that the app
  is unrecognized because it isn't signed; click **More info → Run anyway**.
- **Portable**: extract `WezTerm-windows-*.zip` anywhere and run `wezterm-gui.exe`.

## Building from source

See upstream's [build instructions](https://wezterm.org/install/source.html); nothing
differs for the fork. The `wezterm-*-src.tar.gz` in each release includes the git
submodules.
