pub use postgres::GenericConnection;
pub use postgres::rows::Row;
use postgres::rows::LazyRows;
use postgres::types::ToSql;
use errors::*;
use std::marker::PhantomData;
use fallible_iterator::FallibleIterator;
use r2d2::Pool;
use r2d2_postgres::PostgresConnectionManager;
use migration;

pub type DbPool = Pool<PostgresConnectionManager>;
pub struct SelectIterator<'trans, 'stmt, T> {
    inner: LazyRows<'trans, 'stmt>,
    _ph: PhantomData<T>
}
impl<'a, 'b, T> Iterator for SelectIterator<'a, 'b, T> where T: DbType {
    type Item = Result<T>;

    fn next(&mut self) -> Option<Result<T>> {
        match self.inner.next() {
            Ok(None) => None,
            Ok(Some(x)) => Some(Ok(T::from_row(&x))),
            Err(e) => Some(Err(e.into()))
        }
    }
}
pub trait DbType: Sized {
    fn table_name() -> &'static str;
    fn from_row(row: &Row) -> Self;
    fn from_select<T: GenericConnection>(conn: &T, where_clause: &str, args: &[&ToSql]) -> Result<Vec<Self>> {
        let query = format!("SELECT * FROM {} {}", Self::table_name(), where_clause);
        let qry = conn.query(&query, args)?;
        let mut ret = vec![];
        for row in &qry {
            ret.push(Self::from_row(&row));
        }
        Ok(ret)
    }
}
pub trait InsertableDbType: DbType {
    type Id;
    fn insert_self<T: GenericConnection>(&self, conn: &T) -> Result<Self::Id>;
}
pub fn initialize_database<T: GenericConnection>(conn: &T) -> Result<()> {
    migration::initialize_migrations(conn)?;
    Ok(())
}
