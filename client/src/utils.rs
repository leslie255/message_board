use std::fmt::Debug;
use std::future::Future;

/// Like `Result::unwrap`, but `log::error!(..)` on error instead of printing.
pub trait PrettyUnwrap<T, E: Debug> {
    #[track_caller]
    fn pretty_unwrap(self) -> T;
}

impl<T, E: Debug> PrettyUnwrap<T, E> for Result<T, E> {
    /// Like `Result::unwrap`, but `log::error!(..)` on error instead of printing.
    #[track_caller]
    fn pretty_unwrap(self) -> T {
        match self {
            Ok(x) => x,
            Err(error) => {
                log::error!("{error:?}");
                panic!();
            }
        }
    }
}

/// `await` without the `a`.
/// Blockingly poll a future to get its output.
pub trait Wait: Future {
    /// `await` without the `a`.
    /// Blockingly poll a future to get its output.
    fn wait(self) -> Self::Output;
}

impl<F: Future> Wait for F {
    fn wait(self) -> Self::Output {
        tokio::runtime::Runtime::new()
            .pretty_unwrap()
            .block_on(self)
    }
}

