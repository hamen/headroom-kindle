# Kindle Paperwhite 1 (B024) - Reverse Engineering & Hacks

This document chronicles the exhaustive trial-and-error process of turning a 2012 Kindle Paperwhite 1 (Firmware 5.6.1.1) into a passive, battery-efficient E-Ink display. 
The journey involved bypassing Amazon's aggressive UI watchdogs, fighting I/O hardware limits, and discovering undocumented quirks in community hacks.

## 1. Hardware & Battery Quirks
- **Brown-outs & I/O Errors**: The battery is old and struggles to maintain voltage under high I/O load. Our initial attempts to copy the jailbreak payload caused instant brown-outs and corrupted the FAT filesystem.
- **The `fsck.fat` Rescue**: We had to write a Bash script wrapped in an Alpine Docker container to `fsck.fat -a /dev/sdb1` to clear the "dirty bit", delete the corrupted `winterbreak2` folder, and sync writes gracefully.
- **Mounting**: Auto-mounting on modern Linux via USB triggers heavy indexers (like tracker-miner) which spiked I/O. We used a privileged Docker container to mount `/dev/sdb1` manually to bypass host OS meddling.

## 2. Jailbreak (WinterBreak2)
- We used **WinterBreak2**, which exploits the Experimental Browser.
- Copied the `winterbreak2` folder to the root of the Kindle (`/mnt/us/`).
- Accessed `file:///mnt/us/winterbreak2/dialoger.html` via the Kindle browser to trigger the payload.
- Installed the **Jailbreak Hotfix** (`Update_jailbreak_hotfix_1.16.N_install.bin`) via `Settings -> Update Your Kindle` to make it permanent against OTA updates.

## 3. KUAL & Firmware 5.6+ Defenses
- Firmware 5.6+ actively blocks `*.azw2` "Active Content" that isn't signed by Amazon. Standard KUAL fails to open with "Application not signed by an authorized developer".
- **KUAL Coplate**: We had to use the "Coplate" version of the KUAL Booklet to bypass the signature checks.
- **Hidden KUAL**: Even with Coplate, the Kindle Home database (`ccdb`) hides the KUAL booklet. It must be launched via the Search bar.
- **Search Bar Execution Blocks**: Firmware 5.6.1.1 blocks standard `;sh /mnt/us/script.sh` execution from the search bar (an old exploit Amazon patched). The only command that consistently works is `;log mrpi` (MobileRead Package Installer).
- **The MRPI Trojan Attempt**: We attempted to hijack `/mnt/us/extensions/MRInstaller/bin/mrinstaller.sh` to run our own code via `;log mrpi`. While it ran, the Amazon UI (`lab126_gui`) detected the screen tampering and immediately forced a repaint, or kernel-panicked if we killed the UI.

## 4. SSH & USBNetwork (The Golden Key)
Because KUAL and native screensaver hacks (`linkss`) were being aggressively overridden by Amazon's UI (showing the default typewriter/pens screensavers instead of our images), we needed raw root access.
- Installed **USBNetwork** via MRPI (`;log mrpi`).
- Activated the daemon via the search bar: `;un`.
- Connected via USB. The Kindle exposes an RNDIS network interface (`enxee4900000000`).
- Configured local IP: `sudo ip addr add 192.168.15.201/24 dev enxee4900000000`.
- The Kindle IP is always `192.168.15.244`.
- **Root Password**: Derived from the Serial Number. For our PW1 (`B024...`), the password is `***REMOVED***` (or `fionae577` on older FW).
- **SSH Key Injection**: We used a Python `paramiko` script to inject `id_rsa.pub` into `/root/.ssh/authorized_keys` and `/mnt/us/usbnet/etc/authorized_keys` to bypass password prompts permanently.

## 5. Controlling the E-Ink Display (The Final Battle)
Amazon's UI (`lab126_gui`) is a watchdog. It will fight any attempt to change the screen, re-rendering the Home page or the lockscreen immediately.

### Attempted & Failed Methods:
1. **ScreenSavers Hack (`linkss`)**: Failed. FW 5.6.1.1 overrides it. The system either pulled screensavers from compressed `.sqsh` system assets, or the "Show Cover" setting conflicted with it.
2. **FBInk**: The community build of `fbink` for K5 does **not** include image support to save space. We discovered this via SSH logs: `[FBInk] Image support is disabled in this FBInk build!`. It only renders text, making it useless for our PNG dashboard.
3. **The `slideshow.sh` loop**: We tried a background bash loop `while true; do eips -f -g bg.png; sleep 600; done`. This fought with the framework and caused the Kindle to freeze, requiring a 40-second hard reboot.

### The Working Solution: Display-Only Mode (`eips` + `stop lab126_gui`)
We use the native Amazon hardware tool `eips` to manipulate the framebuffer, but we MUST kill the Amazon UI first so it doesn't repaint.

**The Golden Command Sequence (executed via SSH):**
```bash
# 1. Kill the Amazon GUI so it stops fighting for the screen
stop lab126_gui 2>/dev/null

# 2. Clear the e-ink screen twice (removes severe ghosting)
/usr/sbin/eips -c
/usr/sbin/eips -c

# 3. Draw the PNG image (Must be EXACTLY 758x1024, Grayscale)
/usr/sbin/eips -f -g /mnt/us/dashboard.png
```

To restore the Kindle to normal reading mode, perform a hard reboot (hold the power button for 40 seconds) to restart the `lab126_gui` framework.

## 6. The Automation Pipeline (Paramiko & Rust)
- **Rust Dashboard**: Generates the 758x1024 grayscale UI. We adopted a *Tesserae / Windows Phone* tile design, using completely black tiles with white text for maximum e-ink contrast, abandoning thinner fonts that washed out.
- **Python Paramiko Deployment**: We abandoned bash `scp`/`ssh` because `ssh-agent` intercepted the calls and demanded local passphrases, breaking automation. We wrote `scripts/push_to_kindle.py` using `paramiko` to silently authenticate, upload `dashboard.png`, and execute the `eips` screen takeover in one go.
- **Rust push (2026-10-12)**: The paramiko script moved into the Rust binary (`src/kindle.rs`, `ssh2` crate) so one executable fetches, renders and draws. Same flow: password login (no `ssh-agent`), SFTP upload, `eips` takeover. A systemd user timer runs it every 10 minutes over the USB cable.
- **Grayscale only**: `eips -g` draws an RGB PNG stretched to twice the width (only the left half shows). The PNG must be 8-bit grayscale.
