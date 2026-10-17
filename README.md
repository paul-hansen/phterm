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
- Lower typing latency. Typed characters reach the screen sooner: the window no longer
  repaints before the shell has echoed a key (which made the echo wait for the next
  frame), doesn't paint an extra frame every time the cursor moves on Wayland, and the
  `max_fps` and `mux_output_parser_coalesce_delay_ms` defaults are tuned for latency.
- Faster processing of program output: ~2-3x for text-heavy output and full-screen
  programs such as vim, and ~20x for Unicode-heavy output, in
  [vtebench](https://github.com/alacritty/vtebench).
  Rendering no longer blocks the processing of output, plain ASCII text is written to
  the screen in bulk, and full-screen programs such as vim no longer pay for scrollback
  compression on the alternate screen.
- Better notifications from programs such as Claude Code (OSC 9 and OSC 777). Clicking
  one takes you to the pane, tab and window that sent it, and on Linux they expire after
  the system's default duration instead of staying on screen until dismissed. On
  Wayland, raising the window uses xdg-activation, which also makes `window:focus()`
  work there. Messages containing `;` are shown in full instead of being dropped or
  cut short.

See the [changelog](docs/changelog.md) for details. Everything else is upstream
wezterm, so the [upstream docs](https://wezterm.org/) apply and your existing
`~/.wezterm.lua` works unchanged.

## Things to know before switching

- **It replaces upstream wezterm.** The binaries, package name (`wezterm`), app id and
  config location are the same as upstream, so install one or the other, not both.
  Installing this over an upstream install replaces it.
- **Mux client and server must both be from this fork.** The image placeholder change
  bumps the mux protocol version, so this build can't connect to an upstream
  `wezterm-mux-server` (SSH/unix/TLS domains), or the other way round. Install the
  fork on the remote host too.
- **Two defaults differ from upstream.** `max_fps` defaults to `165` (upstream: `60`)
  and `mux_output_parser_coalesce_delay_ms` to `0` (upstream: `3`), to cut typing
  latency. Set them in your config to get upstream's behavior back, e.g. if a
  full-screen program that doesn't use synchronized output shows half-drawn screens.
- **Update notifications come from this fork.** The built-in update checker looks at
  this repo's releases instead of upstream's, and links to the release notes here.
  Builds older than this change still check upstream, so update manually once.
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
