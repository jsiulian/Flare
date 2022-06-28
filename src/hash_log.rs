#[macro_export]
macro_rules! some_log {
    ( $f:tt, $l:expr, $( $x:expr ),* ) => {
        {
            if cfg!(debug_assertions) {
                log::$f!($l, $($x,)*)
            } else {
                #[allow(unused_imports)]
                use std::collections::hash_map::DefaultHasher;
                #[allow(unused_imports)]
                use std::hash::Hash;
                #[allow(unused_imports)]
                use std::hash::Hasher;

                log::$f!(
                    $l,
                    $(
                        {
                            let mut hasher = DefaultHasher::new();
                            $x.hash(&mut hasher);
                            format!("{:x}", hasher.finish())
                        },
                     )*
               )
            }
        }
    };
}

#[macro_export]
macro_rules! trace {
    ( $l:expr, $( $x:expr ),* ) => {
        crate::some_log!(trace, $l, $(
                ($x)
        ),*);
    }
}

#[macro_export]
macro_rules! debug {
    ( $l:expr, $( $x:expr ),* ) => {
        crate::some_log!(debug, $l, $(
                ($x)
        ),*);
    }
}

#[macro_export]
macro_rules! info {
    ( $l:expr, $( $x:expr ),* ) => {
        crate::some_log!(info, $l, $(
                ($x)
        ),*);
    }
}

#[macro_export]
macro_rules! warn {
    ( $l:expr, $( $x:expr ),* ) => {
        crate::some_log!(warn, $l, $(
                ($x)
        ),*);
    }
}
