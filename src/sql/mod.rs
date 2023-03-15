use diesel::mysql::MysqlConnection;
use diesel::r2d2::{Pool, PooledConnection, ConnectionManager, PoolError};

use crate::structs::config::Config;

pub type MysqlPool = Pool<ConnectionManager<MysqlConnection>>;
pub type MysqlPooledConnection = PooledConnection<ConnectionManager<MysqlConnection>>;

fn init_pool(database_url: &str) -> Result<MysqlPool, PoolError> {
  let manager = ConnectionManager::<MysqlConnection>::new(database_url);

  Pool::builder().build(manager)
}

pub fn establish_connection() -> MysqlPool {
  let sql_vals = Config::get_config().mysql;
  let database_url = format!("mysql://{}:{}@{}/{}",
    sql_vals.username, sql_vals.password,
    sql_vals.endpoint, sql_vals.database,
  );

  init_pool(&database_url).expect("Failed to create pool")
}
