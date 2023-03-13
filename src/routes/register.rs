use crate::DISCORD_GUILD_ID;
use crate::constants::*;
use crate::guards::discord::{BearerToken, DiscordToken};
use crate::guards::mojang::Ratelimiter;
use crate::models::{MinecraftUser, NewMinecraftUser, Findable};
use crate::structs::config::Config;
use crate::WhitelistDatabase;

use rocket::http::Status;
use serenity::{
  http::client::Http,
  model::id::ChannelId,
  utils::Colour,
};
use uuid::Uuid;

#[post("/register/<uuid>", format = "json", data = "<bearer>")]
pub fn register(
  conn: WhitelistDatabase,
  _limiter: Ratelimiter,
  uuid: String,
  bearer: DiscordToken<BearerToken>,
) -> Result<Status, Status> {
  match Uuid::parse_str(&uuid) {
    Ok(uuid) => {
      let uuid = uuid
        .to_simple()
        .encode_lower(&mut Uuid::encode_buffer())
        .to_owned();
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
                if data.suspended == 1 {
                  return Err(Status::Forbidden)
                }
    
                use diesel::RunQueryDsl;
                use crate::schema::minecrafters::dsl::*;
                use crate::diesel::ExpressionMethods;
                match diesel::update(&data).set(minecraft_uuid.eq(&uuid)).execute(&*conn) {
                  Ok(_) => {
                    let config = Config::get_config();
                    let bot = Http::new_with_token(&format!("Bot {}", &config.discord.token));

                    let old = Uuid::parse_str(&data.minecraft_uuid)
                      .unwrap()
                      .to_hyphenated()
                      .encode_lower(&mut Uuid::encode_buffer())
                      .to_owned();
                    let new = Uuid::parse_str(&uuid)
                      .unwrap()
                      .to_hyphenated()
                      .encode_lower(&mut Uuid::encode_buffer())
                      .to_owned();
        
                    let _ = ChannelId(305_572_919_730_372_618).send_message(&bot, |m| {
                      m.embed(|em| {
                        em.title("Account change");
                        em.description(format!("<@{}> changed their Minecraft UUID from `{}` to `{}`", &data.discord_id, old, new));
                        em.color(Colour::new(0x00FF_0000));
                        em.footer(|f| f.text(EMBED_FOOTER))
                      })
                    });
                    
                    Ok(Status::Ok)
                  },
                  Err(_) => Err(Status::InternalServerError),
                }
              } else {
                let res = NewMinecraftUser {
                  discord_id: *user.id.as_u64(),
                  minecraft_uuid: Uuid::parse_str(&uuid)
                    .unwrap()
                    .to_simple()
                    .encode_lower(&mut Uuid::encode_buffer())
                    .to_owned(),
                  minecraft_name: String::from("deprecated"),
                };

                match res.create(&*conn) {
                  Ok(_) => Ok(Status::Ok),
                  Err(_) => Err(Status::InternalServerError),
                }
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
    },
    Err(_) => Err(Status::BadRequest)
  }
}
