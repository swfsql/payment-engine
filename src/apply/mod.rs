pub mod macros;

pub mod chain;
pub mod prepared;
pub mod token;

pub use chain::Chain;
pub use prepared::Prepared;
pub use token::{ConsumedToken, Token, TokenProtected};

/// Return type enforcing either that _all_ of the Tokens were consumed,
/// or that _none_ of the Tokens were consumed.
pub type TResult<'t, T, E> = std::result::Result<ConsumedToken<'t, T>, (E, Token<'t, T>)>;

/// Information to diverge some access trait implementations.
pub mod target {
    /// Access into a protected type.
    pub struct Type;

    /// Access into a token.
    pub struct Token;

    /// Access into a function.
    pub struct Function;
}

/// Indicates access into fields.
pub trait Take<T, Target> {
    fn take_ref(&self) -> &T;
    fn take_mut(&mut self) -> &mut T;
}

/// Indicates access into fields.
pub trait TakeOwned<T, Target> {
    fn take_owned(self) -> T;
}

pub trait PartialApply<T, F, E> {
    /// Creates a copy of `T`.
    fn get_next(&self) -> T;
    /// Applies a modification into a `T` (presumably the copy of `T`).
    fn modify_next(next: T, f: F) -> Result<T, E>;
    /// Replaces the original `T` with the modified copy of `T`.
    fn replace(&mut self, next: T);
}

pub trait Apply<'t, T, F, E> {
    /// Consumes the token.
    fn consume_token(self) -> ConsumedToken<'t, T>;
    /// Creates a copy of `T`, modifies it, and then replaces it into the
    /// original `T`.
    fn apply(self) -> Result<ConsumedToken<'t, T>, (E, Token<'t, T>)>;
}
