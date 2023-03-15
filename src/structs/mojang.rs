use reqwest::StatusCode;
use serde_json::json;

use crate::constants::*;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MinecraftUser {
  pub id: String,
  pub name: String,
}

impl MinecraftUser {
  pub async fn get_user(username: &str) -> Result<Vec<MinecraftUser>, StatusCode> {
    let client = reqwest::Client::new();
    let payload = json!([&username]);
    // Will panic if cannot connect to Mojang
    let resp = client.post(MOJANG_API).json(&payload).send().await;
    match resp {
      Ok(res) => {
        match res.status() {
          StatusCode::OK => Ok(res.json().await.unwrap()),
          _ => Err(res.status()),
        }
      },
      Err(e) => Err(e.status().unwrap()),
    }
  }
}

impl Clone for MinecraftUser {
  fn clone(&self) -> Self {
    Self {
      id: String::from(&self.id),
      name: String::from(&self.name),
    }
  }
}
