# arcadia

ARCropolis's native in-game menus. They run on the game's own scene framework and layout widgets
instead of a web view, so they open in-engine with no browser and no separate process. This crate
replaced the web pages entirely, the old `menus` crate is gone.

## What it shows

- A hub with three buttons: Mod manager, Workspace, Configuration.
- The **mod manager**: the mods under `sd:/ultimate/mods`, scrolled and filtered by category, each
  toggled on or off, with the highlighted mod's `preview.webp` shown and its description in a
  side panel. B saves the preset.
- The **workspace** screen: create, rename, delete and activate workspaces, and open the mod
  manager on one workspace's mods without activating it.
- The **configuration** screen: ARCropolis's flags as ON/OFF rows plus the logging level, which
  opens a small level picker.
- The **update notes**: the changelog after an update, or the release notes of one on offer with
  Update and Later buttons. Its own layout, fixed panes and every texture bundled in the archive
  rather than drawn at run time (built by `tools/changelog_layout.py`, documented in
  `docs/changelog_layout.md` in the scene repo), scrolled by moving its column.

Everything writes through the `config` crate, the same storage the web pages used.

## How it gets on screen

Three hooks (`src/hooks.rs`): one rewrites the main menu's How to Play push (and, at boot, the
title push, keeping the real title behind ours) into our own scene, one registers our scene table
on the menu driver, and one watches the driver's main menu state so the eShop tile can steer it
into the how-to-play state when a request is pending, then hand back exit code 2 on the way out,
which returns to the main menu through the unlock check. The menus open from the eShop tile, from
the How to Play push, or from the `arcrop_show_*` API, which arms a request and opens right away
when the main menu is up, otherwise at the next opportunity.

Holding Plus at boot is written but commented out in `src/lib.rs`, so it does nothing today.

Anything asked for before the game has a menu waits on the title screen push instead. That push is
taken over: the title goes in first and our scene goes in front of it, so the screen shows up first
and the title follows once it closes. Two things use that:

- **After an update.** ARCropolis writes `sd:/ultimate/arcropolis/changelog.toml` when it updates
  itself. The first boot after that reads it, hands the notes to `show_changelog(notes, false)` and
  deletes the file, so the page comes up once with a single OK button.
- **An update on offer.** With auto update on, the updater thread asks GitHub, turns the release
  body into the same notes and calls `show_changelog(notes, true)`, which adds an Update button
  next to Later. It then waits for `take_changelog_choice()` off the loading path. On Update the
  page says the download is running and stays up until the game restarts itself.

## Game addresses

`src/offsets.rs` is a plain table of offsets for whichever game version it was last hardcoded for,
one function per address, written by `tools/patterns.py --hardcode --program <name>` from
`tools/addresses.toml`. Nothing is searched or cached at boot. The only piece of the shared
`patterns` crate still in use is the offset to address helper.

The tree flips between `main_1304` (console) and `main_1305` with that command plus the offsets
patch described in the workspace CLAUDE.md. This doc doesn't say which one is current, check
`src/offsets.rs` itself.

The pattern machinery stays in the tools for when the next game version lands:

- `tools/addresses.toml` lists every address the crate needs and how it is found (`fn` a function,
  `adrp_ldr`/`adrp_add` a global reached through a nearby function). Each entry carries one address
  per game version it's known for: `addr` for 13.0.4, `addr_1305` for 13.0.5, and so on for any
  future `addr_<digits>` key as new versions come along.
- The tool works over one text dump per game version, `target/main_1304.text`,
  `target/main_1305.text`, etc. `tools/patterns.py --dump --program main_1304` (or `main_1305`, or
  whatever a Ghidra program is named) pulls one out of the currently open Ghidra project. Pull every
  version's dump before building, the generator needs them all present to check uniqueness across
  versions.
- `tools/patterns.py --hardcode --program <name>` writes the constants table for that version, the
  form the crate ships with today. Regenerate it and rebuild when the target version changes.
