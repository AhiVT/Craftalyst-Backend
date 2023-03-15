#![allow(clippy::single_component_path_imports)]

use diesel::MysqlConnection;
use serenity::model::id::UserId;
use std::marker::Sized;

use crate::schema::minecrafters;
use crate::structs::Account;

pub trait Deleteable {
  fn delete(&self, account_type: Account, connection: &MysqlConnection) -> Result<usize, diesel::result::Error>;
}

pub trait Findable {
  fn find(id: u64, connection: &MysqlConnection) -> Result<Self, diesel::result::Error>
    where Self: Sized;
}

pub trait Searchable<T, O> {
  fn search(val: T, connection: &MysqlConnection) -> Result<O, diesel::result::Error>;
}

pub trait Suspendable {
  fn suspend(&self, account_type: Account, connection: &MysqlConnection) -> Result<usize, diesel::result::Error>;
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MCStatus {
  pub status: u8,
  pub uuid: String,
}

impl From<MinecraftUser> for MCStatus {
  fn from(data: MinecraftUser) -> Self {
    Self {
      status: data.suspended as u8,
      uuid: data.minecraft_uuid,
    }
  }
}

#[derive(Queryable, Serialize, Deserialize, Identifiable, AsChangeset)]
#[primary_key(discord_id)]
#[table_name="minecrafters"]
pub struct MinecraftUser {
  pub discord_id: u64,
  pub minecraft_uuid: String,
  pub minecraft_name: String,
  pub suspended: i8,
}

#[derive(Insertable, Deserialize, AsChangeset)]
#[table_name="minecrafters"]
pub struct NewMinecraftUser {
  pub discord_id: u64,
  pub minecraft_uuid: String,
  pub minecraft_name: String,
}

impl NewMinecraftUser {
  pub fn create(&self, connection: &MysqlConnection) -> Result<usize, diesel::result::Error> {
    use diesel::RunQueryDsl;

    diesel::insert_into(minecrafters::table)
      .values(self)
      .execute(connection)
  }
}

impl Deleteable for UserId {
  fn delete(&self, account_type: Account, connection: &MysqlConnection) -> Result<usize, diesel::result::Error> {
    use diesel::QueryDsl;
    use diesel::RunQueryDsl;

    match account_type {
      Account::Mojang => {
        use crate::schema::minecrafters::dsl;

        diesel::delete(dsl::minecrafters.find(self.as_u64()))
          .execute(connection)
      },
      Account::Steam => Ok(0),
      Account::All => {
        use crate::schema::minecrafters::dsl as mc_dsl;

        diesel::delete(mc_dsl::minecrafters.find(self.as_u64()))
          .execute(connection)
      },
    }
  }
}

impl Suspendable for UserId {
  fn suspend(&self, account_type: Account, connection: &MysqlConnection) -> Result<usize, diesel::result::Error> {
    use diesel::QueryDsl;
    use diesel::RunQueryDsl;

    match account_type {
      Account::Mojang => {
        use crate::diesel::ExpressionMethods;
        use crate::schema::minecrafters::dsl::*;

        diesel::update(minecrafters.find(self.as_u64()))
          .set(suspended.eq(1))
          .execute(connection)
      },
      _ => Ok(0)
    }
  }
}

impl Findable for MinecraftUser {
  fn find(id: u64, connection: &MysqlConnection) -> Result<Self, diesel::result::Error> {
    use diesel::QueryDsl;
    use diesel::RunQueryDsl;

    minecrafters::table
      .find(&id)
      .first(connection)
  }
}

impl Searchable<&str, String> for MinecraftUser {
  fn search(val: &str, connection: &MysqlConnection) -> Result<String, diesel::result::Error> {
    use diesel::QueryDsl;
    use diesel::RunQueryDsl;
    use crate::schema::minecrafters::dsl::*;
    use crate::diesel::ExpressionMethods;

    minecrafters
      .filter(minecraft_uuid.eq(val))
      .select(minecraft_uuid)
      .first(connection)
  }
}
