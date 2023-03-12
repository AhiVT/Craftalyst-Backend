use serde::{Deserialize, Serialize};
use serenity::prelude::TypeMapKey;
use std::fs::File;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Config {
  pub discord: Discord,
  pub minecraft: Minecraft,
  pub steam: Steam,
  pub mysql: Sql,
}

impl Config {
  pub fn get_config() -> Self {
    let f = match File::open("./config.yaml") {
      Ok(val) => val,
      Err(e) => {
        println!("WARNING! Couldn't open configuration: {:#?}", e);

        let buffer = File::create("./config.yaml").unwrap();
        serde_yaml::to_writer(buffer, &Config::default()).unwrap();
        println!("A new configuration file has been created for you.\nPlease configure, then relaunch.");

        panic!("Program requires configuration.")
      },
    };

    serde_yaml::from_reader(&f).unwrap()
  }
}

impl TypeMapKey for Config {
  type Value = Config;
}

impl Default for Config {
  fn default() -> Self {
    Self {
      discord: Discord::default(),
      minecraft: Minecraft::default(),
      steam: Steam::default(),
      mysql: Sql::default(),
    }
  }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Discord {
  pub start_bot: bool,
  pub guild_id: u64,
  pub channel_id: u64,
  pub token: String,
}

impl Default for Discord {
  fn default() -> Self {
    Self {
      start_bot: false,
      guild_id: 0u64,
      channel_id: 0u64,
      token: String::from(""),
    }
  }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Minecraft {
  pub enabled: bool,
}

impl Default for Minecraft {
  fn default() -> Self {
    Self {
      enabled: false,
    }
  }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Steam {
  pub api_key: String,
  pub enabled: bool,
}

impl Default for Steam {
  fn default() -> Self {
    Self {
      api_key: String::from("unset"),
      enabled: false,
    }
  }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Sql {
  pub username: String,
  pub password: String,
  pub endpoint: String,
  pub port: u16,
  pub database: String,
}

impl Default for Sql {
  fn default() -> Self {
    Self {
      username: String::from("username"),
      password: String::from("password"),
      endpoint: String::from("example.com"),
      port: 3306u16,
      database: String::from("whitelist-suite"),
    }
  }
}
