use crate::DISCORD_GUILD_ID;
use crate::WhitelistDatabase;
use crate::guards::discord::{BearerToken, DiscordToken};
use crate::models::{MinecraftUser, MCStatus, Findable};

use rocket::http::Status;
use rocket_contrib::json::Json;
use serenity::http::client::Http;
use uuid::Uuid;

#[post("/whitelist/status", format = "json", data = "<bearer>")]
pub fn status(
  conn: WhitelistDatabase,
  bearer: DiscordToken<BearerToken>,
) -> Result<Json<MCStatus>, Status> {
  let token = format!("Bearer {}", bearer.token);
  let client = Http::new_with_token(&token);

  if let Ok(user) = client.get_current_user() {
    if let Ok(guilds) = user.guilds(&client) {
      let mut in_guild = false;
      for (_idx, guild) in guilds.into_iter().enumerate() {
        if guild.id == DISCORD_GUILD_ID {
          in_guild = true;
          break;
        }
      }

      match in_guild {
        true => {
          if let Ok(data) = MinecraftUser::find(*user.id.as_u64(), &conn) {
            let mut stat = MCStatus::from(data);

            match Uuid::parse_str(&stat.uuid) {
              Ok(uuid) => {
                stat.uuid = uuid
                  .to_hyphenated()
                  .encode_lower(&mut Uuid::encode_buffer())
                  .to_owned();
              },
              Err(_) => {},
            };

            Ok(Json(stat))
          } else {
            Err(Status::NotFound)
          }
        },
        false => Err(Status::Forbidden),
      }
    } else {
      Err(Status::Forbidden)
    }
  } else {
    Err(Status::Forbidden)
  }
}