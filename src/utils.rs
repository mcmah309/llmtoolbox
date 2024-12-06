macro_rules! unwrap_match {
    ($binding:ident, $one:ident::$two:ident) => {
        match $binding {
            $one::$two($binding) => $binding,
            _ => panic!(),
        }
    };
    ($binding:ident, $one:ident::$two:ident, $msg:tt) => {
        match $binding {
            $one::$two($binding) => $binding,
            _ => panic!($msg),
        }
    };
}
pub(crate) use unwrap_match;