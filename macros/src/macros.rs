macro_rules! try_match {
    ($expression:expr, $pattern:pat $(if $guard:expr)? => $result:expr) => {
        match $expression {
            $pattern $(if $guard)? => Some($result),
            _ => None
        }
    };
    ($expression:expr, $($variant:tt)*) => {
        crate::macros::try_match!($expression, $($variant)*(__variant) => __variant)
    };
    ($pattern:pat $(if $guard:expr)? => $result:expr) => {
        |__arg| crate::macros::try_match!(__arg, $pattern $(if $guard)? => $result)
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

// Because nightly doesn't give the same span for `method`
macro_rules! bail_method {
    ($method:expr, $err:expr) => {
        crate::macros::bail!($method.sig.fn_token, $err)
    };
}
pub(crate) use bail_method;

macro_rules! fields {
    ($obj:expr => $($field:ident),* $(,)?) => {$(
        let $field = &$obj.$field;
    )*};
}
pub(crate) use fields;
