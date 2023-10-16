use std::path::{PathBuf, Path};

use serde::{Serialize, Deserialize};

const CONFIG_DIR: &str = ".config";
const APP_NAME: &str = "visify";
const CONFIG_NAME: &str = "config";
const DEFAULT_REDIRECT_URI_PORT: u16 = 8888;

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Config {
    pub redirect_uri_port: Option<u16>,
}

impl Config {
    pub fn load() -> Result<Config, confy::ConfyError> {
        let mut config: Config = confy::load(APP_NAME, CONFIG_NAME)?;

        config.redirect_uri_port = Some(config.redirect_uri_port.unwrap_or(DEFAULT_REDIRECT_URI_PORT));
        Ok(config)
    }

    pub fn store(self) -> Result<(), confy::ConfyError> {
        confy::store(APP_NAME, CONFIG_NAME, self)
    }
}

pub fn app_config_dir() -> PathBuf {
    let path = Path::new(env!("HOME"));
    let home_config_dir = path.join(CONFIG_DIR);
    home_config_dir.join(APP_NAME)
}
