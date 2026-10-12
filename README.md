# Headroom Kindle E-Ink Dashboard

A Rust-based dashboard generator and deployment pipeline to turn a Kindle Paperwhite 1 (2012) into a passive, battery-efficient E-Ink display. 
The dashboard generates a Tesserae/Home-Assistant style image with the AI subscription quota left for Claude, Codex, Z.AI and Grok (read from the same endpoints the [Headroom](https://github.com/rock3r/headroom) app uses) and pushes it to the Kindle over USB/Network.

## Project Structure
- `src/main.rs`: Rust program that fetches the quota and renders a 758x1024 `.png` dashboard.
- `src/providers/`: one quota fetcher per provider (`claude`, `codex`, `zai`, `grok`). `src/tokens.rs` reads the CLI logins, `src/quota.rs` holds the shared window/error types.
- `src/render.rs`: the drawing code (`image` and `imageproc`). We use high-contrast thick fonts (`LiberationSans`) and a dark-tile UI paradigm because thin grey lines wash out on old E-Ink screens.
- `tests/fixtures/`: hand-written sample responses for the parser tests (`cargo test`, no network).
- `assets/`: Contains the font (`font.ttf`, LiberationSans Bold) used for rendering. All text is bold: thin text washes out on this e-ink, worst of all white on black.
- `scripts/push_to_kindle.py`: A Python script using `paramiko` to silently push the image to the Kindle via SSH and execute the screen takeover. (We chose `paramiko` to bypass local `ssh-agent` passphrase prompts that broke automation in bash).
- `KINDLE_HACKS.md`: **MUST READ.** Extremely detailed documentation on the jailbreaking process, fighting the firmware 5.6.1.1 UI watchdogs, the discovery of the `fbink` image-support missing feature, and the ultimate `eips` solution.

## How it works
1. **Render**: The Rust binary fetches the quota of the four providers and generates `dashboard.png` (758x1024) locally. The dashboard is drawn landscape (1024x758) for a Kindle standing on its long side with its top edge to the left, then turned 90 degrees clockwise into the portrait framebuffer. It is a 2x2 grid with one tile per provider: the big number is the percent left of the tightest window; on the right, two lines per window with its percent left and its reset time. The PNG must be 8-bit grayscale: `eips` draws an RGB PNG stretched to twice the width.
2. **Push**: The `push_to_kindle.py` script connects to the Kindle via SSH (USBNetwork on IP `192.168.15.244`).
3. **Display**: The script kills the native Amazon UI (`stop lab126_gui 2>/dev/null`), clears the ghosting (`eips -c`), and uses the hardware command `eips -f -g` to draw the PNG directly to the e-ink display. The Kindle is then left in a "zombie" state where the screen consumes 0 battery and never goes to sleep.

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
- Rust / Cargo
- Python 3 with `paramiko` (for the SSH deployment)
- A Jailbroken Kindle PW1 with **USBNetwork** enabled and SSH keys injected.

## Usage
To build the dashboard and push it to the connected Kindle:
```bash
# Build the Rust dashboard generator
cargo build

# Run the generator
./target/debug/headroom-kindle

# Push to the Kindle via SSH and force the E-Ink refresh
python3 scripts/push_to_kindle.py
```

## Inspiration & UI Design
The UI of this dashboard is heavily inspired by two excellent open-source projects designed specifically for E-Ink displays:
- [hass-eink-dashboard](https://github.com/cryptomilk/hass-eink-dashboard): Taught us to use thick, high-contrast layouts and grid-based cards to counter the washout effect of old e-ink displays.
- [Tesserae](https://github.com/dmellok/tesserae): Influenced our current design paradigm (black tiles with white text and massive typography). Inverting the colors on crucial tiles entirely prevents the "thin gray lines" from fading out on the 2012 Pearl E-Ink screen.
