// Take a look at the generated `clorinde` crate if you want to
// see what it looks like under the hood.
use clorinde::queries::module_2::authors;

#[tokio::main]
pub async fn main() {
    // You can learn which database connection types are compatible with Clorinde in the book
    // https://clorinde-rs.netlify.app/book/using_queries/db_connections.html
    let pool = create_pool().await.unwrap();
    let client = pool.get().await.unwrap();

    // The `all` method returns queried rows collected into a `Vec`
    let authors = authors().bind(&client).all().await.unwrap();
    dbg!(&authors[0].dob);
}

/// Connection pool configuration.
///
/// This is just a simple example config, please look at
/// `tokio_postgres` and `deadpool_postgres` for details.
use clorinde::deadpool_postgres::{Config, CreatePoolError, Pool, Runtime};
use clorinde::tokio_postgres::NoTls;

async fn create_pool() -> Result<Pool, CreatePoolError> {
    let mut cfg = Config::new();
    cfg.user = Some(String::from("postgres"));
    cfg.password = Some(String::from("postgres"));
    cfg.host = Some(String::from("127.0.0.1"));
    cfg.port = Some(5435);
    cfg.dbname = Some(String::from("postgres"));
    cfg.create_pool(Some(Runtime::Tokio1), NoTls)
}
