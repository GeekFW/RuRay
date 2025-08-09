# RuRay - Xray Core Desktop Client

一个基于 Tauri 2.0 + Nuxt + NuxtUI 开发的现代化 Xray Core 桌面客户端。

## 功能特性

- 🎨 **现代化 UI** - 基于 NuxtUI 的美观界面
- 🌙 **多主题支持** - 暗黑/明亮/极客绿三种主题
- 🪟 **窗口拖拽** - 支持无边框窗口拖拽
- 📌 **系统托盘** - Windows 任务栏图标和右键菜单
- 📋 **标准布局** - 服务器列表、日志、菜单栏、状态栏
- 🔗 **多协议支持** - VMess、VLESS、Trojan、Socks5、HTTP
- 🎯 **单窗口应用** - 简洁的单窗口设计
- ⚡ **极简模式** - 快速切换的极简界面
- 🌐 **代理模式** - 全局/PAC/不使用代理模式切换
- ⚙️ **系统代理** - 自动接管 Windows 代理设置
- 🚀 **启动动画** - 优雅的应用启动体验
- 🔄 **自动更新** - Xray Core 自动检查和更新
- 💾 **JSON 存储** - 代理配置以 JSON 格式存储
- 🔍 **连通性测试** - 服务器列表连通性测试
- 📊 **实时统计** - 网络速率和流量统计

## 技术栈

- **前端**: Nuxt 3 + NuxtUI + Vue 3 + TypeScript
- **后端**: Rust + Tauri 2.0
- **UI 框架**: Tailwind CSS + Headless UI
- **图标**: Heroicons + Lucide Icons

## 开发环境要求

- Node.js 18+
- pnpm
- Rust 1.70+
- Tauri CLI 2.0

## 安装和运行

1. 安装依赖：
```bash
pnpm install
```

2. 开发模式运行：
```bash
pnpm tauri dev
```

3. 构建应用：
```bash
pnpm tauri build
```

## 项目结构

```
ruray/
├── src-tauri/          # Rust 后端代码
│   ├── src/
│   │   ├── commands.rs # Tauri 命令定义
│   │   ├── config.rs   # 配置管理
│   │   ├── proxy.rs    # 代理管理
│   │   ├── system.rs   # 系统功能
│   │   ├── xray.rs     # Xray Core 管理
│   │   └── lib.rs      # 主入口
│   ├── Cargo.toml      # Rust 依赖配置
│   └── tauri.conf.json # Tauri 配置
├── components/         # Vue 组件
├── assets/            # 静态资源
├── nuxt.config.ts     # Nuxt 配置
├── package.json       # Node.js 依赖
└── README.md          # 项目说明
```

## 配置文件位置

- **Windows**: `%APPDATA%\RuRay\config.json`
- **macOS**: `~/Library/Application Support/RuRay/config.json`
- **Linux**: `~/.config/RuRay/config.json`

## 许可证

MIT License

## 作者

Lander

## 贡献

欢迎提交 Issue 和 Pull Request！
