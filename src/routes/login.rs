use crate::guards::discord::{AccessCode, Bearer, DiscordToken};

use rocket::http::Status;
use rocket_contrib::json::Json;
use std::convert::TryFrom;

#[post("/login", format = "json", data = "<oauth>")]
pub fn exchange(oauth: DiscordToken<AccessCode>) -> Result<Json<Bearer>, Status> {
  let bearer = Bearer::try_from(oauth);

  match bearer {
    Ok(token) => Ok(Json(token)),
    Err(_) => Err(Status::Forbidden),
  }
}
