# iRacing Home Assistant Monitor

Monitors iRacing session state and sends it to Home Assistant via MQTT.

## Home Assistant

Requires **Home Assistant 2025.1 or newer**:

- Entities are registered via [MQTT device-based discovery](https://www.home-assistant.io/integrations/mqtt/#device-discovery-payload) (introduced in HA 2024.6).
- Unavailable telemetry values (e.g. session time remaining in untimed sessions) reset their sensors to `unknown` by publishing a `None` state, which HA supports for non-numeric sensors since 2025.1.

State and configurable telemetry sensors (iRacing telemetry variables, see [config.toml](config.toml)) are published to `iracing/state`; configurable session-info attribute groups are published as retained JSON to `iracing/<group>`.

Availability is split into two retained `online`/`offline` topics: `iracing/availability` (service health, with `offline` set as the MQTT last will) and `iracing/sim_availability` (whether iRacing is running). Data sensors require both to be online; a "Sim running" binary sensor is tied only to service health, so it distinguishes "monitor up, iRacing not running" (off) from "monitor down" (unavailable).

## Requirements

Uses the [winresource](https://crates.io/crates/winresource) crate to set the icon for the executable, which requires `windres.exe` and `ar.exe` from [mingw-w64](https://www.mingw-w64.org/) (this is probably not required when we use the Wix Toolset).

Uses the [Wix Toolset](https://github.com/wixtoolset/) to build the Windows installer via Github Actions.

## TODO
- [x] Add feature `iced_gui` to disable GUI
- [ ] Avoid iced dependencies (`iced_futures`) when feature iced_gui is disabled
- [x] Move winit event loop messaging into main
- [ ] Option to "run on boot" on Windows (use registry)
- [ ] Consider [cargo-bundle](https://crates.io/crates/cargo-bundle/0.6.1) for creating Linux and Windows installers, adding icons etc.
- [x] Initialize sim monitor from config file on initial startup
- [x] Fix tray icon not updating on UserEvent on Linux. `user_event` and `update_session_state` is called, but the icon or menu does not update.
- [x] Use proper location for config file (%APPDATA% on Windows, XDG_CONFIG_... on Linux)
- [x] Fix double tray icons
- [x] Quitting from tray with GUI does not work (Windows)
- [x] Quitting from GUI does throws errors (Windows)
- [x] Figure out where the tray icon went for GUI mode (Windows and Linux)
- [x] Fix sim_monitor stops responding/stream dies if an invalid mqtt config is provided
- [x] Separate backend and frontend
- [x] Fix config update when config file is changed (does not seem to work at the moment, default config is always returned)
- [x] Config file to retain settings between runs
- [ ] Encrypt mqtt password in config file
- [x] Settings pages
- [x] Log to file (only when flag is set?) (--)
- [x] Linux/dev-mode (that doesn't depend on running on Windows)
- [x] Add Windows taskbar icon
- [x] Separate main page and settings page in gui
- [x] Windows installer

## License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.