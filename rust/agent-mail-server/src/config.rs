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
    pub fn from_env() -> Self {
        Self::parse()
    }
}
