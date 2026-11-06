//! Official `flashValue` line: `BeAced,3,0b00001011,0`

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashProfile {
    pub product: String,
    pub index: u8,
    /// Capability bitmask as used by the official app (e.g. `0b00001011`).
    pub flags: u8,
    pub reserved: u8,
}

impl Default for FlashProfile {
    fn default() -> Self {
        Self {
            product: "BeAced".into(),
            index: 3,
            flags: 0b0000_1011,
            reserved: 0,
        }
    }
}

impl FlashProfile {
    pub fn parse_line(line: &str) -> Result<Self> {
        let parts: Vec<&str> = line.trim().split(',').collect();
        anyhow::ensure!(
            parts.len() >= 4,
            "flashValue line expected 4 CSV fields, got {}",
            parts.len()
        );
        let flags = parse_flags(parts[2])?;
        Ok(Self {
            product: parts[0].trim().to_string(),
            index: parts[1].trim().parse().context("flash index")?,
            flags,
            reserved: parts[3].trim().parse().context("flash reserved")?,
        })
    }
}

fn parse_flags(raw: &str) -> Result<u8> {
    let s = raw.trim();
    if let Some(bin) = s.strip_prefix("0b").or_else(|| s.strip_prefix("0B")) {
        return u8::from_str_radix(bin, 2).context("binary flash flags");
    }
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return u8::from_str_radix(hex, 16).context("hex flash flags");
    }
    s.parse().context("decimal flash flags")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_official_flash_line() {
        let p = FlashProfile::parse_line("BeAced,3,0b00001011,0").unwrap();
        assert_eq!(p.product, "BeAced");
        assert_eq!(p.index, 3);
        assert_eq!(p.flags, 0b0000_1011);
        assert_eq!(p.reserved, 0);
    }
}
