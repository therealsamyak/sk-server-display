# Server status display

Server status display is a full-screen, continuously updating status board. It runs on the physical
monitor of a Windows laptop while Sunshine streams a game from a separate virtual display to
Moonlight. Windows is the priority platform. The same code builds for macOS and Linux.

The display reads live data from your own machines over Tailscale. It only reads. It never changes a
service, a container, or a device.

The app uses [GPUI](https://github.com/zed-industries/zed/tree/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui),
the UI framework of the Zed editor, at Zed commit `6501df15765d66d6c2a2b66b1eb22434ccef3f86`. This
README is also the design document. It states which behavior the GPUI source supports and which
behavior needs a hands-on test.

## Data sources

The display reads these sources. Each one is optional, except the tailnet:

| Panel | Source | How the app reads it | Poll interval |
| --- | --- | --- | --- |
| Host card, this machine | `sysinfo` crate | Local CPU per core, memory, system disk, network rate, temperature, load, and uptime. | 2 s |
| Tailnet | Tailscale | Runs `tailscale status --json` on the display machine. | 10 s |
| Host card, Glances | [Glances](https://glances.readthedocs.io/) | `GET /api/4/{cpu,percpu,mem,load,sensors,fs,uptime,network}` on each host. When Glances does not answer, the card shows the Tailscale presence of the host. | 2 s |
| Status card | Tailscale | The device entry in `tailscale status --json`: online state, last seen, OS, and path. For laptops and other hosts that need no metrics. | 10 s |
| Services | [Uptime Kuma](https://github.com/louislam/uptime-kuma) | `GET /api/status-page/<slug>` and `/api/status-page/heartbeat/<slug>`. These public status page endpoints need no sign-in. | 30 s |
| Disks | [Scrutiny](https://github.com/AnalogJ/scrutiny) | `GET /api/summary`. SMART result, temperature, and power-on hours per drive. | 5 min |

All requests are HTTP `GET` requests or the read-only Tailscale status command. The app stores no
credentials.

## Configure the display

To configure the display, do these steps:

1. Copy `config.example.toml` to the configuration path for your platform:
   - Windows: `%APPDATA%\server-status-display\config.toml`
   - macOS: `~/Library/Application Support/server-status-display/config.toml`
   - Linux: `~/.config/server-status-display/config.toml`
2. Add one `[[host]]` block for each host card, in column order. Set `glances_url` for a host
   that runs Glances. Set `this_machine = true` for metrics of the machine that runs the display. A
   block with only `name` shows a status card: the online state of that device.
3. Replace the example URLs with full Tailscale MagicDNS names of your hosts.
4. Set `wallpaper` to an image file, or remove the line for a solid background.
5. Set `style` to the style made for your wallpaper. See [Styles](#styles).

Without `[[host]]` blocks, the display shows one card for this machine. Without a configuration
file, it shows this machine and the tailnet only, on a solid
background. Git ignores `config.local.toml` in the repository root, so you can keep a private
configuration there and pass it with `--config config.local.toml`.

### Run Glances at sign-in on a Mac

A Mac host card needs Glances on the Mac. A user LaunchAgent starts it at sign-in. macOS lists it
in System Settings > General > Login Items & Extensions, under "Allow in the Background". To set
it up, do these steps on the Mac:

1. Install Glances with its web server:

   ```sh
   uv tool install "glances[web]"
   ```

2. Create `~/.config/glances/glances.conf`. The REST API then answers only requests for these host
   names, which blocks DNS rebinding:

   ```ini
   [outputs]
   webui_allowed_hosts=desk.example.ts.net,desk,100.64.0.2,localhost,127.0.0.1
   ```

3. Create `~/Library/LaunchAgents/local.glances.web.plist`. Replace `you` with your user name and
   `100.64.0.2` with the Tailscale IPv4 address of the Mac (`tailscale ip -4`):

   ```xml
   <?xml version="1.0" encoding="UTF-8"?>
   <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
   <plist version="1.0">
   <dict>
       <key>Label</key>
       <string>local.glances.web</string>
       <key>ProgramArguments</key>
       <array>
           <string>/Users/you/.local/bin/glances</string>
           <string>-w</string>
           <string>--disable-webui</string>
           <string>--bind</string>
           <string>100.64.0.2</string>
           <string>--port</string>
           <string>61208</string>
           <string>-C</string>
           <string>/Users/you/.config/glances/glances.conf</string>
           <string>--disable-plugin</string>
           <string>all</string>
           <string>--enable-plugin</string>
           <string>cpu,percpu,mem,load,sensors,fs,uptime,network</string>
       </array>
       <key>RunAtLoad</key>
       <true/>
       <key>KeepAlive</key>
       <true/>
       <key>ThrottleInterval</key>
       <integer>30</integer>
       <key>ProcessType</key>
       <string>Background</string>
       <key>StandardOutPath</key>
       <string>/Users/you/Library/Logs/glances-web.log</string>
       <key>StandardErrorPath</key>
       <string>/Users/you/Library/Logs/glances-web.log</string>
   </dict>
   </plist>
   ```

4. Load the agent:

   ```sh
   launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/local.glances.web.plist
   ```

Glances listens on the Tailscale address only, so other devices on the local network cannot read
it. It runs only the plugins that the display reads, and not the process list. If Tailscale is not
up at sign-in, the bind fails, and launchd starts Glances again after 30 seconds. To remove the
agent, run `launchctl bootout gui/$(id -u)/local.glances.web` and delete the plist file.

## Run the status display

You need the Rust toolchain in `rust-toolchain.toml`. The first build compiles GPUI from Git and
takes several minutes. On Linux, GPUI needs the libraries in the Zed Linux build guide.

To run the display, use this command from the repository root:

```sh
cargo run --release
```

The command-line options are:

| Option | Default | Effect |
| --- | --- | --- |
| `--config <path>` | platform path | Reads this configuration file. |
| `--display <auto, primary, or N>` | `auto` | Selects the monitor. `auto` prefers the physical display on Windows. |
| `--windowed` | off | Opens a normal window instead of fullscreen. |
| `--allow-focus` | off | Windows only. Lets a click give the status window keyboard focus. |

The keyboard shortcuts work only when the status window has keyboard focus:

| Key | Action |
| --- | --- |
| `F11` | Enter or leave fullscreen. |
| `Esc` | Leave fullscreen. The window stays open on the same display. |
| `D` | Move the window to the next display. |
| `R` | Poll all sources now. |
| `Ctrl+Q` (macOS: `Cmd+Q`) | Quit. |

The window also has `Fullscreen` or `Windowed`, `Next display`, `Refresh`, and `Quit` buttons in the
upper-right corner. The buttons and the display label show for 3 seconds after the pointer moves
over the window. On Windows, these buttons are the main controls, because the window does not take
keyboard focus by default.

## Private data

The configuration file and the screen contents name private hosts and services. Keep them out of
Git:

- Git ignores `config.local.toml`. The committed `config.example.toml` uses `example.ts.net` names.
- The repository has no screenshots of real data. Agent screenshots stay in the Git-ignored
  `.agent-screenshots/` folder.
- Git ignores the `local/` folder. Keep the wallpaper file there or in the configuration folder.

## Design

### Goals

The status display must do these things:

- Fill the physical laptop monitor and update without user action.
- Never take keyboard input away from the game that Sunshine streams.
- Show clearly when data is old or missing. Old numbers must never look current.
- Stay on the physical monitor when the virtual display appears, disappears, or becomes primary.
- Build from one code base for Windows, macOS, and Linux.

### Screen layout

The screen is a wallpaper with a large clock and translucent data panels. The upper part looks like a
lock screen. The panels look like a terminal system monitor, such as btop. The layout fills the
window and scales from a 1600-pixel reference width. The wireframe shows a 1920 × 1080 display with
one Glances host and one alert:

```text
┌────────────────────────────────────────────────────────────────────────────────────────────────┐
│ wallpaper under a 28 % black layer      [Display 1 of 2 …] [Windowed] [Next display] [Refresh] │
│                                         [Quit]  (only for 3 s after the pointer moves)         │
│  15:42           clock, 128 px            ┌ alert, only when needed ─────────────────────────┐ │
│  Monday 23 November                       │ laptop (local) is not responding.                │ │
│                                           │ Data is 12s old. <last error>                    │ │
│                                           └──────────────────────────────────────────────────┘ │
│ ┌ server ─────────── up 6h 41m ┐ ┌ Services ─────── 14/14 up ┐ ┌ Disks ─────────── 2/3 passed ┐ │
│ │ CPU 10%       38°C load 0.44 │ │ ● Service A          100% │ │ ● nvme0  41°C 477 GiB        │ │
│ │ ▁▂▁▃▂▁▂▅▃▂▁▂▁▂▃▂▁  CPU graph │ │ ● Service B          100% │ │ ● sdb  39°C 932 GiB threshold│ │
│ │ 0 ▬────── 9%  1 ▬────── 9%   │ │ ● Service C         99.8% │ └──────────────────────────────┘ │
│ │ 2 ▬────── 6%  3 ▬────── 5%   │ │ ● Service D         99.9% │ ┌ Tailnet ───────── 5/6 online ┐ │
│ │ Net ↓ 1.6 KiB/s ↑ 2.6 KiB/s  │ │ …                         │ │ ● server  linux this machine │ │
│ │ ▁▁▂▁▁ receive  ▔▔▔ send      │ │                           │ │ ● laptop  windows       idle │ │
│ │ Mem  ██████──── 12.6/31.1 GiB│ │                           │ │ ● tablet  android  2d 7h ago │ │
│ │ Disk ███──────── 135/473 GiB │ │                           │ └──────────────────────────────┘ │
│ └──────────────────────────────┘ └───────────────────────────┘                                  │
└────────────────────────────────────────────────────────────────────────────────────────────────┘
```

The host column of the current configuration has two Glances hosts and two laptops. The laptops
show their online state only:

```text
┌ server ──────────────────── up 47m ┐
│ CPU  ▬────────  10%  36°C  ▁▂▁▃▂▁▂ │
│ Mem  ▬▬▬▬─────  16.4 / 31.1 GiB    │
│ Disk ▬▬───────  138 / 473 GiB  ↓ ↑ │
└────────────────────────────────────┘
┌ desk ───────────────────── up 1d 2h ┐
│ CPU  ▬────────   2%        ▁▁▁▁▁▁▁ │
│ Mem  ▬▬▬▬▬────  15.2 / 32.0 GiB    │
│ Disk ▬▬▬▬─────  197 / 460 GiB  ↓ ↑ │
└────────────────────────────────────┘
┌ laptop-1 ──────────────────────────┐
│ ● offline, seen 3d 1h ago    macOS │
└────────────────────────────────────┘
┌ laptop-2 ──────────────────── idle ┐
│ ● online                   windows │
└────────────────────────────────────┘
```

The layout rules are:

- The panels fill the lower 64 % of the window. The clock, the date, and the alerts use the rest.
- A host with only a name gets a status card. It is one row high: a dot, `online` or
  `offline, seen 3d 1h ago`, and the OS. The header shows the Tailscale path when the device is
  online. Status cards keep a fixed height, and metric cards share the rest of the host column.
  The tailnet panel does not list devices that have a host card.
- A Glances host that does not answer also gets a presence card, if Tailscale knows the device. A
  Mac or a laptop that sleeps is a normal state, so the card stays calm: a gray dot,
  `offline, seen 2h 5m ago`, and a gray note. If Tailscale shows the device online, an amber note
  says that Glances does not answer and shows the error. In both cases the feed stays live, so no
  alert shows. The Uptime Kuma panel shows a Glances outage on a server that Kuma monitors.
- One host gets a wide first column. Two hosts get one column each. Three or more hosts share one
  column.
- A host panel shows the core grid only when the panel is tall enough. The CPU and network graphs
  shrink first.
- Each panel header shows one fact when the feed is live: uptime, services up, disks passed, or
  devices online. A feed that is not live shows `connecting`, `stale 12s`, or `offline` there, and
  the panel border changes color.
- The CPU graph shows the last 90 samples, 3 minutes at the 2-second poll. Each bar uses the meter
  colors.
- The network graph shows receive rates above the center line in blue and send rates below it in
  purple. Its scale is the highest rate of the shown samples, and at least 64 KiB/s, so idle traffic
  stays flat. The scale value shows in the upper-left corner of the graph.
- Meters and graph bars are green below 75 %, amber from 75 %, and red from 90 %. Temperature is
  amber from 75 °C and red from 85 °C.
- Services sort problems first: down, pending, unknown, maintenance, then up. A service that is not
  up also shows its state and the time of its last heartbeat.
- Disks show a green dot when Scrutiny reports `passed`. A failed disk also shows `SMART failed`,
  `threshold`, or `failed`.
- Tailnet devices show this machine first, then online devices, then offline devices with the time
  since Tailscale last saw them.

The panels use IBM Plex Mono, so digits stay in their columns as values change. The clock, the
date, and the panel titles use the fonts of the style: IBM Plex Sans, Serif, or Mono.

### Wallpaper

Set `wallpaper` in the configuration file to show an image behind the panels. A relative path
starts from the folder of the configuration file. The app decodes the file once on a background
thread and scales it down to the physical size of the window's display. GPUI's `img` element then
shows it with `ObjectFit::Cover`, so the image fills the window and keeps its aspect ratio
([img.rs#L197-L205](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui/src/elements/img.rs#L197-L205),
[style.rs#L28-L40](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui/src/style.rs#L28-L40)).
The app reads JPEG, PNG, and the other formats that GPUI turns on in the `image` crate.

The repository does not include a wallpaper. The test screenshots use the Windows XP "Bliss" photo
as a placeholder. Microsoft owns that photo, so it stays in the Git-ignored `local/` folder. Replace
it with your own image.

In the `hills` style, a black layer at 28 % opacity covers the wallpaper, so the white clock stays
readable on a bright image. The panels use a dark color at 78 % opacity. The other styles set
their own layers. See [Styles](#styles).

### Styles

A style sets the palette, the fonts, and the place of the clock and the panels for one wallpaper.
The art comes first: the panels go where the art is quiet, and the colors come from the art. Set
the style with the `style` key in the configuration file.

The `hills` style is the default. It puts the clock at the top and all panels in a band at the
bottom, for a landscape photo. The other styles use a side arrangement:

- One column area at the left edge, one at the right edge, or both. The middle of the screen stays
  free for the character.
- Each area holds the host cards, the services, disks, and tailnet panels, or both groups side by
  side. The clock and the alerts stand on top of or under the host column.
- Each panel has a fixed height from its row count. A host card with metrics gives up height when
  its column is full, so the CPU graph gets shorter. Lists do not shrink.
- A shade behind each area fades out toward the middle. It uses one color of the art, so it
  changes the tone only, not the hue.
- In a side area, the host cards show the stacked body: meters above the CPU graph.

The wireframe shows the `yolanda-kokomi` style at 1920 × 1080: the clock and the hosts at the left, the
other panels at the right, and the character free in the middle:

```text
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│ 13:46                                                         ┌ Services ──── 14/14 up ┐ │
│ Tuesday 24 November                                           │ ● Service A       100% │ │
│ ┌ alert, only when needed ──┐                                 │ ● Service B      99.8% │ │
│ └───────────────────────────┘                                 │ …                      │ │
│ ┌ server ───── up 4h 46m ───┐            character            └────────────────────────┘ │
│ │ CPU  ▬─────    10%  41°C  │           (no panels)           ┌ Disks ───── 2/3 passed ┐ │
│ │ Mem  ▬▬▬───  13.1/31.1 GiB│                                 │ ● nvme0  40°C  477 GiB │ │
│ │ Disk ▬▬────   140/473 GiB │                                 └────────────────────────┘ │
│ │ ▁▂▁▃▂▁▂ CPU graph   ↓  ↑  │                                 ┌ Tailnet ──── 1/2 online┐ │
│ └───────────────────────────┘                                 │ ● phone   iOS     idle │ │
│ ┌ laptop ──────────── idle ─┐                                 └────────────────────────┘ │
│ │ ● online          windows │                                                            │
│ └───────────────────────────┘                                                            │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

#### Favorites

These eight styles are on the favorites list. The full tables below give their arrangement and
palette.

| Style | Artist | Wallpaper |
| --- | --- | --- |
| `furina-deep` | Hewaje | [Furina in deep water](https://www.pixiv.net/en/artworks/116931545) |
| `focalors-blossom` | 喵咕君QAQ(KH3) | [Focalors in blue blossoms](https://www.pixiv.net/en/artworks/114423788) |
| `fu-xuan-stars` | DAWN凌晨 | [Fu Xuan among constellations](https://www.pixiv.net/en/artworks/111937693) |
| `yolanda-nahida` | Yolanda | [Nahida in a flower ring](https://www.pixiv.net/en/artworks/124030045) |
| `yolanda-kokomi` | Yolanda | [Kokomi among jellyfish](https://www.pixiv.net/en/artworks/101664845) |
| `trailblazers-orbit` | Minamo | [The Trailblazers on a star chart](https://www.pixiv.net/en/artworks/146476781) |
| `cyrene-bow` | _Chowol | [Cyrene with a bow of light](https://www.pixiv.net/en/artworks/146437680) |
| `castorice-violet` | YonesDraws | [Castorice among violet ribbons](https://x.com/YonesDraws/status/1938192528768905460) |

#### All style drafts

The styles below are drafts, one for each wallpaper. The wallpapers are illustrations by
[Yolanda on pixiv](https://www.pixiv.net/en/users/8349252/illustrations). The artist owns them,
so the repository does not include them or screenshots of them. Download a wallpaper from its
pixiv page into the Git-ignored `local/` folder, then point `wallpaper` at it.

| Style | Wallpaper | Arrangement | Palette and type |
| --- | --- | --- | --- |
| `yolanda-kokomi` | [Kokomi among jellyfish](https://www.pixiv.net/en/artworks/101664845) | Clock and hosts at the left, other panels at the right | Navy glass, aqua and pink, Plex Sans |
| `yolanda-nilou` | [Nilou in a gold mandala](https://www.pixiv.net/en/artworks/102143351) | Other panels at the left, clock and hosts at the right | Ink and gold hairlines, Plex Serif |
| `yolanda-raiden` | [Raiden Shogun and Ei](https://www.pixiv.net/en/artworks/115029674) | Clock and hosts over the starfield, other panels over the city | Violet night, Plex Sans |
| `yolanda-acheron` | [Acheron and Black Swan](https://www.pixiv.net/en/artworks/117657717) | Clock and hosts at the left, other panels at the right | Black and white with one red, square corners, Plex Mono |
| `yolanda-nahida` | [Nahida in a flower ring](https://www.pixiv.net/en/artworks/124030045) | All panels at the left, clock under the hosts | Light: pale leaf cards, forest green type |
| `yolanda-fugue` | [Fugue as a fox spirit](https://www.pixiv.net/en/artworks/125521081) | All panels and the clock at the left. The moon stays free | Warm night, moonlight and fox red |
| `yolanda-aglaea` | [Aglaea in a cathedral](https://www.pixiv.net/en/artworks/126929805) | Clock and hosts at the left, other panels at the right | Candle brown and gold, Plex Serif |
| `yolanda-evernight` | [Evernight among letters](https://www.pixiv.net/en/artworks/136562527) | Clock and hosts at the left, other panels at the right | Light paper cards, dark red type, Plex Serif |
| `yolanda-yaoguang` | [Yaoguang with peacock feathers](https://www.pixiv.net/en/artworks/141114001) | All panels and the clock at the left | Teal glass and pale gold |

The styles below are drafts for wallpapers by other artists. Each artist took part in an official
HoYoverse event on pixiv: HoYoFair, the Honkai: Star Rail "Multiverse Vistas" contests, the "Merrily
on Star Rail" contest, or a Genshin Impact illustration contest. Each wallpaper is from 2023 or
later. Use them in the same way as the Yolanda wallpapers.

| Style | Artist | Wallpaper | Arrangement | Palette and type |
| --- | --- | --- | --- | --- |
| `furina-balloon` | シオザワホウショウ | [Furina in a balloon over Fontaine](https://www.pixiv.net/en/artworks/115029794) | All panels and the clock at the right, over the mist | Light: mist cards, teal type, Plex Serif |
| `rosaria-ruins` | シオザワホウショウ | [Rosaria in flooded ruins](https://www.pixiv.net/en/artworks/115029756) | Clock and hosts at the left, other panels at the right | Slate glass, glyph blue, sash red |
| `lumine-cathedral` | といれっとぺーぱー | [Lumine in a sunlit cathedral](https://www.pixiv.net/en/artworks/104137242) | Clock and hosts at the left, other panels at the right | Umber glass and gold, Plex Serif |
| `ayaka-moonlit` | 仰望天空的樹 | [Ayaka in a moonlit room](https://www.pixiv.net/en/artworks/105339824) | Clock and hosts at the left, other panels at the right | Ink glass, butterfly blue, ribbon red, Plex Serif |
| `keqing-wedding` | ぶし | [Keqing in a wedding dress](https://www.pixiv.net/en/artworks/110502639) | All panels and the clock at the left, over the sky | Light: lilac cards, violet type |
| `bronya-snow` | Fukuro袋子 | [Bronya on a snowy field](https://www.pixiv.net/en/artworks/110744252) | All panels and the clock at the right | Light: ice cards, navy type, coat gold |
| `fu-xuan-stars` | DAWN凌晨 | [Fu Xuan among constellations](https://www.pixiv.net/en/artworks/111937693) | Clock and hosts at the left, other panels at the right | Violet glass over stars, Plex Serif |
| `nahida-forest` | UllV◇ | [Nahida in the Sumeru forest](https://www.pixiv.net/en/artworks/111987496) | All panels and the clock at the left, over the trees | Moss glass, dendro green, sun gold |
| `focalors-blossom` | 喵咕君QAQ(KH3) | [Focalors in blue blossoms](https://www.pixiv.net/en/artworks/114423788) | All panels and the clock at the right | Light: water cards, deep blue type, Plex Serif |
| `kirara-delivery` | キョウ屋斎 | [Kirara on a delivery ride](https://www.pixiv.net/en/artworks/114733430) | All panels and the clock at the left, over the town | Light: cream cards, dusk navy, cat green |
| `furina-sky` | コユコム | [Furina in an open sky](https://www.pixiv.net/en/artworks/121122586) | All panels at the left, clock under them | Navy glass, white type, Plex Serif |
| `ruan-mei-blossom` | コユコム | [Ruan Mei by a plum window](https://www.pixiv.net/en/artworks/114596167) | Clock and hosts at the left, other panels at the right | Light: blush cards, plum type, Plex Serif |
| `mavuika-ride` | xUDe | [Mavuika on a desert ride](https://www.pixiv.net/en/artworks/125448796) | All panels and the clock at the right, under the sky | Light: sand cards, ember and sky teal |
| `castorice-garden` | カルヒKarlHe | [Castorice in a violet garden](https://www.pixiv.net/en/artworks/128606489) | All panels and the clock at the left, over the trunks | Dusk glass, lilac type, Plex Serif |
| `yae-mist` | 大猫 | [Yae Miko in a misty forest](https://www.pixiv.net/en/artworks/139487231) | All panels and the clock at the left, over the mist | Light: paper cards, ink type, shrine red, Plex Serif |
| `shenhe-lanterns` | kirabunni | [Shenhe and Ganyu under lanterns](https://www.pixiv.net/en/artworks/116114165) | Clock and hosts at the left, other panels at the right | Amber glass, lantern gold, Plex Serif |
| `furina-deep` | Hewaje | [Furina in deep water](https://www.pixiv.net/en/artworks/116931545) | Clock and hosts at the left, other panels at the right | Teal glass, caustic aqua |
| `kafka-ruins` | miiio | [Kafka in a ruined city](https://www.pixiv.net/en/artworks/111279076) | Clock and hosts at the left, other panels at the right | Graphite glass, magenta, square corners, Plex Mono |
| `march-shrine` | camu | [March 7th and Stelle at a snowy shrine](https://www.pixiv.net/en/artworks/114753990) | Clock and hosts at the left, other panels at the right | Cocoa glass, lantern amber, March pink, Plex Serif |
| `acheron-rain` | xUDe | [Acheron in red rain](https://www.pixiv.net/en/artworks/117062300) | All panels and the clock at the left | Ash glass, one red, square corners, Plex Mono |
| `odette-stars` | Rurei | [Odette on a starry floor (2026)](https://www.pixiv.net/en/artworks/152255385) | All panels and the clock at the left, over the dark sky | Navy glass, pale blue, Plex Serif |
| `yaoguang-masks` | 787namihira | [Yaoguang among theater masks (2026)](https://www.pixiv.net/en/artworks/146476145) | All panels and the clock at the right, over the masks | Indigo glass, mask orange, square corners |
| `jingliu-moon` | 云开明 | [Jingliu before the full moon](https://www.pixiv.net/en/artworks/109669413) | Clock and hosts at the left, other panels at the right | Navy glass, moon white, Plex Serif |
| `lumine-stars` | 雨壱絵穹 | [Lumine and Aether among constellations](https://www.pixiv.net/en/artworks/107210391) | Clock and hosts at the left, other panels at the right | Night blue glass, star gold, Plex Serif |
| `march-meteor` | GUMi | [March 7th under meteors](https://www.pixiv.net/en/artworks/111548763) | All panels and the clock at the right, over the sky | Plum glass, rose and periwinkle |
| `fu-xuan-orbit` | 于安awa | [Fu Xuan among dark orbits](https://www.pixiv.net/en/artworks/112002007) | Clock and hosts at the left, other panels at the right | Dusk glass, orchid, ring gold, Plex Serif |
| `firefly-meadow` | ChipCheap | [Firefly in a night meadow](https://www.pixiv.net/en/artworks/119048272) | Clock and hosts at the left, other panels at the right | Moss glass, firefly light |
| `mualani-sky` | 碧星 | [Mualani under a Natlan sky](https://www.pixiv.net/en/artworks/122772986) | All panels and the clock at the left, over the sky | Light: sky cards, deep sea type |
| `ayaka-frost` | よる | [Ayaka in frost and butterflies](https://www.pixiv.net/en/artworks/125643452) | All panels and the clock at the right | Light: ice cards, indigo type, Plex Serif |
| `robin-snow` | ziQ | [Robin with a gift in the snow](https://www.pixiv.net/en/artworks/126936381) | All panels and the clock at the left, over the pines | Navy glass, snow white, scarf violet |
| `march-fireworks` | Zakimpo | [March 7th and Stelle at the fireworks](https://www.pixiv.net/en/artworks/126950821) | Clock and hosts at the left, other panels at the right | Warm night glass, spark gold |
| `firefly-sunrise` | Gungo | [Firefly at a city sunrise](https://www.pixiv.net/en/artworks/127013746) | All panels and the clock at the right, over the towers | Amber glass, sun gold, Plex Serif |
| `firefly-dusk` | ビン | [Firefly at violet dusk](https://www.pixiv.net/en/artworks/127067726) | Clock and hosts at the left, other panels at the right | Lilac glass, window gold |
| `castorice-butterflies` | elidrich_lee_ | [Castorice among white butterflies](https://www.pixiv.net/en/artworks/131995733) | Clock and hosts at the left, other panels at the right | Slate glass, wing white, Plex Serif |
| `castorice-ruins` | ようかんもち | [Castorice in moonlit ruins](https://www.pixiv.net/en/artworks/132468617) | All panels and the clock at the left, over the sky | Flat violet glass, square corners, Plex Mono |

The styles below are drafts for wallpapers that won an official HoYoverse art contest. The
Honkai: Star Rail "Multiverse Vistas" creator competition lists its winners on the HoYoverse
results pages for [2024](https://act.hoyoverse.com/puzzle/hkrpg/pz_3z780LOFem/index.html),
[2025](https://act.hoyoverse.com/puzzle/hkrpg/pz_XSyfZGxY8k/index.html), and
[2026](https://act.hoyoverse.com/puzzle/hkrpg/pz_SWdABZWWc2/index.html). Each results page links
the full-size file of each winning work. The pixiv contests list their winners on the
[Genshin Impact](https://www.pixiv.net/contest/genshin2) and
[Honkai: Star Rail](https://www.pixiv.net/contest/houkaistarrail) results pages. For a pixiv
winner, the wallpaper is a later work by the same artist.

| Style | Artist | Award | Wallpaper | Arrangement | Palette and type |
| --- | --- | --- | --- | --- | --- |
| `trailblazers-orbit` | Minamo | Multiverse Vistas 2026, top illustration award | [The Trailblazers on a star chart](https://www.pixiv.net/en/artworks/146476781) | Clock and hosts at the left, other panels at the right | Navy glass, chart white, Plex Sans |
| `cyrene-bow` | _Chowol | Multiverse Vistas 2026, illustration award | [Cyrene with a bow of light](https://www.pixiv.net/en/artworks/146437680) | Clock and hosts at the left, other panels at the right | Plum glass, bow magenta, Plex Serif |
| `pearl-water` | うい みちこ | Multiverse Vistas 2026, illustration award | [Pearl paints with water](https://www.pixiv.net/en/artworks/146822020) | All panels and the clock at the left | Teal glass, water white, Plex Serif |
| `stelle-shards` | えるも | Multiverse Vistas 2026, illustration award | [Stelle among glass shards](https://www.pixiv.net/en/artworks/146397839) | Clock and hosts at the left, other panels at the right | Navy glass, light gold |
| `sparxie-stage` | rdrd_1224 | Multiverse Vistas 2026, illustration award | [Sparxie and Sparkle on stage](https://x.com/rdrd_1224/status/2069740572148162847) | Clock and hosts at the left, other panels at the right | Rose glass, round corners |
| `kafka-city` | PB___q | Multiverse Vistas 2025, illustration award | [Kafka over a city of fireworks](https://x.com/pb___q/status/1938578669301318121) | All panels and the clock at the right | Dark violet glass, neon pink |
| `aglaea-meadow` | e武器 | Multiverse Vistas 2025, illustration award | [Aglaea, Cipher, and Tribbie in a meadow](https://x.com/e_buki818/status/1938472574524309556) | Clock and hosts at the left, other panels at the right | Light: sky cards, navy type, Plex Serif |
| `castorice-violet` | YonesDraws | Multiverse Vistas 2025, illustration award | [Castorice among violet ribbons](https://x.com/YonesDraws/status/1938192528768905460) | Clock and hosts at the left, other panels at the right | Violet glass, lilac type, Plex Serif |
| `cyrene-flowers` | ArtElphe | Multiverse Vistas 2025, illustration award | [Cyrene among flowers in glass jars](https://x.com/ArtElphe/status/1938177165494780390) | Clock and hosts at the left, other panels at the right | Light: blush cards, plum type, Plex Serif |
| `robin-concert` | 에레 | Multiverse Vistas 2024, illustration winner | [Robin sings above a night city](https://x.com/ere_chip/status/1796706911448150321) | All panels and the clock at the right | Indigo glass, note gold, Plex Serif |
| `sparkle-maple` | 0zerofive012 | Multiverse Vistas 2024, illustration winner | [Sparkle among red maple leaves](https://x.com/0zerofive012/status/1792112529495515425) | All panels and the clock at the left | Warm dark glass, maple orange, Plex Serif |
| `acheron-eclipse` | Darius Sim | Multiverse Vistas 2024, illustration winner | Acheron before a black eclipse, on the 2024 results page | Clock and hosts at the left, other panels at the right | Black and white with one red, square corners, Plex Mono |
| `hunters-daylight` | なかゥま/ NAKAuMA | Top prize, pixiv Honkai: Star Rail contest 2023 | [Silver Wolf and Kafka in a pale morning (2024)](https://www.pixiv.net/en/artworks/121547648) | All panels and the clock at the right | Light: lilac cards, navy type |
| `lumine-linen` | きの | Excellence prize, pixiv Genshin Impact contest 2022 | [Lumine and Paimon on white linen (2023)](https://www.pixiv.net/en/artworks/112092132) | All panels and the clock at the right, over the sheet | Light: paper cards, warm ink, Plex Serif |

The `yolanda-evernight`, `ayaka-moonlit`, and `lumine-linen` files have black bars at the top and the
bottom. Crop them before you use them. The display fills the window with `ObjectFit::Cover`, so it
keeps the bars otherwise.

The display draws the wallpaper file as it is. No AI image tool makes or changes an image. A style
is only Rust code: fixed colors, fonts, and panel areas. To make a style for another wallpaper,
copy one constant, pick colors from the art, and set the area widths where the art is quiet.

The styles are in `src/status_style.rs`, `src/status_style_contest.rs`, and
`src/status_style_winners.rs`. Each style is one constant. Remove the styles that you do not keep.

### Visual cleanup

The screen follows the [unslop-ui](https://github.com/felipebasurto/unslop-ui) rules. These items
were removed from the first version:

- The "Server status" title and the summary line that chained counts with middle dots (U3, U6).
- The overall "ALL LIVE" pill and the "5 of 5 sources live" caption. The alerts already name each
  problem source (U9, U14).
- The `LIVE` badge on each panel. Only a feed that is not live shows its state (U9).
- The source name after each panel title, and the OS and CPU model subtitles (U3, U7).
- The "24 h uptime · last check" caption, and the heartbeat age on services that are up (U5, U14).
- The keyboard shortcut footer and the buttons that always showed. This file lists the shortcuts.
  The buttons show when the pointer moves (U4, U5).
- Uppercase labels such as `CPU ×6`, `MEM`, `TEMP`, and `LOAD` (U1).
- Spaced units in compact values. The panels show `41°C`, `6h 41m`, and `2d 7h ago`.
- The `passed` label on each healthy disk. The green dot already shows it (U14).
- The size and the middle dots in the display label (U6).

The remaining dots and state labels render from feed or row state. Spacing uses steps of 4 design
pixels, with 2 and 6 pixels inside graphs and meters. The heaviest font weight is SemiBold. Nothing animates.

The styles keep these rules:

- Each style takes its colors from its wallpaper. The shade behind a panel area goes from one
  color to the same color at zero opacity, so no gradient goes from one hue to another (U17).
- State colors stay state colors. Green, amber, and red show only feed, row, and meter state. The
  `yolanda-acheron` style shows a healthy state in white and keeps red for problems.
- No panel has an accent stripe on one edge (U24). Borders are 1 pixel on all sides.
- Panel titles are short nouns in sentence case, with no uppercase labels (U1).

### Refresh, stale, and offline behavior

Each source is a separate feed with its own poll interval. Polls run on background threads. A
1-second tick starts each poll that is due. Only one poll per feed runs at a time, so a slow source
does not stack requests. The tick does not draw. A panel draws again when its feed has a new poll
result, or when its age text or its freshness changes. The clock draws again once a minute. HTTP requests time out at 75 % of
the poll interval, from 1.5 s to 10 s.

The freshness limits come from the poll interval of each feed:

| Value | Rule | 2 s feed | 30 s feed | 5 min feed |
| --- | --- | --- | --- | --- |
| Stale after | 3 missed polls | 6 s | 90 s | 15 min |
| Offline after | 15 missed polls, and at least 30 s | 30 s | 7.5 min | 75 min |

Each feed state uses the age of its last successful poll:

| Feed state | Condition | Display |
| --- | --- | --- |
| Connecting | No successful poll yet, and the offline limit is not reached. | Blue `connecting` in the panel header. "Waiting for data" in the panel. |
| Live | Last success is newer than the stale limit. | The header fact, such as `14/14 up`. Normal colors. |
| Stale | Last success is between the stale and offline limits. | Amber `stale 12s` and amber border. An amber alert with the data age and the last error. Values and graphs are gray. |
| Offline | Last success is older than the offline limit, or no success before it. | Red `offline`, red border, and a red alert with the retry interval. The panel shows "No data" and the last error. |

Uptime Kuma, Scrutiny, Tailscale, and the presence fallback of a Glances host report their own
state inside a live feed. A down service, a
failed disk check, or an offline device shows in its row. It does not make the feed stale.

The display polls again immediately after the system wakes from sleep. GPUI reports wake events
through `App::on_system_wake`
([app.rs#L1900-L1909](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui/src/app.rs#L1900-L1909)).

GPUI limits the frame rate of an inactive window to about 30 frames per second by default
([platform.rs#L2734-L2738](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui/src/platform.rs#L2734-L2738)).
The status window is inactive while you play. It redraws at most once per second, so this limit
has no visible effect.

### Keyboard focus

Keyboard focus is the main risk in this setup. Sunshine sends remote keyboard and mouse input to the
Windows foreground window. If the status window becomes the foreground window, the game stops
receiving input. The design prevents this in four ways:

1. The window opens with `WindowOptions::focus = false`. On Windows, GPUI then shows the window with
   `SW_SHOWNOACTIVATE`
   ([gpui_windows/src/window.rs#L580-L585](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_windows/src/window.rs#L580-L585)).
2. The app never calls `Window::activate_window` or `App::activate`. On Windows,
   `activate_window` sends a synthetic Alt key press with `SendInput` before `SetForegroundWindow`
   ([gpui_windows/src/window.rs#L817-L868](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_windows/src/window.rs#L817-L868)).
   During a stream, that key press can reach the game.
3. On Windows, the app adds `WS_EX_NOACTIVATE` to the window after GPUI creates it. GPUI has no
   option for this style. The app gets the `HWND` through GPUI's `HasWindowHandle` implementation
   ([window.rs#L8112](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui/src/window.rs#L8112)).
   With this style, a click on the window does not make it the foreground window. Use
   `--allow-focus` to turn this off.
4. When the window has keyboard focus, a short amber alert shows: "Keyboard focus is here" and
   "Click another window to move it." The text names no stream client, so it applies to any setup. The view observes activation changes with `observe_window_activation`.

The trade-off of `WS_EX_NOACTIVATE` is that the keyboard shortcuts do not work in the default Windows
mode. The window buttons work with the mouse. Use them to leave fullscreen, change display, refresh,
or quit.

GPUI handles focus differently on each platform:

| Platform | `focus: false` at open | Source |
| --- | --- | --- |
| Windows | Supported. Shows with `SW_SHOWNOACTIVATE`. | [window.rs#L580-L585](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_windows/src/window.rs#L580-L585) |
| macOS | Supported. Uses `orderFront` instead of `makeKeyAndOrderFront`. | [window.rs#L1370-L1375](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_macos/src/window.rs#L1370-L1375) |
| Linux | Not supported. `WindowParams::focus` is dead code on Linux. The compositor decides. | [platform.rs#L2825-L2829](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui/src/platform.rs#L2825-L2829) |

On macOS, simple fullscreen calls `makeKeyAndOrderFront` when it enters or leaves
([gpui_macos/src/window.rs#L638-L646](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_macos/src/window.rs#L638-L646)).
Native fullscreen moves the window to its own Space
([window.rs#L2031-L2042](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_macos/src/window.rs#L2031-L2042)).
Both can take focus on macOS. The focus alert shows when this happens.

### Display selection

The `--display` option selects the monitor:

| Selector | Windows | macOS | Linux |
| --- | --- | --- | --- |
| `auto` (default) | The physical display. See the ranking below. | The primary display. | The primary display. Wayland reports none, so the first display. |
| `primary` | The display at the origin `(0, 0)`. | The display with the menu bar. | X11: the default X screen. Wayland: the first display. |
| `N` | The Nth display from left to right, then top to bottom. | Same. | Same. |

Windows `auto` does not use the Windows primary display. A virtual display setup can make the
virtual display primary while a stream runs. GPUI's Windows primary display is the monitor at point
`(0, 0)`
([gpui_windows/src/display.rs#L83-L96](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_windows/src/display.rs#L83-L96)),
so it follows that change.

GPUI does not report the connector type or whether a display is virtual. The app reads this
information directly from Windows with `QueryDisplayConfig` and `DisplayConfigGetDeviceInfo`. Then
it ranks the active displays:

1. Built-in panel: internal, embedded DisplayPort, or embedded UDI output. The primary display wins a
   tie.
2. Primary external monitor on a physical connector, such as HDMI or DisplayPort.
3. Other external monitor.
4. Indirect wired display, such as a USB display adapter.
5. Never: indirect virtual output or unknown output. Virtual display drivers use these types.

The app maps the chosen Windows display to a GPUI display with the GDI device name, such as
`\\.\DISPLAY1`, and the `HMONITOR` handle. GPUI on Windows uses the `HMONITOR` value as the
`DisplayId`
([gpui_windows/src/display.rs#L79-L81](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_windows/src/display.rs#L79-L81)).
This mapping is a GPUI implementation detail, not a public contract. If a GPUI update changes it,
`auto` uses the primary display, and the display label shows "primary".

The display label shows the number and the kind of the chosen display, such as
`Display 1 of 2 (built-in panel)`. It shows next to the
window buttons after the pointer moves. Use this label to confirm the choice.

### Fullscreen and exit

The window opens with `WindowBounds::Fullscreen`. GPUI then calls the platform fullscreen toggle
after it creates the window
([window.rs#L1755-L1759](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui/src/window.rs#L1755-L1759)).
Each platform uses a different fullscreen type:

| Platform | Fullscreen type | Source |
| --- | --- | --- |
| Windows | Borderless window. GPUI removes the frame and sizes the window to the physical bounds of the display. It uses `SWP_NOACTIVATE`, so the toggle does not take focus. | [window.rs#L306-L366](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_windows/src/window.rs#L306-L366) |
| macOS | Native fullscreen in a separate Space. `toggle_simple_fullscreen` is also available. | [window.rs#L2031-L2042](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_macos/src/window.rs#L2031-L2042) |
| Linux Wayland | `xdg_toplevel.set_fullscreen(None)`. The compositor selects the output. | [wayland/window.rs#L1984-L1993](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_linux/src/linux/wayland/window.rs#L1984-L1993) |
| Linux X11 | `_NET_WM_STATE_FULLSCREEN`. The window manager selects the monitor. | [x11/window.rs#L1716-L1725](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_linux/src/linux/x11/window.rs#L1716-L1725) |

To leave fullscreen, do one of these things:

- Move the pointer over the window, then click `Windowed`. This works in every mode.
- Press `Esc` or `F11` when the window has keyboard focus.

Leaving fullscreen does not quit the app. The window becomes a normal window at 80 % of the display
size. To quit, click `Quit`, press `Ctrl+Q` (macOS: `Cmd+Q`), or close the window.

### Monitor changes

The app subscribes to `App::observe_displays`. GPUI reports `Added`, `Removed`, and `Changed` events
([platform.rs#L123-L141](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui/src/platform.rs#L123-L141)).
The platform calls can be spurious
([platform.rs#L424-L428](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui/src/platform.rs#L424-L428)).

When a display event arrives, the app does these steps:

1. Waits 750 ms for more events. A Sunshine stream start can add a display, change the primary
   display, and change resolution in one burst.
2. Runs the display selection again.
3. Stops if the chosen display ID and bounds are the same as before.
4. Otherwise, reads the fullscreen state of the old window and opens a new window on the chosen
   display with the same state.
5. Closes the old window.

The app reopens the window instead of moving it. On Windows, GPUI stores the display bounds when the
window enters fullscreen
([window.rs#L342-L348](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_windows/src/window.rs#L342-L348)).
A moved or resized fullscreen window can keep old bounds. A new window always uses the current bounds.
The status data does not reset, because the feeds live outside the window.

On Windows, GPUI receives `WM_DISPLAYCHANGE` on a hidden top-level window, so display events arrive
with no window open
([gpui_windows/src/platform.rs#L213-L217](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_windows/src/platform.rs#L213-L217)).
On macOS, GPUI uses the screen parameters notification
([gpui_macos/src/platform.rs#L1465-L1487](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_macos/src/platform.rs#L1465-L1487)).
On Linux, the Wayland backend reports output changes. The X11 backend reports one display for the
whole X screen and has no RandR change handler in this GPUI revision
([x11/display.rs#L14-L36](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_linux/src/linux/x11/display.rs#L14-L36)).

### Rendering and resource use

The display is idle for most of its run time. It draws only when visible text changes, and each
redraw draws only the views that changed. These techniques come from Zed and from other open-source
GPUI apps:

| Technique | What the app does | Source |
| --- | --- | --- |
| Cached panel views | Each feed panel is its own GPUI entity in the layout through `Entity::cached`. GPUI reuses the last drawing of a cached view until its entity is notified. A notify marks only that view and its parent views as dirty. | GPUI [view.rs#L249-L258](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui/src/view.rs#L249-L258) and [window.rs#L2563-L2575](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui/src/window.rs#L2563-L2575). Zed panes [pane_group.rs#L536](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/workspace/src/pane_group.rs#L536) and docks [dock.rs#L1356](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/workspace/src/dock.rs#L1356). gpui-component tabs [tab_panel.rs#L811](https://github.com/longbridge/gpui-component/blob/146eaf025d8bb4f0f20119b4d64c4917940bd9ad/crates/component/src/dock/tab_panel.rs#L811). |
| Feed events | The board emits `StatusFeedUpdated` for each poll result and does not notify. Only the panel of that feed subscribes to its index and draws again. | `src/status_board.rs`, `src/status_panel.rs` |
| Redraw timers | While a view renders, it records the next time that its clock, age, or freshness text changes. One timer notifies the view at that time. A view without time-based text never wakes up. | `src/status_redraw.rs` |
| Canvas graphs | One `canvas` paints all bars of a graph with `paint_quad`. Before, each bar was a `div`, so one host panel had more than 500 layout nodes for its graphs. | GPUI [canvas.rs#L10-L19](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui/src/elements/canvas.rs#L10-L19). gpui-component charts [candlestick_chart.rs#L383](https://github.com/longbridge/gpui-component/blob/146eaf025d8bb4f0f20119b4d64c4917940bd9ad/crates/component/src/chart/candlestick_chart.rs#L383). |
| Wallpaper at display size | The app scales the wallpaper to the display before GPUI gets it. GPUI keeps a decoded image at its file size, so a 3840 × 2160 file used about 32 MiB for a 1920 × 1080 display. | GPUI [platform.rs#L3462-L3487](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui/src/platform.rs#L3462-L3487) |
| No accessibility adapter | The app starts with `Application::new_inaccessible`, as Zed does unless `ZED_EXPERIMENTAL_A11Y=1`. A screen reader cannot read the status display. | Zed [main.rs#L96-L101](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/zed/src/main.rs#L96-L101), GPUI [app.rs#L250-L263](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui/src/app.rs#L250-L263) |
| Shared Tailscale reads | The tailnet panel and the presence cards share one `tailscale status --json` run for each poll round, instead of one run for each feed. | `src/collector_tailscale.rs` |
| Kept `sysinfo` lists | The local collector keeps its disk and sensor lists and refreshes only their values. On Windows, this keeps one WMI connection for the temperature sensors. | sysinfo [windows/component.rs#L72-L78](https://github.com/GuillaumeGomez/sysinfo/blob/v0.40.4/src/windows/component.rs#L72-L78) |
| Release profile | ThinLTO and one codegen unit, as Zed and tty7 use. Stripped symbols, as gpui-component and OpenLogi use. | Zed [Cargo.toml#L1159-L1162](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/Cargo.toml#L1159-L1162), [tty7 Cargo.toml#L429-L431](https://github.com/l0ng-ai/tty7/blob/5cc39fd486becd36425691184f98beb56ec4d8a2/Cargo.toml#L429-L431), [gpui-component Cargo.toml#L187-L188](https://github.com/longbridge/gpui-component/blob/146eaf025d8bb4f0f20119b4d64c4917940bd9ad/Cargo.toml#L187-L188), [OpenLogi Cargo.toml#L167-L170](https://github.com/AprilNEA/OpenLogi/blob/da245558a4cafef5271480cc10e633c901f62a82/Cargo.toml#L167-L170) |

The app does not use these techniques:

- **mimalloc.** Zed has it as an optional feature that is off by default
  ([crates/zed/Cargo.toml#L167](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/zed/Cargo.toml#L167),
  [main.rs#L91-L93](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/zed/src/main.rs#L91-L93)). The display allocates little between
  polls.
- **`uniform_list`.** A virtual list helps long lists that scroll. The panels show at most about 20
  rows and do not scroll.
- **Size optimization.** Some GPUI apps build with `opt-level = "s"` and keep GPUI and `taffy` at
  `opt-level = 3`. This version does not test that setting.

The benchmark results for these changes are in [bench/README.md](bench/README.md).

### Fonts

The app bundles these fonts, all under the SIL Open Font License:

- IBM Plex Sans Light, Regular, and SemiBold in `assets/fonts/ibm-plex-sans`.
- IBM Plex Serif Light and Regular in `assets/fonts/ibm-plex-serif`.
- IBM Plex Mono Regular and SemiBold in `assets/fonts/ibm-plex-mono`.

GPUI on
Linux maps the system UI font to IBM Plex Sans, but a standalone app does
not have that font
([gpui_linux/src/linux/platform.rs#L132](https://github.com/zed-industries/zed/blob/6501df15765d66d6c2a2b66b1eb22434ccef3f86/crates/gpui_linux/src/linux/platform.rs#L132)).
The bundled fonts give the same text on every platform.

### Module layout

Each module has one concern:

| File | Concern |
| --- | --- |
| `src/main.rs` | Command-line options, key bindings, and app start. |
| `src/status_board.rs` | GPUI model: the poll loop, the feed events, and the wake refresh. |
| `src/status_fonts.rs` | Bundled IBM Plex Sans, Serif, and Mono font registration. |
| `src/status_view.rs` | Root GPUI view: wallpaper, clock, alerts, buttons, actions, and the band and side arrangements. |
| `src/status_panel.rs` | Cached GPUI view for each feed panel: hosts, services, disks, and tailnet. |
| `src/status_graph.rs` | CPU and network history graphs on a canvas. |
| `src/status_redraw.rs` | Redraw timer for clock, age, and freshness text. |
| `src/status_theme.rs` | Colors of the active style and the layout scale. |
| `src/status_style.rs` | Built-in styles: palette, fonts, and panel arrangement for each wallpaper. |
| `src/status_wallpaper.rs` | Wallpaper decode and scale to the display size. |
| `src/status_window.rs` | Window open, fullscreen state, and monitor change handling. |
| `src/display_select.rs` | Display selectors mapped to GPUI displays. |
| `src/status_config.rs` | TOML configuration and the feed list. |
| `src/status_collector.rs` | Collector switch for all sources. |
| `src/collector_local.rs` | This machine through `sysinfo`. |
| `src/collector_glances.rs` | Glances REST API version 4. |
| `src/collector_kuma.rs` | Uptime Kuma public status page API. |
| `src/collector_scrutiny.rs` | Scrutiny summary API. |
| `src/collector_tailscale.rs` | `tailscale status --json`. |
| `src/status_model.rs` | Reading types for hosts, services, disks, and devices. |
| `src/status_feed.rs` | Feed state: latest reading, CPU and network history, and poll errors. |
| `src/status_freshness.rs` | Freshness limits and feed states. |
| `src/status_format.rs` | Size, rate, age, uptime, and clock text. |
| `src/status_display.rs` | Display selectors and the physical display ranking. |
| `src/display_windows.rs` | Windows display configuration queries and the no-activate style. |

## Verification

### Verified in this version

These checks ran on a Linux machine (KDE Plasma, Wayland) with GPUI at the pinned commit:

- `cargo test` passes 32 unit tests. The tests cover the style names and the panel group
  placement of each side style, the
  configuration, the Glances uptime and temperature parsing, the network interface choice, the
  Kuma, Scrutiny, and Tailscale response conversion, the macOS interface and APFS volume choice,
  the Glances presence fallback, the shared Tailscale result, the feed history,
  the freshness rules and their next change, the formatting and the redraw times, the wallpaper
  size, the display selectors, and the physical display ranking. Test inputs use `example.ts.net`
  names.
- `cargo clippy --all-targets` reports no warnings for Linux.
- `cargo clippy --all-targets --target x86_64-pc-windows-gnu` reports no warnings. This check
  compiles the Windows display, focus, `sysinfo`, and Tailscale command code. It does not run the
  code. The check ran in a temporary copy with TLS turned off in `ureq`, because the `ring` crate
  needs a MinGW C compiler. It used `llvm-windres` for the GPUI manifest and `llvm-dlltool` for
  import libraries.

- The app ran against the real Glances, Uptime Kuma, Scrutiny, and Tailscale sources over Tailscale,
  in a nested KWin Wayland session with two virtual outputs. A focused app on output 2 stood in for
  the game.
- A Glances entry for an offline tailnet device showed the offline panel and alert after 30 s.
- The wallpaper, clock, CPU and network graphs, and core grid rendered at 1920 × 1080 and
  1280 × 720. A Glances entry for a closed port showed the offline panel and alert. With no other
  window focused, the keyboard focus alert showed.
- The window buttons did not show in these screenshots, because the virtual session does not move
  the pointer. The pointer reveal needs a hands-on check.
- A temporary Glances 4 server ran on an Apple silicon Mac (`uvx --from "glances[web]" glances -w`)
  and was then stopped. The Mac card showed CPU, memory, uptime, and network rates. The disk meter
  showed the APFS Data volume, and the network rates came from the Wi-Fi interface, not from the
  Tailscale tunnel. Glances reports no CPU temperature on that Mac, so the card shows none. When
  Glances stopped, the card changed to the presence card with the amber note and no alert. When
  Glances started again, the metrics came back. An offline Mac showed the gray presence card.
- On the Mac Studio, the `local.glances.web` LaunchAgent above started Glances 4.5.9 under launchd.
  A request with the MagicDNS host name got status 200. A request with another `Host` header got
  status 400, and so did a request for the disabled process list. Glances used 0.1 % CPU at idle.
  A restart of the Mac did not run, so the start at sign-in needs a hands-on check.
- Each of the 58 side styles rendered at 1920 × 1080 in the nested KWin session with its
  wallpaper and live data. The clock stayed on one line, and no list row lost its name. The
  screenshots stay out of Git, because the art belongs to the artist. The `hills` style rendered
  the band arrangement as before.
- The benchmark in [bench/README.md](bench/README.md) compared the build before and after the
  changes in "Rendering and resource use". App CPU use fell from 1.38 % to 0.55 % of one core, GPU
  busy time from 0.90 % to 0.35 %, and resident memory from 155 MiB to 106 MiB. The first frame
  showed at the same time. Screenshots of both builds showed the same layout and data, and the
  panels kept updating.

### Hands-on verification

These items need a test on real hardware. Windows items come first:

1. **Windows display ranking.** Start a Sunshine stream with the virtual display. Confirm that the
   display label shows the built-in panel. Record the output technology that the virtual display driver
   reports. The ranking treats indirect virtual and unknown outputs as virtual.
2. **Windows primary display swap.** Start and stop a stream that makes the virtual display primary.
   Confirm that the status window stays on, or returns to, the physical panel within about 1 second.
3. **`HMONITOR` stability.** Confirm that GPUI `DisplayId` values match the `HMONITOR` values after a
   display change.
4. **`WS_EX_NOACTIVATE` with GPUI.** Click the status window during a stream. Confirm that the game
   keeps input and that the window buttons show and work. Confirm that GPUI's DirectComposition
   renderer draws normally with this style.
5. **Taskbar overlap.** The status window is not the foreground window, so Windows can draw the
   taskbar over it. Check whether the taskbar covers the bottom of the panels. If it does, use taskbar auto-hide or
   move the taskbar to another display.
6. **Sleep and lid.** Close and open the laptop lid during a stream. Confirm that the display
   recovers and each panel header shows its live fact again.
7. **macOS fullscreen focus.** Confirm whether native fullscreen takes focus from the streamed app.
   If it does, test `toggle_simple_fullscreen`.
8. **Linux X11.** Confirm fullscreen placement with more than one monitor. GPUI X11 sees one display
   for the whole X screen.

## Open questions

- Which styles do you keep? The repository has nine drafts for wallpapers by Yolanda, 35
  drafts for wallpapers by HoYoverse event artists, 14 drafts for official contest winners, and
  the `hills` style for the Windows XP "Bliss" placeholder. The 15 drafts from `odette-stars` to
  `castorice-ruins` use contest entries and HoYoFair works without a known award. Do you remove
  them?
- Must the app start at sign-in on the Windows laptop? This version adds no startup entry or
  service.

## Licenses

The app uses the Apache License 2.0. GPUI uses the Apache License 2.0. The bundled IBM Plex fonts
use the SIL Open Font License.
