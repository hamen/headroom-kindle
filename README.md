# Headroom Kindle E-Ink Dashboard

A Rust-based dashboard generator and deployment pipeline to turn a Kindle Paperwhite 1 (2012) into a passive, battery-efficient E-Ink display. 
The dashboard generates a Tesserae/Home-Assistant style image with AI quota limits (from Headroom) and pushes it to the Kindle over USB/Network.

## Project Structure
- `src/main.rs`: Rust program that uses `image` and `imageproc` to render a 758x1024 grayscale `.png` dashboard.
- `assets/`: Contains the fonts (`font.ttf`, `font_light.ttf`) used for rendering.
- `scripts/`: Python and Bash scripts to push the image to the Kindle and trigger the e-ink refresh.
- `KINDLE_HACKS.md`: Extremely detailed documentation on how the Kindle PW1 was jailbroken, how SSH was enabled, and the commands used to manipulate the e-ink framebuffer.

## How it works
1. **Render**: The Rust binary generates `dashboard.png` (758x1024) locally.
2. **Push**: A Python script (`scripts/push_to_kindle.py`) connects to the Kindle via SSH (USBNetwork).
3. **Display**: The script kills the native Amazon UI (`lab126_gui`) and uses the hardware command `eips` to clear the screen and draw the PNG directly to the e-ink display.

## Dependencies
- Rust / Cargo
- Python 3 with `paramiko` (for the SSH deployment)
- A Jailbroken Kindle with USBNetwork enabled.

## Usage
To build the dashboard and push it to the connected Kindle:
```bash
cargo build
python3 scripts/push_to_kindle.py
```
