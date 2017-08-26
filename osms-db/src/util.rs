use postgres::GenericConnection;
use postgres::types::ToSql;
use errors::*;

pub fn count<T: GenericConnection>(conn: &T, details: &str, args: &[&ToSql]) -> Result<i64> {
    Ok(conn.query(&format!("SELECT COUNT(*) {}", details), args)?.into_iter()
       .nth(0)
       .ok_or("Count query failed")?
       .get(0))
}
