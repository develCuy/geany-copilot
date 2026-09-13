# Geany Copilot

A native [Geany](https://www.geany.org/) plugin that sends the text around
your cursor (or your current selection) to an LLM backend and inserts the
response back into the document — a lightweight, editor-native AI assistant.

This is `v0.2`, a full rewrite of the plugin in Rust using hand-written FFI
bindings to the Geany/GTK3/GLib C APIs (no maintained `geany-rs` crate exists
on crates.io or GitHub, so the bindings in `src/ffi/` are maintained directly
against the installed Geany headers). It replaces the earlier Lua-script
version of the project (`copilot.lua` / `copywriter.lua`); see [Coming from
the Lua version](#coming-from-the-lua-version) below if you're upgrading
from that.

## Features

- Toolbar button ("Ask Copilot") plus two configurable keybindings (see
  [Usage](#usage))
- A dockable panel (in the right-side sidebar or bottom message window)
  showing the request payload, any error, and — for reasoning models — a live
  "thinking" log, styled to match your editor's color scheme. In the message
  window, a horizontal two-column layout displays the log notebook and
  Clear / Settings buttons in Column 1 (left, default 90% width) and the Ask,
  Stop, Cancel action buttons and status in Column 2 (right); resizing the
  divider is automatically remembered.
  Live token count and tokens/second are shown while a request is running, with
  **Stop** (keep the partial response) and **Cancel** (discard it) buttons
- Two backend types, selectable per preset:
  - **Ollama** (local models)
  - **OpenAI-compatible** APIs (OpenAI, DeepSeek, and other compatible
    providers, including reasoning models that stream `reasoning_content`)
- Multiple named presets, each with its own backend, URL, model, API key,
  system prompt, temperature, and insert mode — add/save/delete presets and
  switch between them from Preferences or the status bar
- A "Select..." button per preset that fetches the backend's live model
  list instead of typing model names by hand
- Three insert modes: insert at cursor, replace the current selection, or
  append after the selection
- Optional automatic language hint (based on the file extension) appended
  to the request so the model knows what language it's looking at
- Configurable request timeout (30s–10min) and max response tokens, both
  also adjustable from the status bar
- Config stored in `GKeyFile` (`.ini`-style) format at
  `~/.config/geany/plugins/geany-copilot/geany-copilot.conf`

## How it works

When triggered, the plugin uses your current selection as context if you
have one; otherwise it grabs roughly 100 characters before and after the
cursor. That context (plus the optional language hint and system prompt) is
sent to the configured backend over `curl`, streamed via SSE.

Note that streaming only drives the **live stats and thinking log** in the
Copilot panel (sidebar or bottom message window) while the request is in
flight — the response itself is inserted into your document once as a whole,
when the request completes (or when you click **Stop**), not incrementally
line-by-line.

Only one request can be in flight at a time; triggering "Ask Copilot" while
a request is already running is a no-op.

## Building

Requires the Geany 2.0 development headers and `libcurl`.

```bash
cargo build --release
```

or, using the provided Makefile:

```bash
make build
```

Run the unit tests with:

```bash
cargo test
```

The suite (65 tests) covers all of the backend logic and config persistence
(100% line coverage of `backend.rs` and `config.rs`), the request worker and
streaming state machine (~87% of `request.rs`, exercised against an
in-process HTTP server), and the null-safety contracts of the UI callbacks.
The GTK widget layer itself needs a running Geany and is not unit-tested
(see [Known limitations](#known-limitations)).

## Installing

```bash
make install
```

This builds the plugin and installs it to
`~/.config/geany/plugins/copilot_plugin.so`. This is the only supported
install location — avoid keeping additional copies elsewhere, since Geany
tracks active plugins by absolute path and a stray duplicate can cause it to
load a stale build.

To remove it:

```bash
make uninstall
```

## Enabling in Geany

1. Restart Geany (or reload plugins)
2. **Tools → Plugin Manager**
3. Check **"Geany Copilot"** in the list
4. An **"Ask Copilot"** button appears in the toolbar

## Configuration

Open **Tools → Preferences → Geany Copilot** (or click the **Settings**
button next to **Clear** in the Copilot panel) to manage presets and options:

- **Add / Save / Delete** a preset
- **Backend type**: Ollama or OpenAI-compatible
- **URL**, **model** (or use **Select...** to fetch the backend's model
  list), **API key**
- **System Prompt**: opens a dedicated multiline editor
- **Temperature** (0.0–2.0, leave blank to let the server decide)
- **Insert mode**: cursor / replace selection / append after selection
- **Include language hint** toggle
- **Thinking log**: toggle enable/disable, with placement option for Sidebar
  or Message Window (bottom panel)

Request timeout and max response tokens are set from the status bar
dropdowns and apply to all presets.

If a preset points at a remote (non-localhost) server, use an `https://`
URL: the API key and the document context around your cursor are sent with
every request, and plain `http://` would expose both on the network.

## Usage

1. (Optional) Assign keybindings: **Edit → Preferences → Keybindings**,
   under the "Geany Copilot" group, bind **"Ask Copilot"** and/or **"Next
   Copilot Preset"** to whatever shortcuts you like — neither has a default.
2. Select the text you want as context, or just place your cursor where you
   want a suggestion.
3. Click the toolbar button (or use your keybinding).
4. Watch progress in the sidebar panel; click **Stop** to keep whatever has
   streamed so far, or **Cancel** to discard the request entirely.
5. On completion, the response is inserted per the active preset's insert
   mode.

## Known limitations

- Pinned to the Geany 2.0 plugin ABI/API this was built against
  (API 247 / ABI `73 << 8`) — it is not guaranteed to load on other Geany
  versions.
- No request queue (see [How it works](#how-it-works)).
- The Geany/GTK/Scintilla integration layer (`config.rs`, `ui.rs`,
  `configure.rs`, and the FFI plumbing in `request.rs`) is raw `unsafe` C
  FFI with global mutable state (e.g. a single `static mut ACTIVE_REQUEST`),
  not safe wrappers. Everything that can run without a live Geany/GTK
  session is unit-tested — backend logic, config load/save (via a fake
  plugin pointing at a temp dir), the streaming worker (via a local HTTP
  server), and the request lifecycle. What remains untested is the code
  that inherently needs a running editor: widget construction in `ui.rs` /
  `configure.rs`, the plugin entry points in `lib.rs`, and the
  Scintilla read/insert calls in `request.rs`.

## Coming from the Lua version

The old `copilot.lua` / `copywriter.lua` scripts (GeanyLua) stored settings
as JSON at `~/.config/geany/plugins/geanylua/geany-copilot/*.json`. This
Rust plugin uses a different config format and path (see
[Configuration](#configuration)) and does **not** read the old JSON files —
you'll need to re-enter your API key and settings once after switching.

## Project layout

```
src/
├── lib.rs          # Entry point: geany_load_module, init, cleanup
├── ffi/
│   ├── mod.rs       # FFI exports
│   ├── geany.rs     # Geany plugin & document API
│   ├── gtk.rs        # GTK3 widgets, dialogs, signals
│   ├── glib.rs       # GLib memory, idle callbacks, GKeyFile, paths
│   └── scintilla.rs  # Scintilla editor message constants & FFI
├── backend.rs       # BackendType enum, BackendPreset, URL & payload generators
├── config.rs        # GKeyFile config loader/saver & preset initialization
├── request.rs       # Request lifecycle & streaming worker thread
├── ui.rs            # Toolbar button, status bar, and sidebar panel
└── configure.rs      # Preferences / Configure dialog logic
```

See `docs/migration.md` for background on the rewrite from the earlier
C++ (`try4`) version.

## License

MIT — see [LICENSE](LICENSE). Interacts with external/local APIs on your
behalf; keep your API keys secure and be aware of any costs your chosen
backend charges.
