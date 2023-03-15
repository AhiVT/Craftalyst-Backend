use std::time::{SystemTime, Duration};

use serenity::builder::CreateApplicationCommand;
use serenity::model::application::interaction::InteractionResponseType;
use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;
use serenity::model::channel::Message;
use serenity::model::prelude::UserId;
use serenity::model::prelude::command::CommandOptionType;
use serenity::model::prelude::interaction::application_command::{CommandDataOption, CommandDataOptionValue};
use serenity::prelude::{Context, SerenityError};
use serenity::utils::Colour;

use crate::commands::MinecraftUserModel;
use crate::constants::{RATELIMIT_INTERVAL, RATELIMIT_REQUESTS, BOT_AUTHOR, GET_CONN_POOL_ERR, EMBED_FOOTER, MC_CHANNEL_ID};
use crate::models::{Findable, NewMinecraftUser};
use crate::sql::MysqlPooledConnection;
use crate::structs::mojang::MinecraftUser;
use crate::structs::{Ratelimiter, Account, MysqlPoolContainer, DieselFind};

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
  command
    .name("mclink")
    .description("Whitelist your Minecraft account")
    .dm_permission(false)
    .create_option(|option| {
      option
        .name("username")
        .description("Your In-Game name as reported by Minecraft chat.")
        .kind(CommandOptionType::String)
        .max_length(16)
        .required(true)
    })
}

async fn check_mojang_ratelimit(ctx: &Context) -> Result<(), String> {
  let mut data = ctx.data.write().await;

  match data.get_mut::<Ratelimiter>() {
    Some(mut ratelimiter) => {
      let time = ratelimiter.0;
      let requests = ratelimiter.1;

      if time.elapsed().unwrap() > RATELIMIT_INTERVAL {
        ratelimiter.0 = SystemTime::now();
        // Not zero because this command is also making a request
        ratelimiter.1 = 1u16;

        Ok(())
      } // Executes if under ratelimit quota
      else if requests < RATELIMIT_REQUESTS {
        ratelimiter.1 += 1u16;

        Ok(())
      } else {
        let time_remaining = Duration::from_secs(RATELIMIT_INTERVAL.as_secs() - time.elapsed().unwrap().as_secs());

        Err(format!("We're currently experiencing heavy load.\nTry again in about {:#?} seconds.", time_remaining.as_secs()))
      }
    },
    None => {
      Err(format!("There was a general error. Please try again.\nContact <@{}> for assistance.", BOT_AUTHOR))
    },
  }
}

async fn get_conn(ctx: &Context) -> Result<MysqlPooledConnection, String> {
  let data = ctx.data.read().await;

  match data.get::<MysqlPoolContainer>() {
    Some(v) => v.get().map_err(|err| err.to_string()),
    None => Err(format!("There was a general error. Please try again.\nContact <@{}> for assistance.", BOT_AUTHOR)),
  }
}

async fn check_sender_not_whitelisted(
  ctx: &Context,
  command: &ApplicationCommandInteraction,
  account_type: Account,
) -> Result<(), String> {
  let author_id = match &command.member {
    Some(invoker) => {
      invoker.user.id.as_u64().clone()
    },
    None => return Err("Message sent from DM".to_string()),
  };
  let conn = match get_conn(ctx).await {
    Ok(val) => val,
    Err(_) => return Err(GET_CONN_POOL_ERR.to_string()),
  };

  let res: DieselFind;

  // This may be an issue
  match account_type {
    Account::Mojang => res = DieselFind::from(MinecraftUserModel::find(author_id, &conn)),
    Account::Steam => return Err("Steam linking no longer supported".to_string()),
    _ => return Err("Invalid account type".to_string())
  };

  match res.0 {
    // User found
    None => Err(format!("This account has already been whitelisted by <@{}>", author_id)),
    Some(e) => {
      use diesel::result::Error;

      match e {
        // If we aren't in the database then we are guaranteed to not be whitelisted
        Error::NotFound => Ok(()),
        _ => Err("An unexpected error occurred. You were not whitelisted.".to_string())
      }
    }
  }
}

async fn not_mc_whitelisted(
  ctx: &Context,
  command: &ApplicationCommandInteraction,
) -> Result<(), String> {
  check_sender_not_whitelisted(ctx, command, Account::Mojang).await
}

