# Kindle Paperwhite 1 (B024) - Reverse Engineering & Hacks

This document contains everything we learned while trying to turn a 2012 Kindle Paperwhite 1 (Firmware ~5.6.1.1) into a passive E-Ink display.

## 1. Hardware & Battery Quirks
- **Brown-outs**: The battery is old. Heavy flash writes (like copying many files) while the screen is refreshing can cause instant brown-outs and hardware resets.
- **Mounting**: Auto-mounting on modern Linux via USB can trigger heavy I/O. We used a privileged Docker container to mount `/dev/sdb1` manually to avoid system automounting crashes.

## 2. Jailbreak (WinterBreak2)
- We used **WinterBreak2**, which exploits the Experimental Browser.
- Copied the `winterbreak2` folder to the root of the Kindle (`/mnt/us/`).
- Accessed `file:///mnt/us/winterbreak2/dialoger.html` via the Kindle browser to trigger the jailbreak.
- Installed the **Jailbreak Hotfix** (`Update_jailbreak_hotfix_1.16.N_install.bin`) via Settings -> Update Your Kindle to make it permanent.

## 3. KUAL & Firmware 5.6+ Defenses
- Firmware 5.6+ actively blocks `*.azw2` "Active Content" that isn't signed by Amazon. Standard KUAL fails to open with "Application not signed by an authorized developer".
- **KUAL Coplate**: We had to use the "Coplate" version of the KUAL Booklet.
- Even with Coplate, the Kindle Home screen hides the KUAL booklet. It must be launched via the Search bar.
- **Search Bar Execution**: Firmware 5.6.1.1 blocks standard `;sh` execution from the search bar. The only command that consistently works is `;log mrpi` (MobileRead Package Installer).

## 4. SSH & USBNetwork (The Golden Key)
Because KUAL and native screensaver hacks (`linkss`) were being aggressively overridden by Amazon's UI (showing the typewriter/pens screensavers), we needed root access.
- Installed **USBNetwork** via MRPI (`;log mrpi`).
- Activated via the search bar: `;un`.
- Connected via USB. The Kindle exposes a network interface (`enxee4900000000`).
- Configured local IP: `sudo ip addr add 192.168.15.201/24 dev enxee4900000000`.
- The Kindle IP is always `192.168.15.244`.
- **Root Password**: The root password is derived from the Serial Number. For our PW1 (B024...), the password is `***REMOVED***` (or `fionae577` on older FW).
- We injected our `id_rsa.pub` into `/root/.ssh/authorized_keys` and `/mnt/us/usbnet/etc/authorized_keys` to bypass password prompts.

## 5. Controlling the E-Ink Display
Amazon's UI (`lab126_gui`) will fight any attempt to change the screen, re-rendering the Home page or the lockscreen.

### Attempted Methods:
1. **ScreenSavers Hack (linkss)**: Failed. FW 5.6+ overrides it with compressed `.sqsh` system assets.
2. **FBInk**: The community build of `fbink` for K5 does **not** include image support to save space (`[FBInk] Image support is disabled in this FBInk build!`). It only renders text.

### The Working Solution (`eips` + `stop lab126_gui`)
We use the native Amazon hardware tool `eips` to manipulate the framebuffer, but we MUST kill the Amazon UI first.

**Command sequence:**
```bash
# 1. Kill the Amazon GUI so it stops fighting for the screen
stop lab126_gui 2>/dev/null

# 2. Clear the e-ink screen (removes ghosting)
/usr/sbin/eips -c
/usr/sbin/eips -c

# 3. Draw the PNG image (Must be EXACTLY 758x1024, Grayscale)
/usr/sbin/eips -f -g /mnt/us/dashboard.png
```

To restore the Kindle to normal reading mode, perform a hard reboot (hold the power button for 40 seconds) to restart the `lab126_gui` framework.
