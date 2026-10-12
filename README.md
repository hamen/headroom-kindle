# Headroom Kindle E-Ink Dashboard

A Rust-based dashboard generator and deployment pipeline to turn a Kindle Paperwhite 1 (2012) into a passive, battery-efficient E-Ink display. 
The dashboard generates a Tesserae/Home-Assistant style image with AI quota limits (from Headroom) and pushes it to the Kindle over USB/Network.

## Project Structure
- `src/main.rs`: Rust program that uses `image` and `imageproc` to render a 758x1024 grayscale `.png` dashboard. We use high-contrast thick fonts (`LiberationSans`) and a dark-tile UI paradigm because thin grey lines wash out on old E-Ink screens.
- `assets/`: Contains the fonts (`font.ttf`, `font_light.ttf`) used for rendering.
- `scripts/push_to_kindle.py`: A Python script using `paramiko` to silently push the image to the Kindle via SSH and execute the screen takeover. (We chose `paramiko` to bypass local `ssh-agent` passphrase prompts that broke automation in bash).
- `KINDLE_HACKS.md`: **MUST READ.** Extremely detailed documentation on the jailbreaking process, fighting the firmware 5.6.1.1 UI watchdogs, the discovery of the `fbink` image-support missing feature, and the ultimate `eips` solution.

## How it works
1. **Render**: The Rust binary generates `dashboard.png` (758x1024) locally.
2. **Push**: The `push_to_kindle.py` script connects to the Kindle via SSH (USBNetwork on IP `192.168.15.244`).
3. **Display**: The script kills the native Amazon UI (`stop lab126_gui 2>/dev/null`), clears the ghosting (`eips -c`), and uses the hardware command `eips -f -g` to draw the PNG directly to the e-ink display. The Kindle is then left in a "zombie" state where the screen consumes 0 battery and never goes to sleep.

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
