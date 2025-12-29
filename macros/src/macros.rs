macro_rules! try_match {
    ($expression:expr, $pattern:pat $(if $guard:expr)? => $result:expr) => {
        match $expression {
            $pattern $(if $guard)? => Some($result),
            _ => None
        }
    };
    ($pattern:pat $(if $guard:expr)? => $result:expr) => {
        |__arg| crate::macros::try_match!(__arg, $pattern $(if $guard)? => $result)
    };
    ($expression:expr, $($variant:tt)*) => {
        crate::macros::try_match!($expression, $($variant)*(__variant) => __variant)
    };
    ($($variant:tt)*) => {
        |__arg| crate::macros::try_match!(__arg, $($variant)*(__variant) => __variant)
    };
}
pub(crate) use try_match;

macro_rules! bail {
    ($tokens:expr, $err:expr) => {
        return Err(syn::Error::new(syn::spanned::Spanned::span(&$tokens), $err))
    };
}
pub(crate) use bail;
