#[macro_export]
macro_rules! impl_from_for_error {
    ($error:ident, $($orig:ident => $var:ident),*) => {
        $(
            impl From<$orig> for $error {
                fn from(err: $orig) -> $error {
                    $error::$var(err)
                }
            }
        )*
    }
}

