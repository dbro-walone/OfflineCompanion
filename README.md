# 离线桌面陪伴助手（Rust 版）

这是 `rust-rewrite` 分支的 Windows 原生 Rust 实现。界面使用 Slint 编译为本地桌面 UI，不包含 WebView、浏览器运行时、账号、遥测或网络请求；本地业务使用 Rust，数据继续保存在 SQLite 与 JSON 中。

## 已实现

- 透明、无边框、置顶的原创桌宠“鸦影”，可交互区域之外支持点击穿透；支持点击反馈、拖动、75%～140% 缩放和右键菜单，菜单在窗口失去焦点后自动隐藏；
- 桌宠动画复用单张 Sprite Atlas，只切换图集裁剪区域，不为每一帧创建独立窗口或控件；
- 独立的待办、提醒、番茄钟、设置、扩展包与提醒气泡窗口；
- 待办新增、完成/恢复、按“显示已完成”筛选刷新、清理已完成项目和关联番茄钟；
- 一次性提醒、24 小时漏发扫描、延后 10 分钟、删除与列表刷新；
- 番茄钟开始、暂停、继续、停止、阶段切换，以及异常退出后的持久化恢复；
- 提醒触发时移动到当前活动显示器中央，并限制在显示器工作区；Windows 下使用原生窗口句柄定位显示器，枚举失败时提供安全 fallback；
- 基于 Windows 最后输入时间的久坐提醒；
- 深色/浅色主题、置顶、主动空闲动作、减少动态效果和久坐阈值等设置，浅色主题会同步 Slint 控件配色；
- SQLite WAL 与兼容原 C# 版本的数据表，番茄状态和设置均会持久化；升级到 Rust 版本时保留已有本地数据与原 C# 版设置；
- 安全 ZIP 扩展包导入：只接受 JSON、PNG、WebP、WAV、OGG 与 TXT，拒绝路径穿越、绝对路径、未知文件类型、单文件超过 100 MB 或总解压量超过 200 MB 的内容；
- 默认角色图集编译进 EXE，运行不需要外置资源目录。

## 技术与内存策略

- Rust stable（MSVC）+ Slint 1.17.1 原生桌面 UI；
- Slint 使用 `backend-winit`、`renderer-femtovg`、`raw-window-handle-06`、`unstable-winit-030` 与 `compat-1-2`，不加载 CLR/WPF；
- SQLite 使用 `rusqlite` 0.38 的 `bundled` 特性，将 SQLite 静态内置；
- Windows x64 目标启用静态 CRT；
- Release 使用 Thin LTO、`opt-level="s"`、`codegen-units=1`、`panic="abort"` 与 `strip="symbols"`；
- 动画复用单张图集并切换源裁剪区域，避免为每帧分配独立窗口或控件。

实际常驻内存仍需在目标 Windows 电脑上以任务管理器或 Process Explorer 测量；不同显卡驱动会影响 GPU 共享内存统计。

## Windows 构建

安装 stable Rust（`x86_64-pc-windows-msvc` 工具链）和 Visual Studio 2022 Build Tools 后执行：

```powershell
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
cargo build --release --locked --target x86_64-pc-windows-msvc
```

产物：

```text
target\x86_64-pc-windows-msvc\release\OfflineCompanion.exe
```

## GitHub Actions 成品

`.github/workflows/rust-release.yml` 在每次推送到 `rust-rewrite` 分支时触发 `Build Rust Desktop Releases`。其中的 Windows x64 任务执行格式检查、Clippy、测试和 Windows Release 编译，并上传：

```text
OfflineCompanion-rust-win-x64/
  OfflineCompanion.exe
  SHA256SUMS.txt
```

从 GitHub 仓库的 **Actions → Build Rust Desktop Releases → Artifacts** 下载 `OfflineCompanion-rust-win-x64` 即可在 Windows 10/11 x64 使用。

## 数据目录

Rust 版沿用原程序位置：

```text
%LocalAppData%\OfflineCompanion\
  data\companion.db
  config\settings.json
  packages\characters\
  packages\actions\
  logs\
  backups\
```

`companion.db` 保存待办、提醒与番茄状态，并使用 WAL 模式；`settings.json` 保存桌宠和界面设置。扩展包只允许 JSON、PNG、WebP、WAV、OGG 与 TXT，不接受 DLL、EXE、脚本、宏或其他未知类型。

## 分支说明

- `main`：旧版 .NET 8 / WPF 实现；
- `rust-rewrite`：当前 Rust + Slint 原生实现，本 README 描述的是此分支。
