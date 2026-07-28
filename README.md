# 离线桌面陪伴助手

基于 .NET 8 与 WPF 的 Windows 11 x64 原生桌面应用。应用完全离线运行，以原创角色“鸦影”提供桌面陪伴，并集成待办、定时提醒、番茄钟和久坐提醒。

## 已实现

- 透明、无边框、置顶的 WPF 桌宠窗口；
- PerMonitorV2 DPI 清单、多显示器工作区约束与边缘检测；
- 逐帧 Sprite Atlas 播放器和声明式动画定义；
- 行为状态机、P0～P8 优先级、动作选择与恢复策略；
- 待办新增、完成/恢复、SQLite 持久化；
- 一次性定时提醒、漏发窗口扫描和提醒气泡；
- 番茄钟开始、暂停、继续和异常退出恢复；
- 久坐状态计算、休息重置、延后次数和当日静默规则；
- JSON 设置的临时文件写入、原子替换和备份回退；
- 角色包/动作包扫描、严格 Manifest 解析和安装界面；
- ZIP 路径穿越、符号链接、文件类型和解压大小防护；
- 默认原创角色“鸦影”作为普通角色包安装，无角色专属引擎代码；
- 领域单元测试和 SQLite/恶意 ZIP 集成测试。

## 工程结构

```text
src/
  Companion.App/             WPF 启动、依赖注入与窗口
  Companion.Presentation/    ViewModel、命令与 Sprite 控件
  Companion.Application/     待办、提醒、番茄钟、久坐用例服务
  Companion.Domain/          实体、状态机、优先级和调度规则
  Companion.Packages/        Manifest、校验器和安全 ZIP 安装
  Companion.Infrastructure/  SQLite、JSON、日志、时钟与系统活动
packages/
  characters/shadow-crow-ninja/
  actions/shadow-crow-office/
tests/
  Companion.UnitTests/
  Companion.IntegrationTests/
  Companion.UiTests/
```

## 构建与运行

需要 Windows 11 x64、Visual Studio 2022 17.8+ 或 .NET 8 SDK。

```powershell
dotnet restore OfflineCompanion.sln
dotnet test OfflineCompanion.sln -c Release
dotnet run --project src/Companion.App/Companion.App.csproj
```

应用数据写入：

```text
%LocalAppData%\OfflineCompanion\
  data\companion.db
  config\settings.json
  packages\characters\
  packages\actions\
  cache\atlases\
  logs\
  backups\
```

运行时不会创建 `HttpClient`、加载远程 WebView、发送遥测或上传崩溃报告。

## 扩展包边界

扩展包只允许 JSON、PNG、WebP、WAV、OGG 与 TXT。禁止 DLL、EXE、BAT、CMD、PS1、JS、Python、宏和其他可执行内容。安装始终先解压到临时目录，通过验证后再原子移动到正式目录。

默认角色素材是为本项目生成的原创占位素材，仅供个人学习和非商业使用；公开发布前请重新确认美术素材许可。

## 尚需 Windows 实机完成的发布门槛

- 多显示器负坐标、DPI 热切换和显示器热插拔矩阵；
- 微软拼音与搜狗输入法候选窗交互；
- 72 小时长稳、空闲 CPU、常驻内存和冷启动基线；
- 锁屏、睡眠/恢复、全屏独占与 UAC 场景；
- MSIX/安装包与开机启动集成。

这些项目依赖 Windows 系统能力，不能在当前 macOS 工作区内完成实机验收。
