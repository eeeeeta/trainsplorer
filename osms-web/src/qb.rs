use postgres::types::ToSql;
use postgres::GenericConnection;
use osms_db::db::DbType;
use super::Result;

pub struct QueryBuilder<'a> {
    args: Vec<&'a ToSql>,
    segments: Vec<String>,
    other: Vec<String>
}
impl<'a> QueryBuilder<'a> {
    pub fn new() -> Self {
        QueryBuilder {
            args: vec![],
            segments: vec![],
            other: vec![]
        }
    }
    pub fn add_other(&mut self, segm: &str, args: &[&'a ToSql]) {
        let mut segm = segm.to_owned();
        for n in 0..args.len() {
            let pos = self.args.len() + 1 + n;
            segm = segm.replace("$$", &format!("${}", pos));
        }
        self.args.extend(args);
        self.other.push(segm);
    }
    pub fn add(&mut self, segm: &str, args: &[&'a ToSql]) {
        if self.other.len() != 0 {
            panic!("QueryBuilder::add() used after add_other()");
        }
        let mut segm = segm.to_owned();
        for n in 0..args.len() {
            let pos = self.args.len() + 1 + n;
            segm = segm.replace("$$", &format!("${}", pos));
        }
        self.args.extend(args);
        self.segments.push(segm);
    }
    pub fn query_dbtype<T: DbType, U: GenericConnection>(self, conn: &U) -> Result<Vec<T>> {
        let where_clause = if self.segments.len() == 0 {
            self.other.join(" ")
        }
        else {
            format!("WHERE {} {}", self.segments.join(" AND "), self.other.join(" ")).into()
        };
        let ret = T::from_select(conn, &where_clause, &self.args)?;
        Ok(ret)
    }
}
