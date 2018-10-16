//! Managing the database pool.

use r2d2_postgres::{TlsMode, PostgresConnectionManager};
use r2d2;
use super::Result;

pub type Pool = r2d2::Pool<PostgresConnectionManager>;
pub type DbConn = r2d2::PooledConnection<PostgresConnectionManager>;
type Conn = <PostgresConnectionManager as r2d2::ManageConnection>::Connection;

#[derive(Debug)]
pub struct AppNameSetter;
impl<E> r2d2::CustomizeConnection<Conn, E> for AppNameSetter {
    fn on_acquire(&self, conn: &mut Conn) -> ::std::result::Result<(), E> {
        // FIXME: this unwrap isn't that great
        conn.execute("SET application_name TO 'osms-web';", &[]).unwrap();
        Ok(())
    }
}
pub fn attach_db(url: &str) -> Result<Pool> {
    let manager = PostgresConnectionManager::new(url, TlsMode::None)?;
    let pool = r2d2::Builder::new()
        .connection_customizer(Box::new(AppNameSetter))
        .build(manager)?;
    {
        let conn = pool.get().unwrap();
        ::osms_db::db::initialize_database(&*conn)?;
    }
    Ok(pool)
}

