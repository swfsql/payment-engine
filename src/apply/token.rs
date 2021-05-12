use super::{target, Prepared, Take, TakeOwned};
use std::marker::PhantomData;

#[derive(Debug)]
pub struct Token<'t, T>(PhantomData<&'t T>);

#[derive(Debug)]
pub struct ConsumedToken<'t, T>(PhantomData<&'t T>);

pub struct TokenProtected<'t, T> {
    inner: &'t mut T,
    token: Token<'t, T>,
}

pub struct DowngradedTokenProtected<'u, 'l, U, L, LowerProtected> {
    upper_token: &'u Token<'u, U>,
    lower_protected: LowerProtected,
    lower_info: PhantomData<&'l L>,
}

impl<'t, T> Token<'t, T> {
    fn new(t: &'t mut T) -> (Self, &'t mut T) {
        (Self(PhantomData), t)
    }
    pub fn then<'t2, 'tboth, T2>(self, _token2: Token<'t2, T2>) -> Token<'tboth, (T, T2)> {
        Token(PhantomData)
    }
    pub fn consume(self) -> ConsumedToken<'t, T> {
        self.into()
    }
    pub unsafe fn with_downgrade<'l, L>(
        &'t mut self,
        lower: TokenProtected<'l, L>,
    ) -> DowngradedTokenProtected<'t, 'l, T, L, TokenProtected<'l, L>> {
        DowngradedTokenProtected::new(self, lower)
    }
    pub unsafe fn force_consume(&self) -> ConsumedToken<'t, T> {
        ConsumedToken(self.0)
    }
}

impl<'t, T> ConsumedToken<'t, T> {
    pub fn then<'t2, 'tboth, T2>(
        self,
        _token2: ConsumedToken<'t2, T2>,
    ) -> ConsumedToken<'tboth, (T, T2)> {
        ConsumedToken(PhantomData)
    }
}

impl<'t, T> From<Token<'t, T>> for ConsumedToken<'t, T> {
    fn from(token: Token<'t, T>) -> Self {
        ConsumedToken(token.0)
    }
}

impl<'u, 'l, U, L, LowerProtected> Take<L, target::Type>
    for DowngradedTokenProtected<'u, 'l, U, L, LowerProtected>
where
    LowerProtected: Take<L, target::Type>,
{
    fn take_ref(&self) -> &L {
        self.lower_protected.take_ref()
    }

    fn take_mut(&mut self) -> &mut L {
        self.lower_protected.take_mut()
    }
}
impl<'u, 'l, U, L, LowerProtected> TakeOwned<L, target::Type>
    for DowngradedTokenProtected<'u, 'l, U, L, LowerProtected>
where
    LowerProtected: TakeOwned<L, target::Type>,
{
    fn take_owned(self) -> L {
        self.lower_protected.take_owned()
    }
}

impl<'u, 'l, U, L, LowerProtected> Take<Token<'l, L>, target::Token>
    for DowngradedTokenProtected<'u, 'l, U, L, LowerProtected>
where
    LowerProtected: Take<Token<'l, L>, target::Token>,
{
    fn take_ref(&self) -> &Token<'l, L> {
        self.lower_protected.take_ref()
    }

    fn take_mut(&mut self) -> &mut Token<'l, L> {
        self.lower_protected.take_mut()
    }
}

impl<'u, 'l, U, L, LowerProtected> TakeOwned<Token<'l, L>, target::Token>
    for DowngradedTokenProtected<'u, 'l, U, L, LowerProtected>
where
    LowerProtected: TakeOwned<Token<'l, L>, target::Token>,
{
    fn take_owned(self) -> Token<'l, L> {
        self.lower_protected.take_owned()
    }
}

impl<'u, 'l, U, L, LowerProtected> Take<&'u Token<'u, U>, target::UpperToken>
    for DowngradedTokenProtected<'u, 'l, U, L, LowerProtected>
{
    fn take_ref(&self) -> &&'u Token<'u, U> {
        &self.upper_token
    }

    fn take_mut(&mut self) -> &mut &'u Token<'u, U> {
        &mut self.upper_token
    }
}

impl<'u, 'l, U, L, LowerProtected> TakeOwned<&'u Token<'u, U>, target::UpperToken>
    for DowngradedTokenProtected<'u, 'l, U, L, LowerProtected>
{
    fn take_owned(self) -> &'u Token<'u, U> {
        self.upper_token
    }
}

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
    pub unsafe fn as_mut(&mut self) -> &'_ mut T {
        self.inner
    }
    pub unsafe fn split_mut(&mut self) -> (&'_ mut T, &'_ mut Token<'t, T>) {
        (self.inner, &mut self.token)
    }
    pub fn prepare<F, E>(self, f: F) -> Prepared<TokenProtected<'t, T>, T, F, E> {
        Prepared::new(self, f)
    }
    pub fn skip<E>(self) -> Prepared<TokenProtected<'t, T>, T, fn(&mut T) -> Result<(), E>, E> {
        Prepared::new(self, |_| Ok(()))
    }

    pub fn consume(self) -> ConsumedToken<'t, T> {
        self.token.consume()
    }
    pub unsafe fn downgrade<'l, L>(
        &'t mut self,
        lower: TokenProtected<'l, L>,
    ) -> DowngradedTokenProtected<'t, 'l, T, L, TokenProtected<'l, L>> {
        DowngradedTokenProtected::new(&self.token, lower)
    }
}

impl<'u, 'l, U, L, LowerProtected> AsRef<L>
    for DowngradedTokenProtected<'u, 'l, U, L, LowerProtected>
where
    LowerProtected: Take<L, target::Type>,
{
    fn as_ref(&self) -> &L {
        self.lower_protected.take_ref()
    }
}

impl<'u, 'l, U, L, LowerProtected> DowngradedTokenProtected<'u, 'l, U, L, LowerProtected> {
    pub fn new(upper_token: &'u Token<'u, U>, lower_protected: LowerProtected) -> Self {
        Self {
            upper_token,
            lower_protected,
            lower_info: PhantomData,
        }
    }
    pub fn prepare<F, E>(
        self,
        f: F,
    ) -> Prepared<DowngradedTokenProtected<'u, 'l, U, L, LowerProtected>, L, F, E> {
        Prepared::new(self, f)
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

impl<'t, T> TakeOwned<&'t mut T, target::Type> for TokenProtected<'t, T> {
    fn take_owned(self) -> &'t mut T {
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
