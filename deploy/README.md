# Deploying a self-hosted sshx server

The server image is published to `ghcr.io/nue-mic/sshx-self-hosted:latest` on
every push to `main`. Pick one of the reverse-proxy options below — both give
you HTTPS, HTTP/2, and gRPC forwarding, which the `sshx` client needs.

Always set a fixed **`SSHX_SECRET`** (e.g. `openssl rand -hex 32`). The server
signs session tokens with it, so a stable secret keeps tokens and the fixed
per-machine URLs valid across restarts.

## Option A — Docker Compose with Caddy (easiest)

`docker-compose.yml` runs the server behind Caddy, which obtains and renews
HTTPS certificates automatically and proxies gRPC over HTTP/2. Point DNS for
`sshx.rtxk.org` (and optionally `sshx.rtxk.us`) at the host, then:

```shell
echo "SSHX_SECRET=$(openssl rand -hex 32)" > deploy/.env
docker compose -f deploy/docker-compose.yml up -d
```

Open https://sshx.rtxk.org to confirm, then install the client with
`curl -sSf https://sshx.rtxk.org/get | sh`.

## Option B — Standalone nginx

If you run your own nginx, start the server (for example
`docker run -d --restart unless-stopped -p 127.0.0.1:8051:8051 -e SSHX_SECRET=... \ ghcr.io/nue-mic/sshx-self-hosted:latest ./sshx-server --listen :: \ --override-origin https://sshx.rtxk.org`)
and use `nginx.conf` as a site config in `/etc/nginx/conf.d/`. Provide TLS
certificates (e.g. via certbot) at the paths referenced in the file.

`sshx.rtxk.org` is the canonical origin (matches `--override-origin`);
`sshx.rtxk.us` is a mirror that reaches the same backend.
