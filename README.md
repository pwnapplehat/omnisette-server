# App Store anisette server (ipaside fork of omnisette-server)

An anisette server **dedicated to the App Store's AMD (type-2) machine-provisioning flow**
(`ds_id = -1`). It speaks the anisette-v3 / "omnisette" protocol, so App-Store-download tools can use
it directly — and it **only** serves that flow. Ordinary anisette (`ds_id = -2`, the kind used for
GrandSlam / Apple-ID *sign-in* and app signing) is intentionally **not** served here.

This is a fork of **[SideStore/omnisette-server](https://github.com/SideStore/omnisette-server)** and
uses a fork of **[SideStore/apple-private-apis](https://github.com/pwnapplehat/apple-private-apis)**
(`omnisette` crate). All credit for the original server and the ADI plumbing goes to SideStore and
Dadoum. See [Credits & license](#credits--license).

## Why App-Store-only

The ADI libraries here are Android-derived (from the Apple Music APK). Apple accepts that kind of
anisette for the **App Store AMD** flow, but **rejects it for GrandSlam / Apple-ID sign-in**
(`-80045` / `-80046`) — that needs *Mac-derived* anisette, which this server does not and cannot
produce. So rather than pretend to be a general anisette server, this one is scoped to exactly what it
can do well: the App Store `ds_id = -1` flow. For sign-in / signing, use a Mac-based anisette service.

## What this fork adds

1. **Per-request `ds_id`, locked to the App Store value.** `POST /v3/get_headers` and the
   `/v3/provisioning_session` WebSocket both require `ds_id = -1`; any other value (or none) is
   rejected. (The underlying capability is a per-request `ds_id` in the
   [apple-private-apis fork](https://github.com/pwnapplehat/apple-private-apis); this server pins it
   to the App Store value.)
2. **Built-in per-client rate limiting.** A dependency-free token-bucket limiter, keyed on the real
   client IP (`CF-Connecting-IP` behind Cloudflare, else `X-Forwarded-For`, else the socket peer).
   Generous by default (burst 200, refill 5/s ≈ 300/min sustained per IP). Tune the constants at the
   top of [`src/rate_limit.rs`](src/rate_limit.rs).
3. **`GET /health`.** A cheap liveness endpoint (no ADI work).

## Public instance

A public instance runs at **`https://anisette.ipaside.com`** — as far as we know the first public
anisette that supports the App Store `ds_id = -1` flow, so App-Store-download tools can point at it
directly. It is rate-limited per client IP (generous for normal use); for heavy or production traffic,
**self-host** (see [Production deployment](#production-deployment)). It does **not** serve ordinary
sign-in anisette — use a Mac-based service for that.

## Endpoints

| Method | Path | Purpose |
|---|---|---|
| `GET`  | `/` | Info message (what this server is) |
| `GET`  | `/health` | Liveness check, returns `ok` |
| `GET`  | `/v3/client_info` | `client_info` / `user_agent` strings |
| `GET`  | `/v3/provisioning_session` | WebSocket provisioning — **requires `ds_id = -1`** |
| `POST` | `/v3/get_headers` | `{identifier, adi_pb, ds_id}` — **`ds_id` must be `-1`** |

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
  on the same box, resource protection, automated backups of the tunnel credentials, `fail2ban`, and
  automatic security updates.

## Credits & license

- Original server: **[SideStore/omnisette-server](https://github.com/SideStore/omnisette-server)**.
- ADI / `omnisette` crate: **[SideStore/apple-private-apis](https://github.com/SideStore/apple-private-apis)** (MPL-2.0), and **[Dadoum](https://github.com/Dadoum)** for the underlying work.

The `omnisette` crate and our changes to it are licensed **MPL-2.0** (see [`LICENSE`](LICENSE) and the
[apple-private-apis fork](https://github.com/pwnapplehat/apple-private-apis)). This repository is a
fork of the upstream server; see the GitHub fork link for its origin and history.
