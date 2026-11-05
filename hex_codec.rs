// HeroBot AI hex codec ported 1:1 from HerobaneNair/HeRoBot
// (herobot-common/.../ai/AiHexCodec.java), CC-BY-NC-4.0.
// Upstream takes an AiScript and calls AiScriptCodec.toJson; the port
// splits that: callers pass the JSON string. Everything else matches:
// raw-deflate (Java nowrap, flate2's deflate is raw) at best
// compression, one version byte (0x01) framed in front, hex-encoded.
// JSON codec: script_codec.rs, ports next.

pub const VERSION: u8 = 0x01;

use flate2::write::DeflateEncoder;
use flate2::read::DeflateDecoder;
use flate2::Compression;
use std::io::{Read, Write};

pub fn encode_json(json: &str) -> String {
    let mut enc = DeflateEncoder::new(Vec::new(), Compression::best());
    enc.write_all(json.as_bytes()).expect("in-memory write");
    let compressed = enc.finish().expect("in-memory finish");
    let mut framed = Vec::with_capacity(compressed.len() + 1);
    framed.push(VERSION);
    framed.extend_from_slice(&compressed);
    hex_encode(&framed)
}

pub fn decode_json(hex: &str, fallback_name: &str) -> Result<String, String> {
    let trimmed = hex.trim();
    if trimmed.is_empty() {
        return Err("hex is empty".into());
    }
    let framed = hex_decode(trimmed).map_err(|_| "not a valid hex string".to_string())?;
    if framed.is_empty() {
        return Err("hex is empty".into());
    }
    if framed[0] != VERSION {
        return Err(format!("unsupported AI hex version: 0x{:02x}", framed[0]));
    }
    let mut dec = DeflateDecoder::new(&framed[1..]);
    let mut json = Vec::new();
    dec.read_to_end(&mut json).map_err(|_| "corrupt AI hex payload".to_string())?;
    let _ = fallback_name;
    String::from_utf8(json).map_err(|_| "payload is not a valid AI script".to_string())
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

fn hex_decode(s: &str) -> Result<Vec<u8>, ()> {
    let bytes = s.as_bytes();
    if bytes.len() % 2 != 0 { return Err(()); }
    let mut out = Vec::with_capacity(bytes.len() / 2);
    let mut i = 0;
    while i < bytes.len() {
        let hi = (bytes[i] as char).to_digit(16).ok_or(())?;
        let lo = (bytes[i + 1] as char).to_digit(16).ok_or(())?;
        out.push(((hi << 4) | lo) as u8);
        i += 2;
    }
    Ok(out)
}
