use std::net::IpAddr;

use tokio::fs;

pub async fn read() -> anyhow::Result<Config> {
    let config_path = "mensa_api.toml";

    let canon = tokio::fs::canonicalize(".").await?;
    tracing::info!("try reading config file {config_path} at {canon:?}");

    let config = if fs::try_exists(config_path).await? {
        tracing::info!("found config");

        let config = fs::read_to_string(canon).await?;
        let config = toml::from_str(&config)?;
        tracing::info!("read config");
        config
    } else {
        tracing::info!("config does not exist, using default config");
        Config::default()
    };

    tracing::info!("using config: {config:#?}");
    Ok(config)
}

#[derive(Debug, serde::Deserialize)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,
    pub db: Option<DbConfig>,
}

impl Default for Config {
    fn default() -> Self {
        #[cfg(debug_assertions)] let db = {
            tracing::info!("using default config for db in debug");
            Some(DbConfig::default())
        };
        #[cfg(not(debug_assertions))] let db = {
            tracing::warn!("no db config provided, running without db");
            None
        };

        let server = ServerConfig::default();

        Self { db, server }
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct DbConfig {
    pub url: String,
    pub database: String,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            url: format!("mongodb://localhost:27017"),
            database: format!("stw_mensa"),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
pub struct ServerConfig {
    pub address: IpAddr,
    pub port: u16,
}

impl Default for ServerConfig {
    fn default() -> Self {
        #[cfg(not(debug_assertions))]
        tracing::warn!("using default server config in release");

        ServerConfig {
            address: IpAddr::from([0, 0, 0, 0]),
            port: 3000,
        }
    }
}


