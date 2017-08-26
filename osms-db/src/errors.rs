error_chain! {
    foreign_links {
        Io(::std::io::Error);
        Postgres(::postgres::error::Error);
        Serde(::serde_json::Error);
    }
}
