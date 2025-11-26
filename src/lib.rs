//! kovi-plugin-gif-lab
//!
//! 一个全能的 GIF 处理实验室插件。
//! 提供 GIF 合成、拆分、变速、倒放、旋转、缩放等功能。

// =============================
//          Modules
// =============================

mod utils {
    use kovi::MsgEvent;
    use regex::Regex;
    use std::sync::OnceLock;

    /// 提取消息中的图片 URL (支持直接发送、引用回复)
    pub async fn get_image_url(
        event: &std::sync::Arc<MsgEvent>,
        bot: &std::sync::Arc<kovi::RuntimeBot>,
    ) -> Option<String> {
        // 1. 检查当前消息
        for seg in event.message.iter() {
            if seg.type_ == "image"
                && let Some(url) = seg.data.get("url").and_then(|u| u.as_str())
            {
                return Some(url.to_string());
            }
        }

        // 2. 检查引用消息
        let reply_id = event.message.iter().find_map(|seg| {
            if seg.type_ == "reply" {
                seg.data.get("id").and_then(|v| v.as_str())
            } else {
                None
            }
        })?;

        if let Ok(reply_id_int) = reply_id.parse::<i32>()
            && let Ok(msg_res) = bot.get_msg(reply_id_int).await
            && let Some(segments) = msg_res.data.get("message").and_then(|v| v.as_array())
        {
            for seg in segments {
                if let Some(type_) = seg.get("type").and_then(|t| t.as_str())
                    && type_ == "image"
                    && let Some(url) = seg
                        .get("data")
                        .and_then(|d| d.get("url"))
                        .and_then(|u| u.as_str())
                {
                    return Some(url.to_string());
                }
            }
        }
        None
    }

    /// 下载图片
    pub async fn download_image(url: &str) -> anyhow::Result<bytes::Bytes> {
        let resp = reqwest::get(url).await?;
        let bytes = resp.bytes().await?;
        Ok(bytes)
    }

    /// 解析 "3x3" 或 "3*3" 或 "3×3" 等格式 (大小写不敏感)
    pub fn parse_grid_dim(s: &str) -> Option<(u32, u32)> {
        static RE: OnceLock<Regex> = OnceLock::new();
        let re = RE.get_or_init(|| Regex::new(r"(?i)(\d+)\s*[xX*×]\s*(\d+)").unwrap());
        re.captures(s).and_then(|caps| {
            let r = caps[1].parse().ok().filter(|&v| v > 0)?;
            let c = caps[2].parse().ok().filter(|&v| v > 0)?;
            Some((r, c))
        })
    }

    pub fn format_size(bytes: usize) -> String {
        const KB: f64 = 1024.0;
        const MB: f64 = KB * 1024.0;
        if bytes as f64 >= MB {
            format!("{:.2} MB", bytes as f64 / MB)
        } else {
            format!("{:.2} KB", bytes as f64 / KB)
        }
    }
}

mod gif_ops {
    use anyhow::{Result, anyhow};
    use base64::{Engine as _, engine::general_purpose};
    use image::{
        AnimationDecoder, DynamicImage, Frame, GenericImageView, ImageBuffer,
        codecs::gif::{GifDecoder, GifEncoder, Repeat},
        imageops,
    };
    use std::io::Cursor;
    use std::time::Duration;

    /// 合成 GIF (网格图 -> 动图)
    pub fn grid_to_gif(
        img_bytes: bytes::Bytes,
        rows: u32,
        cols: u32,
        interval_secs: f64,
        margin: u32,
    ) -> Result<String> {
        let img = image::load_from_memory(&img_bytes)?;
        let (width, height) = img.dimensions();

        // 计算单个切片的尺寸 (考虑边距)
        let tile_width = if cols > 1 {
            (width.saturating_sub((cols - 1) * margin)) / cols
        } else {
            width
        };
        let tile_height = if rows > 1 {
            (height.saturating_sub((rows - 1) * margin)) / rows
        } else {
            height
        };

        if tile_width == 0 || tile_height == 0 {
            return Err(anyhow!("图片尺寸太小或边距过大，无法分割"));
        }

        let delay = image::Delay::from_saturating_duration(Duration::from_secs_f64(interval_secs));
        let mut frames = Vec::with_capacity((rows * cols) as usize);

        for r in 0..rows {
            for c in 0..cols {
                let x = c * (tile_width + margin);
                let y = r * (tile_height + margin);

                if x + tile_width > width || y + tile_height > height {
                    continue;
                }

                let sub_img = img.view(x, y, tile_width, tile_height).to_image();
                frames.push(Frame::from_parts(sub_img, 0, 0, delay));
            }
        }

        if frames.is_empty() {
            return Err(anyhow!("无法生成任何帧，请检查参数"));
        }

        encode_frames_to_b64(frames)
    }

