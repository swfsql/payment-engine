pub mod chain;
pub mod prepared;
pub mod token;
pub mod upgrade;

pub use chain::Chain;
pub use prepared::Prepared;
pub use token::{ConsumedToken, DowngradedTokenProtected, Token, TokenProtected};
pub use upgrade::Upgrade;

pub mod target {
    pub struct Type;
    pub struct Token;
    pub struct Function;
    #[derive(Debug)]
    pub struct UpperToken;
}

pub trait Take<T, Target> {
    fn take_ref(&self) -> &T;
    fn take_mut(&mut self) -> &mut T;
}

pub trait TakeOwned<T, Target> {
    fn take_owned(self) -> T;
}

pub trait PartialApply<T, F, E> {
    fn get_next(&self) -> T;
    fn modify_next(next: T, f: F) -> Result<T, E>;
    fn replace(&mut self, next: T);
}

pub trait Apply<'t, T, F, E> {
    fn consume_token(self) -> ConsumedToken<'t, T>;
    fn apply(self) -> Result<ConsumedToken<'t, T>, E>;
}