- `tools/patterns.py` (no flags) writes the other form instead, `offsets.rs` as byte patterns
  resolved at boot through `patterns::find_any` and cached to
  `sd:/ultimate/arcropolis/cache/<version>/arcadia_offsets.toml`. For each entry it first tries a
  single pattern, built from the 13.0.4 dump, that's unique in every dump present. When a function's
  bytes changed enough that no single pattern covers every version, it builds one pattern per
  version instead, oldest first, and accepts the list only once a replay of the boot search over
  every dump resolves each version to its own known address. That form was booted on 13.0.4 and
  checks clean on 13.0.5, so it's there to switch back to if hardcoding stops being worth it.
- `--check --program main_1304` (or any other open program) re-runs the patterns in a pattern form
  `offsets.rs` against that version's dump and reports what each entry resolves to. It has nothing
  to check on the hardcoded form.
- Some functions ARCropolis itself hooks (their first 20 bytes are a branch at runtime), so those
  entries carry a `skip` so a pattern starts past the clobbered prologue. `LayoutTextBox::
  SetTextString` is the one today.
- On a new game version, import its `main` into Ghidra, dump it with `--dump --program <name>`, add
  its `addr_<digits>` to every entry in `addresses.toml` (the pattern form plus `--check` is the
  quick way to find most of them, the rest by hand in Ghidra), then `--hardcode --program <name>`
  and rebuild.

## Module map

| Path | What |
|---|---|
| `src/lib.rs` | `install()`, the `Request` a button leaves for the menus to open on, the pending update notes and the answer to an update offer |
| `src/hooks.rs` | the three hooks and the captured menu scene |
| `src/offsets.rs` | generated, every address as a constant for the version it was last hardcoded for |
| `src/game/` | one typed `#[repr(C)]` struct per game class (scene framework, layout, list scroller, button selector, sub menu, header bar, fade, loading, popup, texture, keyboard), each call taking real pointers |
| `src/screen.rs` | the shared screen: layout load, fade, header bar, footer, teardown, used by every screen |
| `src/scenes/` | the screens themselves and the hub state machine (`sequence.rs`) |
| `src/data/` | mods, workspaces, settings over the `config` crate, and the changelog lines |
| `src/preview.rs` | a mod's `preview.webp` decoded to a bntx in memory |
| `src/labels.rs` | the msbt label names the header bar and footer are fed |

## Resources

The release zip comes from `cargo skyline package --no-skyline` at the ARCropolis root, which packs the plugin and the `resources/arcadia` tree into `target/release.zip` at the paths the card expects. Regenerate the offsets for the version you ship before packaging.


The menus need eight files at runtime: the six `layout.arc` archives, `ui/message/msg_menu.xmsbt`
and the `config.json` that declares the new directories to ARCropolis. They ship with ARCropolis in
`resources/arcadia/` and are installed to `sd:/ultimate/arcropolis/resources/` by the
`package-resources` entry in the root `Cargo.toml`. At boot ARCropolis walks that folder as one
extra root on top of `sd:/ultimate/mods`, so the files go through the same handlers a mod's files
would, and they win over a mod shipping the same paths.

`package-resources` only applies to `cargo skyline package`. `cargo skyline install` and the ftp
one liner push the `.nro` alone, so during development copy the folder by hand once, and again
whenever a layout changes.

On console:

```
find resources/arcadia -type f -exec sh -c \
  'curl --ftp-create-dirs -T "$1" "ftp://192.168.129.16:5000/ultimate/arcropolis/resources/${1#resources/arcadia/}"' _ {} \;
```

On the Mac (Ryujinx):

```
rsync -a resources/arcadia/ ~/Library/Application\ Support/Ryujinx/sdcard/ultimate/arcropolis/resources/
```

If any of the eight files is missing, ARCropolis shows a boot dialog naming the folder and every
missing path, skips `install()`, and the game boots without the native menus. With `install()`
skipped the eShop hook now calls the game's own shop code, so the eShop tile behaves like vanilla
instead of doing nothing. The list lives in `REQUIRED_FILES` in `src/lib.rs`, checked by
`missing_resources()`.