    /// GIF 拼图 (动图 -> 网格图)
    pub fn gif_to_grid(img_bytes: bytes::Bytes, cols_opt: Option<u32>) -> Result<String> {
        let decoder = GifDecoder::new(Cursor::new(img_bytes))?;
        let frames: Vec<Frame> = decoder.into_frames().collect_frames()?;

        if frames.is_empty() {
            return Err(anyhow!("GIF 没有帧"));
        }

        let count = frames.len() as u32;
        let (frame_w, frame_h) = frames[0].buffer().dimensions();

        let cols = cols_opt
            .unwrap_or_else(|| (count as f64).sqrt().ceil() as u32)
            .max(1);
        let rows = count.div_ceil(cols);

        let total_w = frame_w * cols;
        let total_h = frame_h * rows;

        let mut canvas = ImageBuffer::new(total_w, total_h);

        for (i, frame) in frames.iter().enumerate() {
            let c = (i as u32) % cols;
            let r = (i as u32) / cols;
            image::imageops::overlay(
                &mut canvas,
                frame.buffer(),
                (c * frame_w) as i64,
                (r * frame_h) as i64,
            );
        }

        let mut buffer = Cursor::new(Vec::new());
        canvas.write_to(&mut buffer, image::ImageFormat::Png)?;
        Ok(general_purpose::STANDARD.encode(buffer.get_ref()))
    }

    /// GIF 拆分 (返回 base64 列表)
    pub fn gif_to_frames(img_bytes: bytes::Bytes) -> Result<Vec<String>> {
        let decoder = GifDecoder::new(Cursor::new(img_bytes))?;
        let frames = decoder.into_frames().collect_frames()?;

        frames
            .into_iter()
            .map(|frame| {
                let mut buffer = Cursor::new(Vec::new());
                DynamicImage::ImageRgba8(frame.into_buffer())
                    .write_to(&mut buffer, image::ImageFormat::Png)?;
                Ok(general_purpose::STANDARD.encode(buffer.get_ref()))
            })
            .collect()
    }

    /// GIF 信息
    pub fn gif_info(img_bytes: bytes::Bytes) -> Result<String> {
        let len = img_bytes.len();
        let decoder = GifDecoder::new(Cursor::new(&img_bytes))?;
        let frames = decoder.into_frames().collect_frames()?;

        if frames.is_empty() {
            return Err(anyhow!("无效 GIF"));
        }

        let (w, h) = frames[0].buffer().dimensions();
        let count = frames.len();

        // 计算总时长 (将 Delay 转换为 Duration)
        let duration_ms: u128 = frames
            .iter()
            .map(|f| Duration::from(f.delay()).as_millis())
            .sum();

        Ok(format!(
            "📏 尺寸: {}x{}\n🎞️ 帧数: {}\n⏱️ 时长: {:.2}s\n💾 大小: {}",
            w,
            h,
            count,
            duration_ms as f64 / 1000.0,
            super::utils::format_size(len)
        ))
    }

    /// GIF 变换类型
    pub enum Transform {
        Speed(f64),
        Reverse,
        Resize(u32, u32),
        Scale(f64),
        Rotate(i32),
        FlipH,
        FlipV,
    }

    pub fn process_gif(img_bytes: bytes::Bytes, op: Transform) -> Result<String> {
        let decoder = GifDecoder::new(Cursor::new(img_bytes))?;
        let mut frames = decoder.into_frames().collect_frames()?;

        if frames.is_empty() {
            return Err(anyhow!("GIF 解码失败或无帧"));
        }

        let (orig_w, orig_h) = frames[0].buffer().dimensions();

        match op {
            Transform::Speed(factor) => {
                if factor <= 0.0 {
                    return Err(anyhow!("倍率必须大于 0"));
                }
                for frame in &mut frames {
                    let old_ms = Duration::from(frame.delay()).as_millis() as f64;
                    let new_ms = (old_ms / factor).max(10.0) as u64;
                    let new_delay =
                        image::Delay::from_saturating_duration(Duration::from_millis(new_ms));
                    *frame = Frame::from_parts(
                        frame.buffer().clone(),
                        frame.left(),
                        frame.top(),
                        new_delay,
                    );
                }
            }
            Transform::Reverse => {
                frames.reverse();
            }
            Transform::Resize(w, h) => {
                frames = transform_frames(frames, |img| {
                    img.resize_exact(w, h, imageops::FilterType::Lanczos3)
                });
            }
            Transform::Scale(s) => {
                let target_w = ((orig_w as f64 * s) as u32).max(1);
                let target_h = ((orig_h as f64 * s) as u32).max(1);
                frames = transform_frames(frames, |img| {
                    img.resize_exact(target_w, target_h, imageops::FilterType::Lanczos3)
                });
            }
            Transform::Rotate(deg) => {
                frames = transform_frames(frames, |img| match deg.rem_euclid(360) {
                    90 => img.rotate90(),
                    180 => img.rotate180(),
                    270 => img.rotate270(),
                    _ => img,
                });
            }
            Transform::FlipH => {
                frames = transform_frames(frames, |img| img.fliph());
            }
            Transform::FlipV => {
                frames = transform_frames(frames, |img| img.flipv());
            }
        }

        encode_frames_to_b64(frames)
    }

