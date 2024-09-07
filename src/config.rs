use color_eyre::eyre::Result;

use crate::utils::get_env_var;

pub struct DatabaseConfig {
    pub username: String,
    pub password: Option<String>,
    pub database: String,
    pub host: String,
    pub port: u16,
}

impl DatabaseConfig {
    pub fn from_env() -> Result<Self> {
        let username = get_env_var("USERNAME")?;
        let password = get_env_var("PASSWORD").ok();
        let database = get_env_var("ROOT_DATABASE")?;
        let host = get_env_var("DATABASE_HOST")?;
        let port = get_env_var("DATABASE_PORT")?.parse()?;

        Ok(Self {
            username,
            password,
            database,
            host,
            port,
        })
    }
}

impl From<&DatabaseConfig> for tokio_postgres::Config {
    fn from(value: &DatabaseConfig) -> Self {
        let mut config = tokio_postgres::Config::new();

        config
            .user(value.username.clone())
            .dbname(value.database.clone())
            .host(value.host.clone())
            .port(value.port);

        if let Some(password) = value.password.as_deref() {
            config.password(password.to_owned());
        }

        config
    }
}
