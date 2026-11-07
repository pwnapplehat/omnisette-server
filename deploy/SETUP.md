# Production setup

A complete, hardened deployment on a small Linux VM (tested on Ubuntu, x86-64). Compute needs are
tiny; reliability and isolation are what matter. Run everything below as root.

## 1. Build & install

Build the binary and fetch the Apple libs (see the top-level README), then lay it out under
`/opt/anisette`:

```bash
useradd --system --home /opt/anisette --shell /usr/sbin/nologin anisette
mkdir -p /opt/anisette/provisioning
cp target/release/omnisette-server /opt/anisette/
cp -r lib /opt/anisette/lib            # the extracted Apple .so libraries
chown -R anisette:anisette /opt/anisette
chmod 750 /opt/anisette                 # keep other users out
```

## 2. systemd service

Install the hardened unit and start it (binds to `127.0.0.1:8787` only):

```bash
cp deploy/anisette.service /etc/systemd/system/anisette.service
systemctl daemon-reload
systemctl enable --now anisette.service
curl -s localhost:8787/health          # -> ok
```

The unit runs as the unprivileged `anisette` user, sandboxed (`ProtectSystem=strict`, etc.), and
resource-protected (`OOMScoreAdjust=-800`, `MemoryMin`, high `CPUWeight`) so other workloads on the
box can never starve or OOM-kill it.

## 3. Cloudflare Tunnel (stable URL, hidden origin, free TLS)

```bash
# install cloudflared, then:
cloudflared tunnel login                               # authorize your domain's zone
cloudflared tunnel create anisette
cp deploy/cloudflared-config.yml /etc/cloudflared/config.yml
# edit it: set the tunnel UUID + credentials file + your hostname
cloudflared tunnel route dns anisette anisette.example.com
cloudflared service install
```

Firewall: only SSH needs to be open inbound — the tunnel is outbound.

```bash
ufw allow OpenSSH && ufw --force enable
```

## 4. Isolate the service from your other work on the box

If you also run other projects / CI / AI agents on this VM, make sure they can't touch anisette:

- Do that work as a **separate unprivileged user** (e.g. `dev`) that is **not** in `sudo`, `docker`,
  or the `anisette` group.
- `/opt/anisette` is `750 anisette:anisette` and `/etc/cloudflared` is `700 root` — so `dev` cannot
  read the state, edit the unit, or stop the service (systemd denies it without privilege).
- Optionally cap the `dev` login slice so a runaway job leaves headroom:
  `/etc/systemd/system/user-<uid>.slice.d/limits.conf` with `MemoryMax=` / `CPUQuota=`.

## 5. Backups of the irreplaceable state

The binary and `.so` libs are rebuildable; the **Cloudflare tunnel credentials + origin cert** are
not (losing them means recreating the tunnel and DNS). Back them up daily:

```bash
# /usr/local/sbin/anisette-backup.sh
tar czf "/root/anisette-backups/anisette-$(date +%F-%H%M%S).tar.gz" \
  /etc/cloudflared /root/.cloudflared /etc/systemd/system/anisette.service
```

Drive it with a `systemd` timer (`OnCalendar=daily`, `Persistent=true`) and prune to the newest ~14.

## 6. Baseline host security

```bash
apt-get install -y fail2ban unattended-upgrades
systemctl enable --now fail2ban
printf 'APT::Periodic::Update-Package-Lists "1";\nAPT::Periodic::Unattended-Upgrade "1";\n' \
  > /etc/apt/apt.conf.d/20auto-upgrades
# SSH: key-only
printf 'PasswordAuthentication no\nPermitRootLogin prohibit-password\n' \
  > /etc/ssh/sshd_config.d/99-hardening.conf && sshd -t && systemctl reload ssh
```

## Notes on running a public instance

- All clients share one provisioning identity (`adi.pb`); Apple may throttle a single identity under
  heavy load. For a high-traffic public instance, rotate a pool of identities.
- The built-in rate limiter (`src/rate_limit.rs`) is per-client-IP and generous by default; tune the
  constants for your expected traffic.
