# anisette-server (ipaside fork of omnisette-server)

A production-oriented anisette (anisette-v3 / "omnisette") server. It speaks the exact same
protocol as [SideStore's `omnisette-server`](https://github.com/SideStore/omnisette-server) —
`iloader`, `xtool`, SideStore, AltStore-style clients, and [ipatool]-style tools all work
unchanged — plus the additions below.

This is a fork of **[SideStore/omnisette-server](https://github.com/SideStore/omnisette-server)**
and uses a fork of **[SideStore/apple-private-apis](https://github.com/pwnapplehat/apple-private-apis)**
(`omnisette` crate). All credit for the original server and the ADI plumbing goes to SideStore and
Dadoum. See [Credits & license](#credits--license).

## Public instance

A public instance runs at **`https://anisette.ipaside.com`**. Point any anisette-v3 client
(iloader, xtool, SideStore, ipatool-style tools, etc.) at it — and it supports the App Store
`ds_id` extension described below. It is rate-limited per client IP but the limits are generous for
normal use. Please be considerate with load; if you expect heavy or production traffic, **self-host**
(see [Production deployment](#production-deployment)) rather than relying on this instance.

## What this fork adds

1. **Per-request `ds_id`.** Ordinary anisette provisions and requests OTPs under directory-services
   ID `-2`. The **App Store's AMD (type-2) machine provisioning** requires `-1` (from AMS
   `_accountIDForType:`) — an identity provisioned at `-2` has its `end_provisioning` / `request_otp`
   rejected by the store. This server lets a client choose the `ds_id` **per request**:
   - `POST /v3/get_headers` accepts an optional `"ds_id"` field (default `-2`).
   - the `/v3/provisioning_session` WebSocket accepts an optional `"ds_id"` in the first
     (`identifier`) message (default `-2`).

   Omit it and every existing client behaves exactly as before. Send `-1` for the App Store flow.
   This is what lets a single server serve both ordinary anisette **and** App Store auth.

2. **Built-in per-client rate limiting.** A dependency-free token-bucket limiter, keyed on the real
   client IP (`CF-Connecting-IP` when behind Cloudflare, else `X-Forwarded-For`, else the socket
   peer). Generous by default (burst 200, refill 5/s ≈ 300/min sustained per IP) so ordinary use —
   even several users behind one NAT/CGNAT address — never trips it, while a single abusive source is
   bounded. Tune the constants at the top of [`src/rate_limit.rs`](src/rate_limit.rs).

3. **`GET /health`.** A cheap liveness endpoint (no ADI work) for uptime checks and load balancers.

## Endpoints

| Method | Path | Purpose |
|---|---|---|
| `GET`  | `/` | V1 headers (auto-provisions at `-2` on first call) |
| `GET`  | `/health` | Liveness check, returns `ok` |
| `GET`  | `/v3/client_info` | `client_info` / `user_agent` strings |
| `GET`  | `/v3/provisioning_session` | WebSocket provisioning (accepts optional `ds_id`) |
| `POST` | `/v3/get_headers` | `{identifier, adi_pb, ds_id?}` → anisette headers |

## Build

The server loads Apple's native ADI libraries at runtime; they are **not** redistributed here (they
are Apple's, extracted from the Apple Music APK). Fetch them as upstream documents:

```bash
sudo apt update
sudo apt install --no-install-recommends -y git unzip curl perl make
curl https://sh.rustup.rs -sSf | sh   # option 1
source "$HOME/.cargo/env"

cargo build --release

curl https://apps.mzstatic.com/content/android-apple-music-apk/applemusic.apk -O
unzip applemusic.apk 'lib/*/libstoreservicescore.so' 'lib/*/libCoreADI.so'
rm applemusic.apk
```

Run from the directory that contains `lib/` (and a writable `provisioning/`):

```bash
./target/release/omnisette-server --ip 127.0.0.1 --http-port 8787 --log-level Info
```

`--help` lists all options. The `.so` files, `applemusic.apk`, provisioning state, and any TLS/tunnel
secrets are all git-ignored.

## Production deployment

See [`deploy/`](deploy/) for a complete production setup:

- a hardened **systemd** unit ([`deploy/anisette.service`](deploy/anisette.service)) running as a
  dedicated unprivileged user, bound to localhost;
- a **Cloudflare Tunnel** config template ([`deploy/cloudflared-config.yml`](deploy/cloudflared-config.yml))
  so the origin IP and port are never exposed and TLS is handled at the edge;
- [`deploy/SETUP.md`](deploy/SETUP.md): the full checklist — service isolation from other workloads
  on the same box, resource protection, automated backups of the irreplaceable tunnel credentials,
  `fail2ban`, and automatic security updates.

## Credits & license

- Original server: **[SideStore/omnisette-server](https://github.com/SideStore/omnisette-server)**.
- ADI / `omnisette` crate: **[SideStore/apple-private-apis](https://github.com/SideStore/apple-private-apis)** (MPL-2.0), and **[Dadoum](https://github.com/Dadoum)** for the underlying work.

The `omnisette` crate and our changes to it are licensed **MPL-2.0** (see [`LICENSE`](LICENSE) and the
[apple-private-apis fork](https://github.com/pwnapplehat/apple-private-apis)). This repository is a
fork of the upstream server; see the GitHub fork link for its origin and history.
