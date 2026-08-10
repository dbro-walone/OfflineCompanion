# Pet Runtime v1 验收记录

## 自动化验收

| Issue #47 验收项 | 自动化证据 |
|---|---|
| AT-01 默认角色由 Catalog 加载 | `parses_v1_character_manifest`、`test_missing_action_falls_back_to_default` |
| AT-02 鼠标靠近、等待回应和超时 | `test_event_normalizer_deduplicates_pointer_near`、InteractionSession 三项测试 |
| AT-03 头部与身体点击区分 | `test_click_region_head_and_body_are_distinct` |
| AT-04 动作包真实播放 `idle.thinking` | `idle_tick_schedules_the_enabled_action_pack`、独立图集几何测试 |
| AT-05 关闭空闲动作 | `test_idle_action_is_disabled_by_setting`、`settings_reload_disables_idle_actions_immediately` |
| AT-06 提醒阻止低优先级空闲动作 | `test_idle_cannot_override_active_reminder` |
| AT-07 普通释放落地恢复 | `test_drop_enters_landing_then_idle` |
| AT-08 左右边缘释放 | `test_left_edge_selects_edge_left`、`test_right_edge_selects_edge_right` |
| AT-09 抛出、落地、受惊恢复 | `test_throw_enters_startled_recovery`、速度与位移上限测试 |
| AT-10 减少动态效果保留语义 | `test_reduce_motion_keeps_release_semantics_without_flight` |
| AT-11 动作包即时启用/禁用 | `action_pack_toggle_applies_without_restart_and_falls_back_safely` |
| AT-12 删除后安全回退 | 删除路径边界测试、Catalog 移除引用测试及 UI 回退逻辑 |
| AT-13 负坐标与热插拔越界恢复 | 工作区夹取测试、周期 DisplayChanged 夹取 |
| AT-14 纯离线 | 无网络客户端依赖；扩展包拒绝脚本、动态库与可执行资源 |

质量门禁：

```text
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets
cargo build --release --locked
```

GitHub Actions 在 Windows x64 和 macOS Universal 上重复格式、Clippy、全目标测试及 Release 构建，并上传带 SHA-256 的生产产物。

## 人工验收边界

当前开发环境为 macOS，不能把 CI 构建冒充 Windows 11 手工验收。Windows 11 x64 仍需使用流水线 EXE 逐项检查透明窗口、菜单空白处关闭、拖放三路径、多显示器热插拔、提醒平滑移动、番茄后台运行、浅色/深色主题和包管理操作。

72 小时稳定性也必须以真实经过时间记录，短时自动化不能替代。发布候选版本需要另行记录开始/结束时间、峰值内存、句柄数、CPU、提醒和番茄完成次数及异常日志；在该记录完成前，Issue #47 不应仅凭自动化测试关闭。