    /// 统一的帧变换辅助函数
    fn transform_frames<F>(frames: Vec<Frame>, transform: F) -> Vec<Frame>
    where
        F: Fn(DynamicImage) -> DynamicImage,
    {
        frames
            .into_iter()
            .map(|frame| {
                let delay = frame.delay();
                let img = DynamicImage::ImageRgba8(frame.into_buffer());
                Frame::from_parts(transform(img).into_rgba8(), 0, 0, delay)
            })
            .collect()
    }

    fn encode_frames_to_b64(frames: Vec<Frame>) -> Result<String> {
        let mut buffer = Cursor::new(Vec::new());
        {
            let mut encoder = GifEncoder::new(&mut buffer);
            encoder.set_repeat(Repeat::Infinite)?;
            encoder.encode_frames(frames.into_iter())?;
        }
        Ok(general_purpose::STANDARD.encode(buffer.get_ref()))
    }
}

// =============================
//      Main Plugin Logic
// =============================

use kovi::{Message, PluginBuilder, bot::message::Segment, serde_json::json};
use kovi_plugin_expand_napcat::NapCatApi;
use std::sync::Arc;

/// 帮助信息
const HELP_TEXT: &str = r#"🎬 GIF 实验室 - 帮助

📝 指令列表 (大小写均可):

• gif帮助 / gifhelp - 显示本帮助
• 合成gif [行x列] [间隔秒] [边距]
    将网格图合成为动图
    示例: 合成gif 3x3 0.1 0
• gif拼图 [列数] - 将动图转为网格图
• gif拆分 - 将动图拆成多张静态图
• gif变速 [倍率] - 调整播放速度
    示例: gif变速 2 (加速2倍)
• gif倒放 - 倒序播放
• gif缩放 [倍率|尺寸]
    示例: gif缩放 0.5 或 gif缩放 100x100
• gif旋转 [角度] - 旋转 (90/180/270/-90)
• gif翻转 [水平|垂直] - 镜像翻转
• gif信息 - 查看 GIF 详情

💡 使用时请附带图片或引用图片消息"#;

/// 支持的指令 (统一小写存储)
const COMMANDS: &[&str] = &[
    "gif帮助",
    "gifhelp",
    "合成gif",
    "gif变速",
    "gif倒放",
    "gif信息",
    "gif缩放",
    "gif旋转",
    "gif翻转",
    "gif拆分",
    "gif拼图",
];

/// 检查是否匹配指令（忽略大小写）
fn match_command(input: &str) -> Option<&'static str> {
    let input_lower = input.to_lowercase();
    COMMANDS.iter().find(|&&cmd| cmd == input_lower).copied()
}

/// 需要图片的指令
fn requires_image(cmd: &str) -> bool {
    !matches!(cmd, "gif帮助" | "gifhelp")
}

