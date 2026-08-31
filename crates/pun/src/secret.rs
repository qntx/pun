use std::str::FromStr;

use anyhow::Context;
use iroh::SecretKey;

/// Load `PUN_SECRET` or generate a random iroh secret key.
pub(crate) fn get_or_create_secret(print: bool) -> anyhow::Result<SecretKey> {
    std::env::var("PUN_SECRET").map_or_else(
        |_| {
            let key = SecretKey::generate();
            if print {
                eprintln!("using secret key {}", hex::encode(key.to_bytes()));
            }
            Ok(key)
        },
        |secret| SecretKey::from_str(&secret).context("invalid PUN_SECRET"),
    )
}
