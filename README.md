# SK Status Display

SK Status Display is a GPUI dashboard for a monitor or a spare computer. It shows the time, local system metrics, and remote sources that you configure. A fresh profile starts with a solid background and one card named **This machine**.

## Quick start

1. Download the executable for your device from the GitHub releases.
2. Run the executable.
3. If you want more than local metrics, copy [config.example.toml](config.example.toml) to the configuration path for your OS and edit it.

The executable starts fullscreen by default. Pass `--windowed` to start in a window. Pass `--display auto`, `--display primary`, or `--display 2` to select a monitor. The default is `auto`.

## Basic controls

| Key | Action |
| --- | --- |
| F11 | Enter or leave fullscreen. |
| Escape | Close the picker or collapse an expanded card. Otherwise, leave fullscreen. |
| D | Move to the next display. |
| R | Refresh all configured sources. |
| Ctrl+Q or Cmd+Q | Quit. |

## License

This project is licensed under the [GNU Affero General Public License v3.0](LICENSE.md). The bundled IBM Plex fonts are licensed under the SIL Open Font License.
