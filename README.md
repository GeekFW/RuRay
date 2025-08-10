# RuRay

<div align="center">

![RuRay Logo](ruray/public/tauri.svg)

**一个现代化的跨平台代理管理应用程序**

[![Tauri](https://img.shields.io/badge/Tauri-2.0-blue.svg)](https://tauri.app/)
[![Nuxt.js](https://img.shields.io/badge/Nuxt.js-3.0-green.svg)](https://nuxt.com/)
[![Rust](https://img.shields.io/badge/Rust-1.70+-orange.svg)](https://www.rust-lang.org/)
[![Vue.js](https://img.shields.io/badge/Vue.js-3.0-brightgreen.svg)](https://vuejs.org/)
[![License](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

**⚠️ 这是一个 Pre-Release 版本，仅供测试和开发使用**

</div>

## 📖 项目简介

RuRay 是一个基于 Tauri + Nuxt.js 构建的现代化跨平台代理管理应用程序。它提供了直观的用户界面来管理和配置各种代理服务器，支持多种代理协议，并具有实时状态监控功能。

### 🎯 核心功能

- **🖥️ 跨平台支持**: 支持 Windows、macOS 和 Linux
- **⚡ 高性能**: 基于 Rust 后端，提供极致性能
- **🎨 现代化 UI**: 使用 Vue 3 + Tailwind CSS 构建的响应式界面
- **🔧 配置管理**: 支持多种代理协议的配置和管理
- **📊 实时监控**: 服务器状态实时监控和日志查看
- **🌐 路由规则**: 灵活的路由规则配置
- **💾 配置持久化**: 自动保存和恢复配置

## 🚀 快速开始

### 环境要求

- **Node.js**: >= 18.0.0
- **pnpm**: >= 8.0.0
- **Rust**: >= 1.70.0
- **Tauri CLI**: >= 2.0.0

### 安装依赖

```bash
# 克隆项目
git clone https://github.com/your-username/RuRay.git
cd RuRay/ruray

# 安装前端依赖
pnpm install

# 安装 Tauri CLI (如果尚未安装)
pnpm add -g @tauri-apps/cli
```

### 开发模式

```bash
# 启动前端开发服务器
pnpm run dev

# 启动 Tauri 开发模式 (新终端)
pnpm run tauri dev
```

### 生产构建

```bash
# 构建应用程序
pnpm run tauri build
```

## 🛠️ 技术栈

### 前端
- **框架**: [Nuxt.js 3](https://nuxt.com/) - Vue.js 全栈框架
- **UI 库**: [Vue 3](https://vuejs.org/) + [Tailwind CSS](https://tailwindcss.com/)
- **构建工具**: [Vite](https://vitejs.dev/)
- **包管理**: [pnpm](https://pnpm.io/)

### 后端
- **语言**: [Rust](https://www.rust-lang.org/)
- **框架**: [Tauri](https://tauri.app/) - 跨平台桌面应用框架
- **序列化**: [Serde](https://serde.rs/) - JSON 序列化/反序列化

## ✨ 主要特性

### 🎨 用户界面
- **响应式设计**: 适配不同屏幕尺寸
- **暗色主题**: 现代化的暗色界面设计
- **组件化**: 模块化的 Vue 组件架构
- **实时更新**: 支持热重载开发

### 🔧 配置管理
- **多协议支持**: 支持多种代理协议配置
- **路由规则**: 灵活的流量路由配置
- **配置验证**: 实时配置验证和错误提示
- **导入导出**: 支持配置文件的导入和导出

### 📊 监控功能
- **服务器状态**: 实时显示服务器连接状态
- **日志查看**: 内置日志查看器
- **性能监控**: 连接速度和延迟监控
- **状态指示**: 直观的状态指示器

### 🛡️ 安全特性
- **配置加密**: 敏感配置信息加密存储
- **权限控制**: 最小权限原则
- **安全通信**: 加密的前后端通信

## 📁 项目结构

```
RuRay/
├── changelog/              # 版本变更文档
│   └── v1.md
├── ruray/                  # 主项目目录
│   ├── components/         # Vue 组件
│   │   ├── AppHeader.vue   # 应用头部
│   │   ├── ServerList.vue  # 服务器列表
│   │   ├── LogViewer.vue   # 日志查看器
│   │   └── ...
│   ├── src-tauri/          # Tauri 后端
│   │   ├── src/
│   │   │   ├── config.rs   # 配置管理
│   │   │   ├── main.rs     # 主程序入口
│   │   │   └── ...
│   │   ├── Cargo.toml      # Rust 依赖配置
│   │   └── tauri.conf.json # Tauri 配置
│   ├── package.json        # 前端依赖配置
│   ├── nuxt.config.ts      # Nuxt 配置
│   └── ...
└── README.md               # 项目说明文档
```

## 🔄 开发工作流

### 代码规范
- **Rust 代码**: 遵循 Rust 官方代码规范
- **Vue 代码**: 使用 ESLint + Prettier 格式化
- **提交规范**: 使用 Conventional Commits 规范

### 版本管理
- **语义化版本**: 遵循 SemVer 版本规范
- **变更日志**: 详细记录每个版本的变更内容
- **发布流程**: 自动化构建和发布流程

## 🐛 问题反馈

如果您在使用过程中遇到问题，请通过以下方式反馈：

1. **GitHub Issues**: [提交 Issue](https://github.com/your-username/RuRay/issues)
2. **功能请求**: [提交功能请求](https://github.com/your-username/RuRay/issues/new?template=feature_request.md)
3. **Bug 报告**: [提交 Bug 报告](https://github.com/your-username/RuRay/issues/new?template=bug_report.md)

## 🤝 贡献指南

我们欢迎社区贡献！请阅读 [贡献指南](CONTRIBUTING.md) 了解如何参与项目开发。

### 开发步骤
1. Fork 项目
2. 创建功能分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 创建 Pull Request

## 📄 许可证

本项目采用 MIT 许可证 - 查看 [LICENSE](LICENSE) 文件了解详情。

## 🙏 致谢

- [Tauri](https://tauri.app/) - 跨平台应用框架
- [Nuxt.js](https://nuxt.com/) - Vue.js 全栈框架
- [Tailwind CSS](https://tailwindcss.com/) - CSS 框架
- [Rust](https://www.rust-lang.org/) - 系统编程语言

## 📞 联系方式

- **作者**: Lander
- **项目**: RuRay
- **创建时间**: 2024年12月

---

<div align="center">

**⚠️ 重要提醒: 这是一个 Pre-Release 版本**

此版本仅供测试和开发使用，不建议在生产环境中使用。  
功能可能不完整，可能存在未知的 Bug 和安全问题。

**请在充分测试后再考虑用于正式环境！**

</div>