pub async fn run(ctx: &Context, command: &ApplicationCommandInteraction, options: &[CommandDataOption]) -> Result<(), SerenityError> {
  let option = options
    .get(0)
    .expect("Expected that juicy Minecraft username")
    .resolved
    .as_ref()
    .expect("Expected the value of that argument");

  if let CommandDataOptionValue::String(username) = option {
    if check_mojang_ratelimit(ctx).await.is_ok() && not_mc_whitelisted(ctx, &command).await.is_ok() {
      println!("{}", &username);
      let res = MinecraftUser::get_user(&username).await;
      let json: Vec<MinecraftUser>;

      match res {
        Ok(val) => json = val,
        Err(err) => {
          return command
            .create_interaction_response(&ctx.http, |res| {
              res
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|msg| {
                  msg.embed(|embed| {
                    embed.title("Failure");
                    embed.description(
                      format!("There was an error communicating with Mojang. Here's some more information:\n```{:#?}```", err)
                    );
                    embed.color(Colour::new(0x0000_960C));
                    embed.footer(|f| f.text(EMBED_FOOTER))
                  })    
                })
            }).await
        }
      };

      println!("{:#?}", json);

      // If resulting array is empty, then username is not found
      if json.is_empty() {
        return command
          .create_interaction_response(&ctx.http, |res| {
            res
              .kind(InteractionResponseType::ChannelMessageWithSource)
              .interaction_response_data(|msg| {
                msg.embed(|embed| {
                  embed.title("Failure");
                  embed.description("Username not found. Please check for typos and try again.");
                  embed.color(Colour::new(0x0000_960C));
                  embed.footer(|f| f.text(EMBED_FOOTER))
                })    
              })
          }).await
      }

      // Overwrite json removing the Some()
      let json: MinecraftUser = json[0].clone();

      let author_id = match &command.member {
        Some(invoker) => {
          invoker.user.id.as_u64().clone()
        },
        None => {
          println!("Aborting command since it was sent from a DM.");
          return Err(SerenityError::Other("Server command run from DM"))
        },
      };

      // Add account to database
      let user = NewMinecraftUser {
        discord_id: author_id,
        minecraft_uuid: String::from(&json.id),
        minecraft_name: String::from(&json.name),
      };
      match get_conn(ctx).await {
        Ok(conn) => {
          let result = user.create(&conn);

          match result {
            Ok(_) => {
              command
                .create_interaction_response(&ctx.http, |res| {
                  res
                    .kind(InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|msg| {
                      msg.embed(|embed| {
                        embed.title("Success");
                        embed.description(format!("Your Minecraft account `{}` has been successfully linked.
              Please check <#{}> channel pins for server info and FAQ.
              **If you leave this Discord server for any reason, you will be removed from the whitelist**", json.name, MC_CHANNEL_ID));
                        embed.color(Colour::new(0x0000_960C));
                        embed.footer(|f| f.text(EMBED_FOOTER))    
                      })    
                    })
                }).await
            },
            Err(err) => {
              command
                .create_interaction_response(&ctx.http, |res| {
                  res
                    .kind(InteractionResponseType::ChannelMessageWithSource)
                    .interaction_response_data(|msg| {
                      msg.embed(|embed| {
                        embed.title("Failure");
                        embed.description(format!("Unknown error: Please share this code block with the author: {:#?}", err));
                        embed.color(Colour::new(0x0000_960C));
                        embed.footer(|f| f.text(EMBED_FOOTER))
                      })    
                    })
                }).await
            }
          }
        },
        Err(err) => {
          return command
            .create_interaction_response(&ctx.http, |res| {
              res
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|msg| {
                  msg.embed(|embed| {
                    embed.title("Failure");
                    embed.description(format!("Unknown error: Please share this code block with the author: {:#?}", err));
                    embed.color(Colour::new(0x0000_960C));
                    embed.footer(|f| f.text(EMBED_FOOTER))
                  })    
                })
            }).await
        }
      }
    } else {
      command
        .create_interaction_response(&ctx.http, |res| {
          res
            .kind(InteractionResponseType::ChannelMessageWithSource)
            .interaction_response_data(|msg| {
              msg.embed(|embed| {
                embed.title("Failure");
                embed.description("You've already whitelisted an account!");
                embed.color(Colour::new(0x0000_960C));
                embed.footer(|f| f.text(EMBED_FOOTER))
              })    
            })
        }).await
    }
  } else {
    command
      .create_interaction_response(&ctx.http, |res| {
        res
          .kind(InteractionResponseType::ChannelMessageWithSource)
          .interaction_response_data(|msg| {
            msg.embed(|embed| {
              embed.title("Failure");
              embed.description("Username not valid or already whitelisted.");
              embed.color(Colour::new(0x0000_960C));
              embed.footer(|f| f.text(EMBED_FOOTER))
            })    
          })
      }).await
  }
}
