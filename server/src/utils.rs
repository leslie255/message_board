#![allow(dead_code)]

pub type DynLocalError = Box<dyn std::error::Error>;
pub type DynLocalResult<T> = Result<T, DynLocalError>;
pub type DynError = Box<dyn std::error::Error + Send + Sync>;
pub type DynResult<T> = Result<T, DynError>;

pub trait ServerState: Send + Sync {}
impl<T: Send + Sync> ServerState for T {}

fn infallible() -> std::convert::Infallible {
    unreachable!();
}

/// `todo!` except it returns `Infallible` instead of `!`.
#[allow(unused_macros)]
pub macro todo_($($ts:tt)*) {{
    #[allow(unreachable_code, clippy::diverging_sub_expression)]
    {
        let x: std::convert::Infallible = std::todo!($($ts)*);
        x
    }
}}
