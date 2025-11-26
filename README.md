
# kovi-plugin-gif-lab

[<img alt="github" src="https://img.shields.io/badge/github-araea/kovi__plugin__gif__lab-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/araea/kovi-plugin-gif-lab)
[<img alt="crates.io" src="https://img.shields.io/crates/v/kovi-plugin-gif-lab.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/kovi-plugin-gif-lab)
[<img alt="docs.rs" src="https://img.shields.io/docsrs/kovi-plugin-gif-lab?style=for-the-badge&logo=docs.rs" height="20">](https://docs.rs/kovi-plugin-gif-lab)

Kovi 的 GIF 处理插件，支持合成、拆分、变速、倒放、旋转、拼图等操作。

## 特性

|           |                 |
|-----------|-----------------|
| 全能处理  | 变速、倒放、缩放、旋转、翻转 |
| 网格合成  | 九宫格/自定义网格 → 动态 GIF |
| 拆分转发  | GIF → 静帧，合并转发发送 |
| 智能拼图  | GIF 全帧 → 网格大图 |
| 信息查看  | 尺寸、帧数、时长、大小 |
| 零依赖    | 纯 Rust，轻量高效 |

## 安装

```sh
cargo kovi add gif-lab
```

在 `src/main.rs` 中添加：

```rust
kovi_plugin_gif_lab
```

## 快速开始

```text
# 查看帮助
gif帮助

# 加速播放（发送图片或引用）
gif变速 2.0

# 九宫格静图合成 GIF
合成gif 3x3

# 查看 GIF 每帧
gif拆分
```

> *指令不区分大小写*  
> `gif变速` = `GIF变速` = `Gif变速`

## 指令参考

### 帮助

| 指令           | 说明       |
|----------------|------------|
| `gif帮助`/`gifhelp` | 显示帮助信息 |

### 基础变换

| 指令                 | 说明       | 示例                  |
|----------------------|------------|-----------------------|
| `gif变速 <倍率>`     | 调整播放速度 | `gif变速 2` 加速，`gif变速 0.5` 减速 |
| `gif倒放`           | 倒序播放    | `gif倒放`             |
| `gif缩放 <倍率|尺寸>` | 调整大小   | `gif缩放 0.5` / `gif缩放 100x100`  |
| `gif信息`           | 查看参数    | `gif信息`             |

### 几何操作

| 指令                 | 说明       | 示例          |
|----------------------|------------|---------------|
| `gif旋转 <角度>`     | 旋转 (90/180/270/-90) | `gif旋转 90`  |
| `gif翻转 [方向]`     | 镜像翻转   | `gif翻转` 水平 / `gif翻转 垂直` |

> 翻转方向支持：`水平`、`垂直`、`h`、`v`、`horizontal`、`vertical`

### 合成与拆分

| 指令                     | 说明           | 示例                 |
|--------------------------|----------------|----------------------|
| `合成gif <行x列> [间隔] [边距]` | 网格图合成 GIF | `合成gif 3x3 0.1 0`  |
| `gif拼图 [列数]`          | GIF → 网格图  | `gif拼图` / `gif拼图 5` |
| `gif拆分`                | GIF → 静图 多张 (合并转发) | `gif拆分` |

**合成gif 参数说明：**

- `行x列` 必填，支持 `3x3`、`3*3`、`3×3` 格式  
- `间隔` 每帧间隔秒数，默认 0.1  
- `边距` 网格间隙像素，默认 0  

## FAQ

<details>
<summary><b>Q: 支持哪些图片格式？</b></summary>

- GIF 处理指令仅支持 GIF  
- `合成gif` 支持 JPG、PNG、GIF 等静态图  
</details>

<details>
<summary><b>Q: 为什么处理速度慢？</b></summary>

大尺寸或帧数多需更多时间。建议先用 `gif缩放` 缩小尺寸。
</details>

## 致谢

- [Kovi](https://kovi.threkork.com/) — Rust QQ 机器人框架  
- [image-rs](https://github.com/image-rs/image) — Rust 图像处理库  

<br>

#### License

<sup>
Licensed under either of <a href="LICENSE-APACHE">Apache License, Version
2.0</a> or <a href="LICENSE-MIT">MIT license</a> at your option.
</sup>

<br>

<sub>
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
</sub>
