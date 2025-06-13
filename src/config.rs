use std::{
    fs::{read_to_string, File},
    io::Write,
    path::Path,
};

use serde::{Deserialize, Serialize};

pub(crate) const CONFIG_PATH: &str = "config.toml";

#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct Config {
    pub(crate) server: Server,
    pub(crate) user: User,
}

impl Config {
    pub(crate) fn write_to_file(&self, path: &Path, overwrite: bool) -> bool {
        if overwrite {
            File::create(path)
        } else {
            File::create_new(path)
        }
        .and_then(|mut f| f.write_all(toml::to_string_pretty(self).unwrap().as_bytes()))
        .is_ok()
    }

    pub(crate) fn read_from_file(path: &Path) -> Option<Config> {
        toml::from_str(&read_to_string(path).ok()?).ok()?
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct Server {
    pub(crate) address: String,
    pub(crate) port: u16,
    pub(crate) auto_connect: bool,
}

impl Default for Server {
    fn default() -> Self {
        Self {
            address: String::from("server.slsknet.org"),
            port: 2242,
            auto_connect: true,
        }
    }
}

impl ToString for Server {
    fn to_string(&self) -> String {
        format!("{}:{}", self.address, self.port)
    }
}
#[derive(Debug, Default, Deserialize, Serialize)]
pub(crate) struct User {
    pub(crate) name: String,
    pub(crate) password: String,
    pub(crate) port: u16,
}
