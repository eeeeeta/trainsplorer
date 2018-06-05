//! Managing the database pool.
//!
//! Code here abridged from https://rocket.rs/guide/state/#databases.

use r2d2_postgres::{TlsMode, PostgresConnectionManager};
use r2d2;
use postgres::Connection;
use rocket::Rocket;
use std::ops::Deref;
use rocket::http::Status;
use rocket::request::{self, FromRequest};
use rocket::{Request, State, Outcome};

pub type Pool = r2d2::Pool<PostgresConnectionManager>;
type Conn = <PostgresConnectionManager as r2d2::ManageConnection>::Connection;

#[derive(Debug)]
pub struct AppNameSetter;
impl<E> r2d2::CustomizeConnection<Conn, E> for AppNameSetter {
    fn on_acquire(&self, conn: &mut Conn) -> Result<(), E> {
        // FIXME: this unwrap isn't that great
        conn.execute("SET application_name TO 'osms-web';", &[]).unwrap();
        Ok(())
    }
}
pub fn attach_db(rocket: Rocket) -> Rocket {
    let config = r2d2::config::Builder::new()
        .connection_customizer(Box::new(AppNameSetter))
        .build();
    let manager = {
        let url = rocket.config().get_str("database_url")
            .expect("'database_url' in config");
        PostgresConnectionManager::new(url, TlsMode::None).unwrap()
    };
    let pool = r2d2::Pool::new(config, manager).expect("db pool");
    {
        let conn = pool.get().unwrap();
        ::osms_db::db::initialize_database(&*conn).unwrap();
    }
    rocket.manage(pool)
}

pub struct DbConn(pub r2d2::PooledConnection<PostgresConnectionManager>);

/// Attempts to retrieve a single connection from the managed database pool. If
/// no pool is currently managed, fails with an `InternalServerError` status. If
/// no connections are available, fails with a `ServiceUnavailable` status.
impl<'a, 'r> FromRequest<'a, 'r> for DbConn {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> request::Outcome<DbConn, ()> {
        let pool = request.guard::<State<Pool>>()?;
        match pool.get() {
            Ok(conn) => Outcome::Success(DbConn(conn)),
            Err(_) => Outcome::Failure((Status::ServiceUnavailable, ()))
        }
    }
}

impl Deref for DbConn {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