#[kovi::plugin]
async fn main() {
    let bot = PluginBuilder::get_runtime_bot();

    PluginBuilder::on_msg(move |event| {
        let bot = bot.clone();
        async move {
            let text = match event.borrow_text() {
                Some(t) => t.trim(),
                None => return,
            };

            let parts: Vec<&str> = text.split_whitespace().collect();
            if parts.is_empty() {
                return;
            }

            // 匹配指令（忽略大小写）
            let cmd = match match_command(parts[0]) {
                Some(c) => c,
                None => return,
            };
            let args = &parts[1..];

            // 帮助指令
            if matches!(cmd, "gif帮助" | "gifhelp") {
                event.reply(HELP_TEXT);
                return;
            }

            // 获取图片
            let img_url = match utils::get_image_url(&event, &bot).await {
                Some(u) => u,
                None if requires_image(cmd) => {
                    event.reply("❌ 请附带图片或引用图片消息");
                    return;
                }
                None => return,
            };

            event.reply("⏳ 处理中...");

            let img_bytes = match utils::download_image(&img_url).await {
                Ok(b) => b,
                Err(e) => {
                    event.reply(format!("❌ 图片下载失败: {}", e));
                    return;
                }
            };

            // 处理逻辑分发
            let res: Result<Option<String>, anyhow::Error> = match cmd {
                "合成gif" => {
                    let (rows, cols) = args
                        .first()
                        .and_then(|s| utils::parse_grid_dim(s))
                        .unwrap_or((3, 3));
                    let interval = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(0.1);
                    let margin = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
                    gif_ops::grid_to_gif(img_bytes, rows, cols, interval, margin).map(Some)
                }
                "gif变速" => {
                    let factor = args.first().and_then(|s| s.parse().ok()).unwrap_or(2.0);
                    gif_ops::process_gif(img_bytes, gif_ops::Transform::Speed(factor)).map(Some)
                }
                "gif倒放" => {
                    gif_ops::process_gif(img_bytes, gif_ops::Transform::Reverse).map(Some)
                }
                "gif信息" => match gif_ops::gif_info(img_bytes) {
                    Ok(info) => {
                        event.reply(info);
                        Ok(None)
                    }
                    Err(e) => Err(e),
                },
                "gif缩放" => {
                    let op = args.first().map_or(gif_ops::Transform::Scale(0.5), |s| {
                        if let Some((w, h)) = utils::parse_grid_dim(s) {
                            gif_ops::Transform::Resize(w, h)
                        } else {
                            gif_ops::Transform::Scale(s.parse().unwrap_or(0.5))
                        }
                    });
                    gif_ops::process_gif(img_bytes, op).map(Some)
                }
                "gif旋转" => {
                    let deg = args.first().and_then(|s| s.parse().ok()).unwrap_or(90);
                    gif_ops::process_gif(img_bytes, gif_ops::Transform::Rotate(deg)).map(Some)
                }
                "gif翻转" => {
                    let op = args.first().map(|s| s.to_lowercase()).as_deref().map_or(
                        gif_ops::Transform::FlipH,
                        |s| {
                            if matches!(s, "垂直" | "v" | "vertical" | "纵向") {
                                gif_ops::Transform::FlipV
                            } else {
                                gif_ops::Transform::FlipH
                            }
                        },
                    );
                    gif_ops::process_gif(img_bytes, op).map(Some)
                }
                "gif拼图" => {
                    let cols = args.first().and_then(|s| s.parse().ok());
                    gif_ops::gif_to_grid(img_bytes, cols).map(Some)
                }
                "gif拆分" => match gif_ops::gif_to_frames(img_bytes) {
                    Ok(list) => {
                        send_forward_msg(&bot, &event, list).await;
                        Ok(None)
                    }
                    Err(e) => Err(e),
                },
                _ => Ok(None),
            };

            match res {
                Ok(Some(b64)) => {
                    event.reply(Message::new().add_image(&format!("base64://{}", b64)));
                }
                Ok(None) => {}
                Err(e) => {
                    event.reply(format!("❌ 处理失败: {}", e));
                }
            }
        }
    });
}

/// 发送合并转发消息
async fn send_forward_msg(
    bot: &Arc<kovi::RuntimeBot>,
    event: &Arc<kovi::MsgEvent>,
    base64_list: Vec<String>,
) {
    let bot_info = bot.get_login_info().await.ok();
    let (bot_id, bot_name) = bot_info
        .map(|info| {
            (
                info.data
                    .get("user_id")
                    .and_then(|u| u.as_str())
                    .unwrap_or("0")
                    .to_string(),
                info.data
                    .get("nickname")
                    .and_then(|n| n.as_str())
                    .unwrap_or("Bot")
                    .to_string(),
            )
        })
        .unwrap_or_else(|| ("0".to_string(), "Bot".to_string()));

    let mut nodes: Vec<_> = base64_list
        .into_iter()
        .map(|b64| {
            Segment::new(
                "node",
                json!({
                    "name": bot_name,
                    "uin": bot_id,
                    "content": [{
                        "type": "image",
                        "data": { "file": format!("base64://{}", b64) }
                    }]
                }),
            )
        })
        .collect();

    if nodes.len() > 99 {
        nodes.truncate(99);
        event.reply("⚠️ 帧数过多，仅发送前 99 帧");
    }

    if let Some(group_id) = event.group_id {
        let _ = bot.send_group_forward_msg(group_id, nodes).await;
    } else {
        let _ = bot.send_private_forward_msg(event.user_id, nodes).await;
    }
}
