# iRacing Home Assistant Monitor

Monitors iRacing session state and sends it to Home Assistant via MQTT.

## Requirements

Requires `windres.exe` and `ar.exe` from [mingw-w64](https://www.mingw-w64.org/) to build the Windows resources (only used to set the icon).

## TODO
- [ ] Option to "run on boot"?
- [x] Initialize sim monitor from config file on initial startup
- [ ] Fix tray icon not updating on UserEvent on Linux. `user_event` and `update_session_state` is called, but the icon or menu does not update.
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