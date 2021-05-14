use super::{target, Prepared, Take, TakeOwned};
use std::marker::PhantomData;
use TokenProtected as TP;

/// A tag value that can be moved or consumed, and indicates
/// that modifications into `T` can happen only in a single scope.
///
/// See also `TokenProtected`.
#[derive(Debug)]
pub struct Token<'t, T>(PhantomData<&'t T>);

/// A tag value that can be moved, and indicates that `T` will
/// no longer be able to be modified.
#[derive(Debug)]
pub struct ConsumedToken<'t, T>(PhantomData<&'t T>);

/// A tag value related to containers.
///
/// This indicates that the container had some accessed item, and
/// depending on whether this item was modified or not, this tag can be
/// moved, consumed or be restored back into a `Token`.
#[derive(Debug)]
pub struct UpgraderToken<'u, 'l, U, L> {
    upper: Token<'u, U>,
    lower: PhantomData<&'l L>,
}

/// Wraps a mutable access into `T`, allowing shared access or a
/// controlled exclusive access.
///
/// By using `Token`, this only allows mutation on a single scope.
/// All changes must be "prepared", will happen in a copy of `T`,
/// and only after "applying" the modification, the original `T` will
/// be replaced by the modified copy of `T`.
pub struct TokenProtected<'t, T> {
    inner: &'t mut T,
    token: Token<'t, T>,
}

impl<'u, 'l, U, L> UpgraderToken<'u, 'l, U, L> {
    pub fn new(upper: Token<'u, U>, _lower: &TP<'l, L>) -> Self {
        Self {
            upper,
            lower: PhantomData,
        }
    }

    /// Consumes the token without changing `T` (as it's innaccesible).
    pub fn consume(self, _lower: ConsumedToken<'l, L>) -> ConsumedToken<'u, U> {
        self.upper.into()
    }

    /// Consumes the token without changing `T` (as it's innaccesible).
    pub fn returned(self, _lower: impl Into<Token<'l, L>>) -> Token<'u, U> {
        self.upper
    }

    /// Discards an unconsummed lower Token, and extracets the upper one.
    pub fn discard_lower(self, _lower: Token<'l, L>) -> Token<'u, U> {
        self.upper
    }
}

impl<'t, T> Token<'t, T> {
    fn new(t: &'t mut T) -> (Self, &'t mut T) {
        (Self(PhantomData), t)
    }

    /// Concatenate this token with another one.
    pub fn then<'t2, 'tboth, T2>(self, _token2: Token<'t2, T2>) -> Token<'tboth, (T, T2)> {
        Token(PhantomData)
    }

    /// Consumes the token.
    pub fn consume(self) -> ConsumedToken<'t, T> {
        self.into()
    }
}

impl<'t1, 't2, 'tboth, T1, T2> Token<'tboth, (T1, T2)>
where
    'tboth: 't1 + 't2,
{
    pub fn split2(self) -> (Token<'t1, T1>, Token<'t2, T2>) {
        (Token(PhantomData), Token(PhantomData))
    }
}

impl<'t, T> ConsumedToken<'t, T> {
    /// Concatenates with another Consumed Token.
    pub fn then<'t2, 'tboth, T2>(
        self,
        _token2: ConsumedToken<'t2, T2>,
    ) -> ConsumedToken<'tboth, (T, T2)> {
        ConsumedToken(PhantomData)
    }
}

impl<'t1, 't2, 'tboth, T1, T2> ConsumedToken<'tboth, (T1, T2)>
where
    'tboth: 't1 + 't2,
{
    pub fn split2(self) -> (ConsumedToken<'t1, T1>, ConsumedToken<'t2, T2>) {
        (ConsumedToken(PhantomData), ConsumedToken(PhantomData))
    }
}

/// Consumes a Token.
impl<'t, T> From<Token<'t, T>> for ConsumedToken<'t, T> {
    fn from(token: Token<'t, T>) -> Self {
        ConsumedToken(token.0)
    }
}

/// Allows shared access into the Token.
impl<'t, T> AsRef<T> for TokenProtected<'t, T> {
    fn as_ref(&self) -> &T {
        self.inner
    }
}

impl<'t, T> TokenProtected<'t, T> {
    pub fn new(inner: &'t mut T) -> Self {
        let (token, inner) = Token::new(inner);
        Self { inner, token }
    }

    /// Prepares modifications into a copy of `T`.
    ///
    /// See also `Prepared::apply` for executing the changes on the copy
    /// and replacing them into the original `T`.
    pub fn prepare<F, E>(self, f: F) -> Prepared<TokenProtected<'t, T>, T, F, E> {
        Prepared::new(self, f)
    }

    /// Doesn't change `T`, but returns an identity `Prepared` that may be
    /// chained with other `Prepared` values.
    #[allow(clippy::type_complexity)]
    pub fn skip<E>(self) -> Prepared<TokenProtected<'t, T>, T, fn(&mut T) -> Result<(), E>, E> {
        Prepared::new(self, |_| Ok(()))
    }

    /// Consumes the token without changing `T`.
    pub fn consume(self) -> ConsumedToken<'t, T> {
        self.token.consume()
    }

    /// Consumes the container `&'t mut T` to create an item `TP<L>`,
    /// while storing the container's token into `UpgraderToken`.
    /// The item `L` may later be modified, and the item's consumed
    /// token may be upgraded into a container's consumed token.
    ///
    /// # Safety
    ///
    /// `f1` must ensure that the container is not modified,
    /// such as pushing or removing items into it.
    pub unsafe fn downgrade<'l, F1, L>(self, f1: F1) -> (UpgraderToken<'t, 'l, T, L>, TP<'l, L>)
    where
        F1: FnOnce(&'t mut T) -> &'l mut L,
    {
        let l = TP::new(f1(self.inner));
        let u = UpgraderToken::new(self.token, &l);
        (u, l)
    }

    pub fn token(self) -> Token<'t, T> {
        self.token
    }
}

impl<'t, T> From<TokenProtected<'t, T>> for Token<'t, T> {
    fn from(t: TokenProtected<'t, T>) -> Self {
        t.token()
    }
}

impl<'t, T> Take<T, target::Type> for TokenProtected<'t, T> {
    fn take_ref(&self) -> &T {
        self.inner
    }

    fn take_mut(&mut self) -> &mut T {
        self.inner
    }
}

impl<'t, T> Take<Token<'t, T>, target::Token> for TokenProtected<'t, T> {
    fn take_ref(&self) -> &Token<'t, T> {
        &self.token
    }

    fn take_mut(&mut self) -> &mut Token<'t, T> {
        &mut self.token
    }
}

impl<'t, T> TakeOwned<Token<'t, T>, target::Token> for TokenProtected<'t, T> {
    fn take_owned(self) -> Token<'t, T> {
        self.token
    }
}
