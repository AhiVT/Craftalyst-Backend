table! {
  minecrafters (discord_id) {
    discord_id -> Unsigned<Bigint>,
    minecraft_uuid -> Varchar,
    minecraft_name -> Varchar,
    suspended -> Tinyint,
  }
}

table! {
  steam (discord_id) {
    discord_id -> Unsigned<Bigint>,
    steam_id -> Unsigned<Bigint>,
  }
}

allow_tables_to_appear_in_same_query!(
  minecrafters,
  steam,
);
