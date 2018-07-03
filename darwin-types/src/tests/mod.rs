use pport::parse_pport_document;
macro_rules! mktest {
    ($($name:ident, $path:expr),*) => {
        $(
        #[test]
        fn $name() {
            let data = include_str!($path);
            let _ = parse_pport_document(data.as_bytes()).unwrap();
        }
        )*
    }
}
mktest! {
    parse_train_status_01, "train_status_01.xml"
}
