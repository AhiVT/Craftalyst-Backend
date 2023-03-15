use serenity::{
  builder::CreateApplicationCommand,
  model::{prelude::{
    command::CommandOptionType,
    interaction::{application_command::{
      ApplicationCommandInteraction,
      CommandDataOption, CommandDataOptionValue
    }, InteractionResponseType}, PermissionOverwrite
  }, Permissions},
  prelude::Context, utils::Colour
};
use serenity::prelude::SerenityError;

use crate::{structs::Account, constants::EMBED_FOOTER, models::Deleteable};

use super::get_conn;

pub fn register(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
  command
    .name("mcunlink")
    .description("Unlinks a Minecraft account given an @mention")
    .default_member_permissions(Permissions::ADMINISTRATOR)
    .create_option(|option| {
      option
        .name("at_mention")
        .description("The user to be unlinked.")
        .kind(CommandOptionType::User)
        .required(true)
    })
}

pub async fn run(ctx: &Context, command: &ApplicationCommandInteraction, options: &[CommandDataOption]) -> Result<(), SerenityError> {
  let option = options
    .get(0)
    .expect("Expected that juicy User struct")
    .resolved
    .as_ref()
    .expect("Expected the value of that argument");

  println!("Running command");

  if let CommandDataOptionValue::User(username, _) = option {
    println!("Args valid");
    match get_conn(ctx).await {
      Ok(conn) => {
        let usr = username.id;
        if usr.delete(Account::Mojang, &conn).is_ok() {
          return command
            .create_interaction_response(&ctx.http, |res| {
              res
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|msg| {
                  msg.embed(|embed| {
                    embed.title("Minecraft Unlink Success");
                    embed.description(format!("<@{}> was unlinked successfully.", &usr.as_u64()));
                    embed.color(Colour::new(0x0000_960C));
                    embed.footer(|f| f.text(EMBED_FOOTER))
                  })    
                })
            }).await
        }

        return command
          .create_interaction_response(&ctx.http, |res| {
            res
              .kind(InteractionResponseType::ChannelMessageWithSource)
              .interaction_response_data(|msg| {
                msg.embed(|embed| {
                  embed.title("Minecraft Unlink Failed");
                  embed.description(format!("<@{}> was not unlinked.", usr.as_u64()));
                  embed.color(Colour::new(0x0000_960C));
                  embed.footer(|f| f.text(EMBED_FOOTER))
                })    
              })
          }).await
      },
      Err(_) => {
        return command
          .create_interaction_response(&ctx.http, |res| {
            res
              .kind(InteractionResponseType::ChannelMessageWithSource)
              .interaction_response_data(|msg| {
                msg.embed(|embed| {
                  embed.title("Minecraft Unlink Failed");
                  embed.description("SQL Connection Unavailable");
                  embed.color(Colour::new(0x0000_960C));
                  embed.footer(|f| f.text(EMBED_FOOTER))
                })    
              })
          }).await
      }
    }  
  }

  Ok(())
}
