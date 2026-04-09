# boxmanager

将 mihomo / Clash 风格节点导出为 sing-box `outbounds`，并提供订阅管理与模板构建能力。

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

---

## Sing-box 模板配置

模板文件 (`config.template.json`) 是一份完整的 sing-box 配置，在此基础上扩展了两个概念：
**订阅集合** (`outbound_providers`) 和 **模板 outbound 字段** (`include` / `filter` / `exclude_filter`)。

构建时，工具会解析订阅节点、展开模板字段、生成最终的 `config.json`。所有模板专用字段在构建产物中被移除，产物是一份合法的 sing-box 配置。

### 订阅集合 `outbound_providers`

定义节点的来源。放在模板根对象中，与 `outbounds` 同级。

```jsonc
"outbound_providers": [
  {
    "tag": "订阅1",         // 唯一名称，用于 outbound 中 include 引用
    "type": "remote",       // remote = 远程 URL  |  local = 本地文件
    "url": "https://example.com/sub?token=xxx",
    "update_time": ""       // 上次更新时间（ISO 8601），空串表示未更新
  },
  {
    "tag": "订阅2",
    "type": "local",
    "url": "/path/to/local/nodes.txt",
    "update_time": ""
  }
]
```

| 字段 | 类型 | 必填 | 说明 |
|:---|:---|:---:|:---|
| `tag` | string | ✅ | 订阅集合的唯一标签名 |
| `type` | string | ✅ | `remote`（远程 URL）或 `local`（本地文件） |
| `url` | string | ✅ | 远程订阅地址或本地文件路径 |
| `update_time` | string | | 上次更新时间，ISO 8601 格式，空串表示尚未同步 |

### 模板 outbound 扩展字段

在 `outbounds` 中的 `selector` 或 `urltest` 类型条目上，可以使用以下模板字段来自动注入订阅节点：

| 字段 | 类型 | 说明 |
|:---|:---|:---|
| `include` | string[] | 引用的订阅集合 `tag` 列表。省略则包含**所有** provider 的节点 |
| `filter` | string | 正则表达式，**保留**匹配的节点 |
| `exclude_filter` | string | 正则表达式，**排除**匹配的节点 |

#### 构建逻辑

1. 确定节点来源：`include` 指定的 provider，省略则为全部 provider
2. 收集这些 provider 中的所有节点 tag
3. 应用 `filter`（保留匹配）→ 应用 `exclude_filter`（排除匹配）
4. 将过滤后的节点 tag **追加**到已有的 `outbounds` 数组中
5. 从 outbound 对象中**移除** `include`、`filter`、`exclude_filter` 字段

> 如果该 outbound 已有静态 `outbounds`（如子组引用），动态节点追加在其后面。

### 示例

```jsonc
"outbounds": [
  // ① 纯静态 outbound —— 不含模板字段，构建时原样保留
  {
    "tag": "手动选择",
    "type": "selector",
    "outbounds": ["亚洲", "美国", "台湾"],
    "default": "亚洲"
  },

  // ② 仅从指定订阅取节点 + 正则过滤
  {
    "tag": "亚洲",
    "type": "urltest",
    "include": ["订阅1", "订阅2"],                        // 只用这两个订阅
    "filter": "香港|日本|东京|新加坡|狮城|Japan|Singapore", // 保留亚洲节点
    "url": "https://www.gstatic.com/generate_204",
    "interval": "10m"
  },
  // → 构建后 outbounds = [匹配的亚洲节点...]

  // ③ 排除过滤：从所有订阅中获取、排除指定节点
  {
    "tag": "其他",
    "type": "selector",
    "exclude_filter": "日本|香港|新加坡|美国"              // 排除主流地区
  },
  // → 构建后 outbounds = [不匹配排除规则的节点...]

  // ④ 静态引用 + 动态节点混合
  {
    "tag": "ChatGPT",
    "type": "selector",
    "outbounds": ["台湾"],         // 静态：引用"台湾"子组
    "include": ["订阅1"]           // 动态：追加订阅1的全部节点
  },
  // → 构建后 outbounds = ["台湾", ...订阅1的所有节点]

  // ⑤ 仅从单个订阅取全部节点（不过滤）
  {
    "tag": "自用节点",
    "type": "urltest",
    "include": ["订阅1"]
  }
  // → 构建后 outbounds = [订阅1 的所有节点]
]
```

---

## CLI 用法

### 基础：节点转换

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

### Mihomo 订阅管理

```bash
# 同步订阅状态
cargo run -- sync-mihomo-providers \
  --config ../../box/mihomo/config.yaml \
  --state /data/adb/box/run/webui_subscriptions/mihomo.tsv
```

### Sing-box 订阅管理

```bash
# 列出所有订阅
cargo run -- list-singbox-subscriptions \
  --config ../../box/sing-box/config.template.json

# 添加/更新订阅
cargo run -- set-singbox-subscription \
  --config ../../box/sing-box/config.template.json \
  --name "订阅1" \
  --url "https://example.com/sub"

# 删除订阅
cargo run -- remove-singbox-subscription \
  --config ../../box/sing-box/config.template.json \
  --name "订阅1"
```

### Sing-box 模板构建

```bash
# 从模板 + 订阅缓存 → 生成最终 config.json
cargo run -- build-singbox-config \
  --config ../../box/sing-box/config.template.json \
  --subscriptions-dir /data/adb/box/run/singbox_subscriptions \
  --output ../../box/sing-box/config.json

# 检查订阅缓存状态
cargo run -- inspect-singbox-subscriptions \
  --config ../../box/sing-box/config.template.json \
  --subscriptions-dir /data/adb/box/run/singbox_subscriptions
```
