# Wake-on-LAN setup

WoL must be enabled at every layer: BIOS, OS, and NIC driver.

## BIOS

1. Enter BIOS (usually F2/Del during boot)
2. Look for **Wake-on-LAN**, **PCI-E Wake**, **Magic Packet Wake**, or **Power On by PCIe**
3. Enable it
4. Save and exit

The exact name varies by motherboard vendor. Check your manual if unsure.

## Windows

1. Open **Device Manager** → Network adapters
2. Right-click your Ethernet NIC → **Properties**
3. **Power Management** tab:
   - ✅ Allow this device to wake the computer
   - ✅ Allow magic packets to wake the computer
   - (uncheck "Wake on Magic Packet when system is in...")
4. **Advanced** tab:
   - Set **Wake on Magic Packet** → Enabled
   - Set **Wake on Pattern Match** → Enabled
5. Open **Control Panel → Power Options → Choose what the power buttons do**
   - Uncheck **Turn on fast startup** (this disables WoL for many users)

## Linux

### Check current WoL status

```bash
sudo ethtool <interface> | grep Wake-on
```

Output: `Supports Wake-on: pumbg` / `Wake-on: d` (`d` = disabled, `g` = enabled).

### Enable WoL

```bash
sudo ethtool -s <interface> wol g
```

### Make it persistent

**Ubuntu/Debian** — add a NetPlan hook or systemd service:

```bash
# /etc/systemd/system/wol.service
[Unit]
Description=Enable Wake-on-LAN

[Service]
Type=oneshot
ExecStart=/usr/sbin/ethtool -s <interface> wol g

[Install]
WantedBy=basic.target
```

```bash
sudo systemctl enable wol.service
sudo systemctl start wol.service
```

## Test WoL before using the ESP32

Install a WoL tool on your phone (e.g. **Wolow** on Android) and send a magic packet. If the PC wakes, the ESP32 config is the only thing left to fix. If it doesn't, the PC still needs BIOS or OS changes above.

## TCP probe port

The `/status` command checks whether the PC is online by connecting to a TCP port (default `PC_TCP_PORT`).

Pick a port that your PC listens on:
- **47989** — Moonlight (game streaming)
- **22** — SSH (if running)
- **445** — SMB file sharing (Windows)
- **80/443** — a local web server

Make sure the port isn't blocked by a firewall. On Windows you may need to add an inbound rule in **Windows Defender Firewall**.

## Config reference

```c
#define PC_NAME     "gaming-pc"       // display name in Telegram
#define PC_MAC      "aa:bb:cc:dd:ee:ff"   // from ipconfig /all or ifconfig
#define PC_IP       "192.168.1.50"    // static IP or DHCP reservation
#define WOL_BCAST   "192.168.1.255"   // LAN broadcast address
#define WOL_PORT    9                 // 7 or 9, usually
#define PC_TCP_PORT  47989            // port for /status probe
```

Set a **static IP / DHCP reservation** for the target PC so the ESP32 always knows where to find it.
