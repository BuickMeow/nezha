# Nezha — GPU 加速 MIDI 可视化器

[![en](https://img.shields.io/badge/lang-en-blue.svg)](README.md)

Nezha（哪吒）是一款基于 **Rust**、**wgpu** 和 **egui** 的跨平台 GPU 加速 MIDI 可视化工具。它能够实时渲染瀑布流 / 钢琴卷帘风格的动画，流畅处理数百万级音符。

![Rust](https://img.shields.io/badge/rust-2024%20edition-orange?logo=rust)
![wgpu](https://img.shields.io/badge/wgpu-29-blue)

---

## 特性

- **高性能渲染** — 基于 wgpu 的实例化绘制；CPU 端使用 Rayon 并行构建实例数据。
- **大规模 MIDI 支持** — 已测试 90 万+ 音符；通过 Seek Index 加速扫描，实现快速跳转。
- **双渲染模式** — `TimeBased`（基于真实时间滚动）与 `TickBased`（基于 MIDI Tick 对齐）。
- **丰富的文件支持** — `.mid` / `.midi`、`.dms`，以及压缩包（`.zip`、`.7z`、`.tar`、`.tar.gz`、`.tar.xz`）。
- **实时键盘叠加** — 实时显示按键激活状态，支持按音轨着色的调色板。
- **可自定义样式** — 调色板、圆角、边框、背景、等宽键 / 比例键宽。
- **性能分析** — 可选集成 `puffin`，支持帧级火焰图分析。

---

## 快速开始

需要 **Rust 1.85+** 以及支持 Vulkan / Metal / DX12 的 GPU。

```bash
# 运行桌面端 GUI
cargo run -p nezha-egui
```

然后通过 **文件 → 打开**（或拖拽）加载 MIDI、DMS 或压缩包文件。

---

## 项目结构

```
crates/
├── nezha-core/       # MIDI 解析（midly）、速度映射、Tick/时间换算
├── nezha-renderer/   # wgpu 渲染管线、实例构建器、着色器
│   ├── src/
│   │   ├── renderer.rs      # 主渲染逻辑 & CPU 实例构建
│   │   ├── pipeline.rs      # wgpu 管线状态与绑定组
│   │   ├── shader.wgsl      # 顶点 / 片段着色器（SDF 圆角矩形）
│   │   ├── keyboard.rs      # CPU 键位布局 & 键盘实例生成
│   │   ├── source.rs        # NoteSource trait（解耦渲染器与文件格式）
│   │   ├── state.rs         # 每帧可变状态（扫描索引等）
│   │   ├── style.rs         # RenderStyle、RenderMode、调色板配置
│   │   └── vertex.rs        # NoteInstance、Uniforms、GPU 类型打包
│   └── build.rs             # 通过 naga 校验着色器
├── nezha-egui/       # 桌面应用程序（eframe + egui）
│   └── src/
│       ├── app.rs            # 主应用壳层 & 文件加载
│       ├── config_panel.rs   # 渲染设置 UI
│       ├── piano_view.rs     # 钢琴键盘控件
│       ├── transport/        # 时间轴标尺、播放头、轨道、控制栏
│       └── render_context/   # wgpu 表面、MIDI 缓存、预览目标
├── nezha-archive/    # ZIP / 7Z / TAR 压缩包读取，支持 MIDI 文件过滤
└── nezha-dms/        # DMS 文件解析器 & SMF 转换器
```

---

## 架构亮点

### 渲染管线

1. **解析** — `nezha-core` 读取 SMF 或 DMS 文件，生成按 128 个 Key 分组的 `MidiFile`。
2. **索引** — `Renderer::upload_note_data` 构建每键 Seek Index（块前缀最大结束时间），实现 O(1) 跳过。
3. **构建** — 每帧由 CPU 并行扫描可见音符（Rayon 键组分块），输出 `NoteInstance`。
4. **绘制** — 每 600 万个实例一个 Draw Call；顶点着色器将实例扩展为四边形，并通过 SDF 绘制圆角。

### 关键优化

| 技术 | 效果 |
|---|---|
| 每键 Seek Index（256 音符分块） | 消除跳转/回退时的线性扫描 |
| 并行键组分块 | 按剩余音符权重均衡 Rayon 任务 |
| 动态实例缓冲区槽 | 按 2 的幂增长，跨帧复用 |
| 缓存键位布局 | 避免每帧重新计算黑白键几何 |
| 键盘脏标记 | 暂停时跳过键盘实例重建 |

---

## 性能分析

启用 `profiling` 特性可将帧作用域数据流式传输到 `puffin_viewer`：

```bash
# 终端 1：以分析模式运行应用
cargo run -p nezha-egui --features profiling

# 终端 2：打开火焰图查看器
cargo install puffin_viewer
puffin_viewer --url 127.0.0.1:8585
```

性能作用域（特性关闭时零开销）：
- `render` — 完整帧
- `scans` — CPU Seek Index 更新
- `keyboard` — CPU 键盘计算
- `render_pass` — GPU 渲染通道
- `upload_note_data` — 初始数据上传 / 索引构建

---

## 支持的输入格式

| 扩展名 | 说明 | Crate |
|---|---|---|
| `.mid`、`.midi` | 标准 MIDI 文件 | `nezha-core` |
| `.dms` | DMS 封装 MIDI | `nezha-dms` |
| `.zip` | ZIP 压缩包（随机访问） | `nezha-archive` |
| `.7z` | 7-Zip 压缩包 | `nezha-archive` |
| `.tar`、`.tar.gz`、`.tgz`、`.tar.xz`、`.txz` | TAR 压缩包 | `nezha-archive` |

压缩包会自动扫描其中的 `.mid` / `.midi` 条目；当包含多个文件时，GUI 会弹出选择器。

---

## 路线图

- [x] 键盘计算从 GPU Compute 移至 CPU（消除同步屏障）
- [x] 键盘脏标记（暂停时跳过重建）
- [x] 基于 Rayon 的并行键组实例构建
- [x] NoteSeekIndex 快速跳转
- [ ] 实例大小压缩：48 → 32 字节（打包颜色 + f16 属性）
- [ ] Workgroup 局部原子计数器（减少全局 atomicAdd 竞争）
- [ ] 异步计算重叠（双缓冲实例缓冲区）
- [ ] 片段着色器 LOD（小音符快速路径）

---

## 许可证

待定
