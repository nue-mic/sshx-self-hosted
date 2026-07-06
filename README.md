# sshx（自建版）

一个安全的、基于网页的协作终端。本仓库是 [sshx](https://github.com/ekzhang/sshx)
的自建分支，在其基础上增加了三件事：

- **每台机器固定的访问地址**（重启后 URL 不变）；
- **全平台自动打包 / 递增版本发布**（GitHub Actions）；
- **开箱即用的自建部署配置**（Docker Compose + Caddy，或独立 nginx）。

其余能力与上游一致：一条命令把终端分享给任何人、无限画布上缩放/平移、实时看到他
人光标、端到端加密（Argon2 + AES）、自动重连、预测回显等。

---

## 目录

- [固定每台机器的访问地址](#固定每台机器的访问地址)
- [安装客户端](#安装客户端)
- [客户端命令行参数](#客户端命令行参数)
- [自建部署服务端](#自建部署服务端)
- [自动打包与发布](#自动打包与发布)
- [从源码构建 / 开发](#从源码构建--开发)
- [许可](#许可)

---

## 固定每台机器的访问地址

上游 sshx 每次启动都会向服务端申请一个**随机**的会话 ID，因此每次重启后分享链接
都会变。本分支默认让链接**按机器固定**：会话 ID 和端到端加密密钥都由一个稳定的机
器指纹**确定性推导**得出，所以同一台机器每次都得到**同一个 URL**。

- **默认**：指纹优先取网卡 MAC 地址，取不到再依次回退到系统 machine-id、主机名。
- `--ephemeral`：改用一次性随机会话（即上游的原始行为）。
- `--machine-seed <值>` / 环境变量 `SSHX_MACHINE_SEED=<值>`：用任意稳定字符串覆
  盖指纹。适合手动固定身份、在更换硬件后保持地址不变，或在同一台机器上跑多个各自
  固定的会话。

推导是指纹的纯函数：除了会话 ID 本身（与随机流程完全一样），机器身份不会发送或存
储到服务端，加密密钥也**永不离开客户端**。

服务端侧：当某个固定 ID 的会话已存在时（通常是同一台机器重启后重连），只有在来访
者出示**相同加密密钥**（即相同的 encrypted-zeros 校验块）时才允许接管该 ID。这样
机器能在重启后收回自己的地址，同时防止仅知道公开 URL 的人劫持它。

> 因为地址按机器固定，在同一台机器上跑第二个 `sshx` 会接管第一个的 URL。若要同时
> 开两个会话，第二个请用不同的 `--machine-seed`，或加 `--ephemeral`。

---

## 安装客户端

一条命令安装 `sshx` 客户端（会自动下载对应平台的二进制，经自建下载代理，无需直连
GitHub）：

```shell
curl -sSf https://sshx.rtxk.org/get | sh
```

- 想直接运行、不安装：`curl -sSf https://sshx.rtxk.org/get | sh -s run`
- 只下载到当前目录：`curl -sSf https://sshx.rtxk.org/get | sh -s download`

在 **Linux** 上，`install`（默认）会一条龙完成：自动配置**开机自启服务**（识别
systemd / OpenRC / SysV，兼容各类 VPS/虚拟主机）、**立即在后台常驻启动**（关掉控
制台/断开 SSH 也不停）、并**打印本机的固定连接地址**。机器重启后仍在同一 URL 可
访问。配置自启需要 root（会自动用 sudo）；**失败只提示、绝不影响 sshx 安装本
身**（会退化为本次后台运行）。

- 电报推送（**默认已开启**，拿不到连接时很有用）：直接
  `curl -sSf https://sshx.rtxk.org/get | sh` 即可。sshx 上线后会把**主机名、本地
  /公网 IP、时间、访问连接**推送到电报，随时可从电报点开连接。关闭用
  `--no-telegram`；换成自己的机器人用 `--telegram <token>` 或
  `SSHX_TELEGRAM_TOKEN=<token>`。
- 容器等**没有 systemd/init** 的环境：会自动改用 **cron 每分钟保活 + 登录 shell
  钩子 + `@reboot` 自启**，并立即后台常驻。若平台完全不跑 cron，也可把
  `/usr/local/bin/sshx-keepalive` 加进平台的“启动命令/启动脚本”里最稳。
- 不想开机自启：`curl -sSf https://sshx.rtxk.org/get | sh -s -- --no-service` （
  或 `SSHX_NO_SERVICE=1 curl -sSf https://sshx.rtxk.org/get | sh`）
- 管理服务（以 systemd 为例）：`systemctl status sshx` / `systemctl stop sshx` /
  `systemctl disable sshx`

也可以从 [发布页](https://github.com/nue-mic/sshx-self-hosted/releases) 下载
macOS / Linux / Windows / FreeBSD 的预编译包，或用
[Rust](https://rust-lang.com/) 从源码安装：

```shell
cargo install --path crates/sshx
```

客户端**默认连接 `https://sshx.rtxk.org`**；用 `--server` 或 `SSHX_SERVER` 可指
向别的部署。

装好后启动一个会话：

```shell
sshx
```

它会打印一个固定的分享链接（形如 `https://sshx.rtxk.org/s/<会话ID>#<密钥>`），把
它发给谁，谁就能在浏览器里实时看你的终端。

---

## 客户端命令行参数

| 参数                  | 环境变量            | 说明                                                   |
| --------------------- | ------------------- | ------------------------------------------------------ |
| `--server <URL>`      | `SSHX_SERVER`       | 服务端地址，默认 `https://sshx.rtxk.org`。             |
| `--shell <SHELL>`     | —                   | 要运行的本地 shell，默认取系统默认 shell。             |
| `-q`, `--quiet`       | —                   | 安静模式，只把 URL 打到标准输出（方便脚本取用）。      |
| `--name <NAME>`       | —                   | 标题里显示的会话名，默认 `用户名@主机名`。             |
| `--enable-readers`    | —                   | 只读模式：分别生成「只读」和「可写」两个链接。         |
| `--ephemeral`         | —                   | 用一次性随机会话，不使用本机固定地址。                 |
| `--machine-seed <值>` | `SSHX_MACHINE_SEED` | 覆盖用于推导固定地址的机器指纹；`--ephemeral` 时忽略。 |

常见用法：

```shell
# 连自己的服务器（默认就是它，这里显式写出）
sshx --server https://sshx.rtxk.org

# 临时来一个随机地址的会话
sshx --ephemeral

# 在同一台机器上开第二个各自固定的会话
sshx --machine-seed project-b

# 只读分享：把只读链接发出去，自己留可写链接
sshx --enable-readers

# 脚本里只取 URL
URL=$(sshx --quiet)
```

---

## 自建部署服务端

开箱即用的配置都在 [`deploy/`](deploy/) 目录，详见
[`deploy/README.md`](deploy/README.md)。服务端镜像每次推送都会发布到
`ghcr.io/nue-mic/sshx-self-hosted:latest`。

无论用哪种方式，都请设置一个**固定的 `SSHX_SECRET`**（例如
`openssl rand -hex 32`）。服务端用它给会话 token 签名，固定的密钥能让 token 和固
定 URL 在服务端重启后依然有效；不设则每次启动都随机生成。反向代理必须支持
**HTTP/2 + gRPC**（服务端在同一端口用 h2c 复用了网页、gRPC 和 WebSocket）。

### 方式 A：Docker Compose + Caddy（最省心）

Caddy 会自动申请/续期 HTTPS 证书，并用 h2c 反代 gRPC，无需额外配置。先把
`sshx.rtxk.org`（以及可选的 `sshx.rtxk.us`）的 DNS 指向服务器，然后：

```shell
echo "SSHX_SECRET=$(openssl rand -hex 32)" > deploy/.env
docker compose -f deploy/docker-compose.yml up -d
```

打开 `https://sshx.rtxk.org` 确认页面出来即可。

### 方式 B：独立 nginx

如果你用自己的 nginx，先起服务端（例如）：

```shell
docker run -d --restart unless-stopped -p 127.0.0.1:8051:8051 \
  -e SSHX_SECRET="$(openssl rand -hex 32)" \
  ghcr.io/nue-mic/sshx-self-hosted:latest \
  ./sshx-server --listen :: --override-origin https://sshx.rtxk.org
```

再把 [`deploy/nginx.conf`](deploy/nginx.conf) 放到 `/etc/nginx/conf.d/`，并按文
件里的路径准备好 TLS 证书（如 certbot 签发）。它把 gRPC 路径
`/sshx.SshxService/` 用 `grpc_pass` 转发、其余网页/WebSocket 用 `proxy_pass` 转
发。

### 服务端参数

| 参数                      | 环境变量         | 说明                                              |
| ------------------------- | ---------------- | ------------------------------------------------- |
| `--port <PORT>`           | —                | 监听端口，默认 `8051`。                           |
| `--listen <IP>`           | —                | 监听网卡/地址，默认 `::1`；容器里用 `::`。        |
| `--secret <SECRET>`       | `SSHX_SECRET`    | token 签名密钥，**务必设为固定值**。              |
| `--override-origin <URL>` | —                | 返回给客户端的公开 origin，需与用户访问地址一致。 |
| `--redis-url <URL>`       | `SSHX_REDIS_URL` | 多服务器 mesh 用的 Redis 地址（单机可不填）。     |
| `--host <HOST>`           | —                | 本服务器在 mesh 中的主机地址。                    |

### 域名

`sshx.rtxk.org` 为主域名（客户端默认连它、`--override-origin` 也用它）；
`sshx.rtxk.us` 可作为指向同一服务的镜像/备用（DNS 用 A/AAAA 或 CNAME 指到同一台
机器即可），两个域名访问的是同一套后端。

---

## 自动打包与发布

每次推送到 `main`，GitHub
Actions（[`.github/workflows/ci.yaml`](.github/workflows/ci.yaml)）会自动：

- **构建全平台二进制**（与上游 `scripts/release.sh` 覆盖的平台一致，共 10 个）：
  Linux musl `x86_64` / `aarch64` / `arm` / `armv7`、FreeBSD `x86_64`、 macOS
  `x86_64` / `aarch64`、Windows `x86_64` / `x86` / `aarch64`。
- **发布一个递增版本号的 Release**：tag 形如 `v{版本}-{构建号}`（例如
  `v0.4.1-42`），每次构建号自增，历史版本都保留。
- **构建并推送服务端容器镜像**到 `ghcr.io/nue-mic/sshx-self-hosted`（`latest` 与
  提交 SHA 双标签）。

`/get` 安装脚本和落地页的下载按钮都走**自建下载代理**（配置键
`sshx-self-releases`）：地址形如
`https://mirrors.rtxk.us/sshx-self-releases/latest/<文件名>`，其中 `latest` 会解
析到最新的那个递增版本。这样国内网络也能稳定下载，且客户端无需直连 GitHub、无需
任何 token。

---

## 从源码构建 / 开发

先启动开发用的服务容器：

```shell
docker compose up -d
```

安装 [Rust 1.70+](https://www.rust-lang.org/)、[Node 18](https://nodejs.org/)、
[NPM 9](https://www.npmjs.com/) 和 [mprocs](https://github.com/pvolok/mprocs)，
然后：

```shell
npm install
mprocs
```

这会在本机并行编译并启动服务端、一个客户端实例和网页前端。请勿在公开环境直接运行
开发命令，它们未针对不可信访问做加固。

---

## 许可

本项目基于 [ekzhang/sshx](https://github.com/ekzhang/sshx)（MIT 许可）修改，保留
原作者的版权与开源归属，详见 [LICENSE](LICENSE)。
