//! Oodle(Kraken) 解码薄封装。
//!
//! 仅负责**解码**：把 Palworld `.sav` 中 `PlM`（Oodle/Kraken, save_type=49）的压缩数据段
//! 还原成 GVAS 原始字节。编码（写回）不在本模块处理——按 R11(修订) 统一降级为 PLZ(zlib)，
//! 由 `sav_io.rs::encode_payload` 的 `Plm => PLZ` 分支负责。
//!
//! 解码器：[`oozextract`](oozextract)（MIT，纯 Rust，是开源 `ooz` 的 Rust 移植，与参考
//! `PalworldSaveTools` 的 `palooz` 同宗同构；参考 `oozlib.OozLib.decompress` 调
//! `palooz.decompress(compressed_data, uncompressed_len)`）。
//!
//! ## 真实 API（v0.5.4，已读 vendored 源码 `src/ooz/mod.rs` 确认，非假设）
//! - 没有 free function `decompress(...)`。
//! - 公开导出 `Extractor` 与 `OozError`。
//! - 用法：`let mut ex = Extractor::new(); ex.read_from_slice(input, &mut output)?`
//!   - `input` 为**原始 Kraken 比特流**（Palworld 中即 `.sav` 的 `[12 .. 12+compressed_len]`，
//!     首字节为 `0x8C` 形式的块头，无额外 8 字节长度前缀）。
//!   - `output` 须预先分配为恰好 `uncompressed_len` 大小；返回实际写出字节数。
//! - 注意：Kraken 不像 zlib 会忽略尾部，传入的 `input` 必须**精确切片**，多余字节会导致失败。

use oozextract::Extractor;
use oozextract::OozError;

/// 解码 Oodle(Kraken) 压缩数据。
///
/// # 参数
/// - `payload`: 原始 Kraken 比特流（Palworld `.sav` 中 `PlM` 的
///   `[12 .. 12+compressed_len]` **精确切片**）。
/// - `uncompressed_len`: 头部声明的未压缩长度，用作输出缓冲大小与完整性校验。
///
/// # 返回
/// 解压后的 GVAS 原始字节（应以 `b"GVAS"` 开头）。
///
/// # 错误
/// 解码失败（损坏 / 不支持的 opcode / 长度不符 / 切片不精确）时返回中文化错误，
/// **绝不静默返回损坏数据**。
pub fn oodle_decompress(payload: &[u8], uncompressed_len: usize) -> Result<Vec<u8>, String> {
    // 预分配恰好 uncompressed_len 的缓冲：Kraken 要求输出缓冲正好容纳解压结果。
    let mut output = vec![0u8; uncompressed_len];
    let mut extractor = Extractor::new();
    let written = extractor
        .read_from_slice(payload, &mut output)
        .map_err(|e: OozError| format!("Oodle(Kraken) 解码失败: {}", e))?;

    // 校验实际写出长度与声明长度一致。不一致通常是切片不精确或文件损坏——
    // 宁可明确报错，也不要返回截断/多余的数据造成静默损坏。
    if written != uncompressed_len {
        return Err(format!(
            "Oodle(Kraken) 解码长度不符：声明 {} 字节，实际解出 {} 字节（压缩数据切片不精确或文件损坏？）",
            uncompressed_len, written
        ));
    }
    Ok(output)
}
