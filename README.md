# Headroom Kindle E-Ink Dashboard

A Rust-based dashboard generator and deployment pipeline to turn a Kindle Paperwhite 1 (2012) into a passive, battery-efficient E-Ink display. 
The dashboard generates a Tesserae/Home-Assistant style image with the AI subscription quota left for Claude, Codex, Z.AI and Grok (read from the same endpoints the [Headroom](https://github.com/rock3r/headroom) app uses) and draws it on the Kindle over the USB cable, which also powers the Kindle. One Rust binary does everything; a systemd user timer runs it every 10 minutes.

## Project Structure
- `src/main.rs`: the single binary: fetches the quota, renders the dashboard and draws it on the Kindle.
- `src/providers/`: one quota fetcher per provider (`claude`, `codex`, `zai`, `grok`). `src/tokens.rs` reads the CLI logins, `src/quota.rs` holds the shared window/error types.
- `src/render.rs`: the drawing code (`image` and `imageproc`). We use high-contrast thick fonts (`LiberationSans`) because thin grey lines wash out on old E-Ink screens. Tiles are white with a black border; a tile turns black when its tightest window has less than 20% left, so it stands out.
- `tests/fixtures/`: hand-written sample responses for the parser tests (`cargo test`, no network).
- `assets/`: Contains the font (`font.ttf`, LiberationSans Bold, with its license in `OFL.txt`) used for rendering. All text is bold: thin text washes out on this e-ink, worst of all white on black.
- `src/kindle.rs`: pushes the PNG to the Kindle over SSH (`ssh2`, password login, upload through `cat` on the SSH channel because SFTP fails against the Kindle) and runs the screen takeover. The first run pins the Kindle's SSH host key in `~/.config/headroom-kindle/kindle-host-key`; after that a different key stops the push before the password is sent. It never goes through `ssh-agent`, whose passphrase prompts broke automation in bash.
- `systemd/`: the user service and timer that run the binary every 10 minutes.
- `KINDLE_HACKS.md`: **MUST READ.** Extremely detailed documentation on the jailbreaking process, fighting the firmware 5.6.1.1 UI watchdogs, the discovery of the `fbink` image-support missing feature, and the ultimate `eips` solution.

## How it works
1. **Render**: The binary fetches the quota of the four providers and renders the dashboard (758x1024) in memory. The dashboard is drawn landscape (1024x758) for a Kindle standing on its long side with its top edge to the right (USB port and charger on the left), then turned 90 degrees counterclockwise into the portrait framebuffer. A thin header shows the date and `updated HH:MM` (no clock: the screen changes only every 10 minutes). Below it, a 2x2 grid with one tile per provider: the big number is the percent left of the tightest window; on the right, two lines per window with its percent left and its reset time. The PNG must be 8-bit grayscale: `eips` draws an RGB PNG stretched to twice the width.
2. **Push**: It connects to the Kindle via SSH over the USB cable (USBNetwork, Kindle `192.168.15.244`, PC `192.168.15.201` from the "Kindle USB" NetworkManager profile) and uploads `/mnt/us/dashboard.png`.
3. **Display**: It stops the native Amazon UI (`stop lab126_gui 2>/dev/null`) and draws the PNG with `eips -g`. On the first run of each hour (or with `--full`) it clears the ghosting first (`eips -c` twice, then `eips -f -g`), which flashes the screen; the other runs redraw in place without a flash. The Kindle is left in a "zombie" state: the Amazon UI is off and the e-ink keeps the image with no power.

## Data sources
Each tile re-uses the login of that provider's own CLI on this PC:

| Tile   | Credential                                                                 | Endpoint |
|--------|----------------------------------------------------------------------------|----------|
| CLAUDE | `~/.claude/.credentials.json` (Claude Code login)                          | `api.anthropic.com/api/oauth/usage` |
| CODEX  | `~/.codex/auth.json` (Codex CLI login)                                     | `chatgpt.com/backend-api/wham/usage` |
| Z.AI   | `ZAI_API_KEY`, else the `zai-coding-plan` key in `~/.local/share/opencode/auth.json` | `api.z.ai/api/monitor/usage/quota/limit` |
| GROK   | `~/.grok/auth.json` (Grok CLI login)                                       | `cli-chat-proxy.grok.com/v1/billing` |

**The program only reads these files.** It never refreshes a token and never writes a credential file: these CLIs rotate single-use refresh tokens, so a refresh here would log the CLI out. When a tile says `sign-in expired`, run that CLI once (it refreshes its own token) and render again.

Other tile messages: `not set up` (no login found), `no access` (403), `rate limited` (429), `offline` (network error or 15 s timeout), `bad data` (the response changed shape), `no data` (the account reports no limits).

These are undocumented endpoints. They can change at any time; then only that tile shows `bad data`. Endpoint and field knowledge comes from Headroom (Apache-2.0).

## Dependencies
- Rust / Cargo (`ssh2` builds its own libssh2; it needs the OpenSSL headers that `reqwest` already uses)
- A Jailbroken Kindle PW1 with **USBNetwork** enabled (`;un`), on the USB cable.
- The Kindle root password (derived from the serial number, see KINDLE_HACKS.md section 4) in `~/.config/headroom-kindle/kindle-password` (mode 600), or in `KINDLE_PASSWORD`. It is not in the repo.

## Usage
```bash
# Install the binary to ~/.cargo/bin
cargo install --path .

headroom-kindle            # fetch, render and draw on the Kindle
headroom-kindle --full     # same, but clear the ghosting first (flashes)
headroom-kindle --no-push  # only write dashboard.png in the current directory

# Refresh every 10 minutes
cp systemd/headroom-kindle.service systemd/headroom-kindle.timer ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user enable --now headroom-kindle.timer
journalctl --user -u headroom-kindle   # one line per provider, then "kindle: drawn (...)"
```

The binary exits 0 when the Kindle has drawn the image, even if a provider failed (that tile shows the error). It exits 1 when the Kindle cannot be reached or `eips` fails. When the PC is off or the cable is out, the Kindle keeps the last image; `updated HH:MM` in the header shows how old it is.

## Inspiration & UI Design
The UI of this dashboard is heavily inspired by two excellent open-source projects designed specifically for E-Ink displays:
- [hass-eink-dashboard](https://github.com/cryptomilk/hass-eink-dashboard): Taught us to use thick, high-contrast layouts and grid-based cards to counter the washout effect of old e-ink displays.
- [Tesserae](https://github.com/dmellok/tesserae): Influenced the tile design (massive typography, and black tiles with white text). Here only a tile whose quota is low turns black, so it stands out on the 2012 Pearl E-Ink screen.

## License
The code in this repository is under the MIT License (see `LICENSE`).

`assets/font.ttf` is Liberation Sans Bold 2.1.5 (Copyright 2012 Red Hat, Inc.), under the SIL Open Font License 1.1 (see `assets/OFL.txt`).

The quota endpoints, headers and field names come from [Headroom](https://github.com/rock3r/headroom) by Sebastiano Poggi, Chris Sinco and Marco Gomiero, under the Apache License 2.0. The requests set the same headers Headroom sets.
