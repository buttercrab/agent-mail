use clap::Parser;
use std::net::SocketAddr;

#[derive(Debug, Clone, Parser)]
pub struct Config {
    #[arg(long, env = "AGENT_MAIL_DATABASE_URL")]
    pub database_url: String,

    #[arg(long, env = "AGENT_MAIL_BIND", default_value = "127.0.0.1:8787")]
    pub bind: SocketAddr,

    #[arg(long, env = "AGENT_MAIL_TOKEN")]
    pub token: String,

    #[arg(long, env = "AGENT_MAIL_ENVIRONMENT", default_value = "production")]
    pub environment: String,

    #[arg(
        long,
        env = "AGENT_MAIL_ALLOWED_ORIGINS",
        value_delimiter = ',',
        default_value = "https://agent-mail.cc"
    )]
    pub allowed_origins: Vec<String>,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let mut config = Self::parse();
        config.database_url = required("AGENT_MAIL_DATABASE_URL", config.database_url)?;
        config.token = required("AGENT_MAIL_TOKEN", config.token)?;
        config.environment = required("AGENT_MAIL_ENVIRONMENT", config.environment)?;
        config.allowed_origins = config
            .allowed_origins
            .into_iter()
            .map(|origin| origin.trim().trim_end_matches('/').to_string())
            .filter(|origin| !origin.is_empty())
            .collect();
        if config.allowed_origins.is_empty() {
            anyhow::bail!("AGENT_MAIL_ALLOWED_ORIGINS must contain at least one origin");
        }
        Ok(config)
    }
}

fn required(name: &str, value: String) -> anyhow::Result<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        anyhow::bail!("{name} must not be empty");
    }
    Ok(value)
}
