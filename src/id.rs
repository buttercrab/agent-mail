use rand::TryRng;

pub fn message_id() -> anyhow::Result<String> {
    let now = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    Ok(format!("mail-{now}-{}", random_hex(8)?))
}

pub fn identity() -> anyhow::Result<String> {
    const ADJECTIVES: &[&str] = &[
        "amber", "blue", "bright", "calm", "clear", "copper", "crisp", "green", "open", "quiet",
        "red", "silver", "steady", "swift", "warm", "white",
    ];
    const NOUNS: &[&str] = &[
        "bridge", "cloud", "field", "harbor", "lane", "light", "map", "path", "ridge", "river",
        "signal", "sky", "spark", "stone", "trail", "wave",
    ];
    let mut raw = [0_u8; 6];
    rand::rngs::SysRng.try_fill_bytes(&mut raw)?;
    let adjective = ADJECTIVES[usize::from(raw[0]) % ADJECTIVES.len()];
    let noun = NOUNS[usize::from(raw[1]) % NOUNS.len()];
    Ok(format!("{adjective}-{noun}-{}", hex::encode(&raw[2..])))
}

pub fn session_id() -> anyhow::Result<String> {
    Ok(format!("mcp-{}", random_hex(24)?))
}

fn random_hex(len: usize) -> anyhow::Result<String> {
    let mut raw = vec![0_u8; len];
    rand::rngs::SysRng.try_fill_bytes(&mut raw)?;
    Ok(hex::encode(raw))
}
