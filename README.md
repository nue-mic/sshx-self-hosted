# sshx (self-hosted fork)

A secure web-based, collaborative terminal.

![](https://i.imgur.com/Q3qKAHW.png)

**Features:**

- Run a single command to share your terminal with anyone.
- Resize, move windows, and freely zoom and pan on an infinite canvas.
- See other people's cursors moving in real time.
- Connect to the nearest server in a globally distributed mesh.
- End-to-end encryption with Argon2 and AES.
- Automatic reconnection and real-time latency estimates.
- Predictive echo for faster local editing (à la Mosh).

This is a self-hosting fork of [sshx](https://github.com/ekzhang/sshx). It adds
**fixed, per-machine URLs** and an **automated build pipeline** that publishes
static binaries and a container image to GitHub. Everything else works exactly
like upstream; visit [sshx.io](https://sshx.io) to learn about the base project.

## Fixed per-machine URLs

Upstream sshx asks the server for a brand-new random session ID every time the
client starts, so the shareable URL changes on every restart. This fork makes
the URL **stable per machine** by default: the session ID and the end-to-end
encryption key are derived deterministically from a stable machine fingerprint,
so the same machine always produces the same URL.

- **Default:** the fingerprint comes from the network card MAC address, falling
  back to the system machine-id and then the hostname.
- `--ephemeral`: use a one-off random session instead (the original behavior).
- `--machine-seed <value>` / `SSHX_MACHINE_SEED=<value>`: override the
  fingerprint with any stable string. Useful to pin the identity explicitly,
  keep it stable across hardware changes, or run several distinct fixed sessions
  on one machine.

The derivation is a pure function of the fingerprint. Nothing about the machine
identity is sent to the server beyond the chosen session ID (exactly like the
random flow), and the encryption key never leaves the client.

On the server side, if a client opens a session whose fixed ID already exists,
the server only lets it reclaim that ID when the client presents the same
encryption key (verified via the encrypted-zeros block). This lets a machine
reclaim its own URL after a restart, while preventing anyone who merely knows
the public URL from hijacking the ID.

> Because the URL is fixed per machine, running `sshx` twice on the same machine
> makes the second invocation take over the first one's URL. Use a distinct
> `--machine-seed`, or `--ephemeral`, for a second concurrent session.

## Installation

Build the client from source with [Rust](https://rust-lang.com/) installed:

```shell
cargo install --path crates/sshx
```

This compiles the `sshx` binary into your `~/.cargo/bin` folder. Prebuilt static
binaries are also produced by CI (see [Self-hosting](#self-hosting)).

Point the client at your own server and start a session:

```shell
sshx --server https://sshx.example.com
# equivalently: SSHX_SERVER=https://sshx.example.com sshx
```

## Self-hosting

On every push to `main`, CI (`.github/workflows/ci.yaml`) builds and publishes:

- A **container image** for the server on the GitHub Container Registry:
  `ghcr.io/nue-mic/sshx-self-hosted:latest` (also tagged with the commit SHA).
- **Static binaries** (`sshx` and `sshx-server`) for `x86_64-unknown-linux-musl`
  and `aarch64-unknown-linux-musl`, uploaded as workflow build artifacts.

### Run the server with Docker

```shell
docker run -d --name sshx-server -p 8051:8051 \
  -e SSHX_SECRET="$(openssl rand -hex 32)" \
  ghcr.io/nue-mic/sshx-self-hosted:latest \
  ./sshx-server --listen :: --override-origin https://sshx.example.com
```

Notes:

- **Set a fixed `SSHX_SECRET`.** The server signs session tokens with it, so a
  stable secret keeps tokens (and therefore fixed URLs) valid across server
  restarts. If you omit it, the server generates a random secret on each start.
- `--override-origin` is the public base URL clients should be given; it must
  match how users reach the server.
- Put a **TLS-terminating reverse proxy** in front that supports HTTP/2 and gRPC
  — the client talks gRPC to the server, so plain HTTP/1.1 proxies are not
  enough.
- Redis is optional and only needed to run a multi-server mesh; a single server
  runs fine without it (add `--redis-url` / `SSHX_REDIS_URL` to enable it).

### Server options

```
--port <PORT>              Port to listen on (default 8051)
--listen <IP>              Interface to listen on (default ::1; use :: in Docker)
--secret <SECRET>          Token-signing secret (env SSHX_SECRET) — set a fixed value
--override-origin <URL>    Public origin returned in session URLs
--redis-url <URL>          Redis URL for multi-server mesh (env SSHX_REDIS_URL)
--host <HOST>              This server's mesh host address
```

## Development

First, start service containers for development:

```shell
docker compose up -d
```

Install [Rust 1.70+](https://www.rust-lang.org/),
[Node v18](https://nodejs.org/), [NPM v9](https://www.npmjs.com/), and
[mprocs](https://github.com/pvolok/mprocs). Then, run

```shell
npm install
mprocs
```

This compiles and starts the server, an instance of the client, and the web
frontend in parallel on your machine.

Please do not run the development commands in a public setting, as they are not
hardened for untrusted access.
