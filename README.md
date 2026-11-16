# Classical Recording Studio · 古典录音工作台

整理现场录音和 CD 抓轨的桌面程序, 支持 Windows 与 macOS. 按时间点调用旁边的裁剪版 FFmpeg 分轨, 写入标签和封面, 并按场次放进专辑文件夹. 项目文件仍是 `.crstudio.json`. 程序本身是 MIT, 旁边的 FFmpeg 是单独执行的 GPL 程序.

**[下载 Windows 和 macOS 发行包](https://github.com/Tamako0626/Classical-Recording-Studio-/releases)**

## 使用

1. 从 Releases 下载对应 ZIP, 解压后运行 `ClassicalRecordingStudio`. 不要只拷走主程序, `ffmpeg` 和 `ffprobe` 必须留在同一目录.
2. 在「录音项目」里新建或打开项目. 工作台数据在本机应用目录, 不在项目文件里保存 API Key.
3. 「整场分轨」导入 WAV, FLAC, MP3, MKV 等录音, 填写起点和终点, 选择 FLAC, WAV, MP3 或 MKV, 以及 44100, 48000, 96000 Hz. MP3 不能用 96 kHz. 带封面的 WAV 会改成 FLAC.
4. 「CD 抓轨」选择音频和 CUE, 或粘贴带轨号的文字, 再导入当前项目. CUE 可读 UTF-8, UTF-16 和 GBK.
5. 「封面视频」用一张 PNG 或 JPG 做成 1920x1080 的 H.264. 自动模式下, 48 kHz 及以上的无损音轨使用 MKV, 其余使用 AAC MP4.
6. 「模型配置」可填写 OpenAI 兼容接口, 用来生成英文场次字段. Key 只留在本机.

程序只调用发行包里的 FFmpeg, 不使用系统 PATH 上的完整 FFmpeg. 封面只接受 PNG 和 JPG.

这一版没有 OCR, HEIC, PDF 和 PowerShell 脚本. 曲目别名只覆盖一小份常见作品, 其余可检索 MusicBrainz.

## 裁剪版 FFmpeg

sidecar 只保留这个程序用到的能力:

| 用途 | 组件 |
| --- | --- |
| 编码 | libx264, aac, flac, pcm_s16le, pcm_s24le, pcm_s32le, libmp3lame |
| 解码 | aac, alac, mp3, flac, vorbis, opus, PCM, ape, wavpack, png, mjpeg |
| 封装 | flac, wav, mp3, matroska, mp4, mov |
| 滤镜 | 高通, 低通, 均衡, 降噪, 响度, 淡入淡出, 限制器, 缩放和黑边 |

Windows 包由 MSVC 的 `cl` 编译, 不依赖 `msys-2.0.dll`. 第一次发应用标签之前, 需要先让 `slim ffmpeg` workflow 把 sidecar 传到 `sidecars` release.

## 源码

界面使用 [GPUI Kit](https://gpui-kit.com) 0.10. 编译在 GitHub Actions 上进行, 本仓库不要求在开发机安装 Rust 或 FFmpeg 工具链.

- `slim ffmpeg` 在 `main` 上改到 `scripts/build_ffmpeg.sh` 时编译 sidecar.
- `Windows` 和 `macOS` 在 `v*` 标签上编译程序, 下载 sidecar, 打成 ZIP.
