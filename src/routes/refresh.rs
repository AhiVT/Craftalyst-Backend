use crate::guards::discord::{
  Bearer, RefreshToken, DiscordToken,
};

use rocket::http::Status;
use rocket_contrib::json::Json;
use std::convert::TryFrom;

#[post("/refresh", format = "json", data = "<refresh>")]
pub fn refresh(refresh: DiscordToken<RefreshToken>) -> Result<Json<Bearer>, Status> {
  let bearer = Bearer::try_from(refresh);

  match bearer {
    Ok(token) => Ok(Json(token)),
    Err(_) => Err(Status::Forbidden),
  }
}
