# 角色与动作包制作指南

扩展包是 ZIP 文件，根目录必须包含 `manifest.json`。支持 `schemaVersion` 1 和 2；包类型为 `character` 或 `action`。版本与引擎约束使用 SemVer。安装器限制单文件 100 MB、总解压量 200 MB，只接受 JSON、PNG、WebP、WAV、OGG 和 TXT。

角色包至少声明 `id`、`name`、`version`、`engineVersion`、`defaultAction`、帧尺寸和动作映射。动作包还要声明兼容角色及版本范围。动画 JSON 声明图集、FPS、播放模式、打断策略、恢复策略和 entry/loop/exit 分段；分段重复次数最多 100，所有帧编号必须在图集范围内。

允许动画文件通过 `../atlases/name.png` 引用同一个包内的图集，但解析后的路径不能离开包目录。绝对路径、路径穿越、EXE、DLL、脚本、宏和动态库都会被拒绝。运行时不执行扩展包中的任何代码。

安装采用临时目录校验和原子替换。校验失败时旧版本保持可用；卸载动作包后，正在引用的动作会安全回退到角色默认动作。
