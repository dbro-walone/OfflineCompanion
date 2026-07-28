# 离线桌面陪伴助手（Rust 版）

这是 `rust-rewrite` 分支的 Windows 原生 Rust 实现。界面使用 Slint 编译为本地桌面 UI，不包含 WebView、浏览器运行时、账号、遥测或网络请求；本地业务使用 Rust，数据继续保存在 SQLite 与 JSON 中。

## 已实现

- 透明、无边框、置顶的原创桌宠“鸦影”，支持 Sprite Atlas、点击反馈、拖动、缩放和右键菜单；
- 柔和圆角的待办、提醒、番茄钟、设置、扩展包与提醒气泡窗口；
- 提醒触发时移动到当前活动显示器中央，并将气泡限制在显示器工作区；
- 待办新增、完成/恢复、筛选、清理和关联番茄钟；
- 一次性提醒、24 小时漏发扫描与延后 10 分钟；
- 番茄钟开始、暂停、继续、停止、阶段切换和异常退出恢复；
- 基于 Windows 最后输入时间的久坐提醒；
- 深色/浅色主题、75%～140% 缩放、置顶、减少动态效果和久坐阈值设置；
- SQLite WAL 与兼容原 C# 版本的数据表，升级到 Rust 版本后保留本地待办、提醒和番茄记录；
- 安全 ZIP 扩展包导入：拒绝路径穿越、绝对路径、未知文件类型及超限解压内容；
- 默认角色图集编译进 EXE，运行不需要外置资源目录。

## 技术与内存策略

- Rust stable + Slint 原生桌面 UI；
- winit Windows 后端与 FemtoVG 渲染器，不加载 CLR/WPF；
- SQLite 使用 `rusqlite` 的静态内置 SQLite；
- Release 启用 Thin LTO、单代码生成单元、符号剥离、`panic=abort` 和静态 CRT；
- 动画复用单张图集，只切换源裁剪区域，不为每一帧创建独立窗口或控件。

实际常驻内存仍需在目标 Windows 电脑上以任务管理器或 Process Explorer 测量；不同显卡驱动会影响 GPU 共享内存统计。

## Windows 构建

安装 stable Rust（MSVC 工具链）和 Visual Studio 2022 Build Tools 后执行：

```powershell
cargo test --locked --all-targets
cargo build --release --locked --target x86_64-pc-windows-msvc
```

产物：

```text
target\x86_64-pc-windows-msvc\release\OfflineCompanion.exe
```

## GitHub Actions 成品

每次推送到 `rust-rewrite` 分支会触发 `Build Rust Windows Release`。流水线执行格式检查、Clippy、测试和 Windows Release 编译，并上传：

```text
OfflineCompanion-rust-win-x64/
  OfflineCompanion.exe
  SHA256SUMS.txt
```

从 GitHub 仓库的 **Actions → Build Rust Windows Release → Artifacts** 下载即可在 Windows 10/11 x64 使用。

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

扩展包只允许 JSON、PNG、WebP、WAV、OGG 与 TXT。禁止 DLL、EXE、脚本、宏或其他可执行内容。

## 分支说明

- `main`：原 .NET 8 / WPF 版本；
- `rust-rewrite`：Rust 原生版本。
