# mihomo2singbox

将 mihomo / Clash 风格节点导出为 sing-box `outbounds`。

## 功能

- 读取 mihomo YAML 中的 `proxies`
- 读取 provider YAML 中的 `proxies`
- 读取一行一个的 URI 订阅
- 自动尝试解码 base64 订阅内容
- 生成 sing-box `outbounds`
- 可选生成 `Proxy` selector 和 `Auto` urltest
- 将 mihomo `dialer-proxy` 映射为 sing-box `detour`

## 已支持协议

- `ss`
- `vmess`
- `vless`
- `trojan`
- `socks5` / `socks`
- `http`
- `wireguard`
- `hysteria2` / `hy2`

## 示例

```bash
cargo run -- \
  --input ../../box/mihomo/config.yaml \
  --output ./outbounds.json
```

```bash
cargo run -- \
  --input ./subscription.txt \
  --include-urltest \
  --output ./outbounds-with-selector.json
```

```bash
cargo run -- sync-mihomo-providers \
  --config ../../box/mihomo/config.yaml \
  --state /data/adb/box/run/webui_subscriptions/mihomo.tsv
```
