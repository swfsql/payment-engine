
use super::{target, Apply, Chain, ConsumedToken, PartialApply, Take, TakeOwned, Token, Upgrade};
use std::marker::PhantomData;

pub struct Prepared<OuterT, T, F, E> {
    inner: OuterT,
    f: F,
    _t: PhantomData<T>,
    _err: PhantomData<E>,
}

impl<'t, OuterT, T, FInner, E> Take<FInner, target::Function> for Prepared<OuterT, T, FInner, E> {
    fn take_ref(&self) -> &FInner {
        &self.f
    }

    fn take_mut(&mut self) -> &mut FInner {
        &mut self.f
    }
}

impl<'t, OuterT, T, FInner, E> TakeOwned<FInner, target::Function>
    for Prepared<OuterT, T, FInner, E>
{
    fn take_owned(self) -> FInner {
        self.f
    }
}

impl<'t, OuterT, T, FInner, E> Take<Token<'t, T>, target::Token> for Prepared<OuterT, T, FInner, E>
where
    OuterT: Take<Token<'t, T>, target::Token>,
{
    fn take_ref(&self) -> &Token<'t, T> {
        self.inner.take_ref()
    }

    fn take_mut(&mut self) -> &mut Token<'t, T> {
        self.inner.take_mut()
    }
}

impl<'t, OuterT, T, FInner, E> TakeOwned<Token<'t, T>, target::Token>
    for Prepared<OuterT, T, FInner, E>
where
    OuterT: TakeOwned<Token<'t, T>, target::Token>,
{
    fn take_owned(self) -> Token<'t, T> {
        self.inner.take_owned()
    }
}

impl<'u, 'l, OuterT, U, L, FInner, E> Take<&'u Token<'u, U>, target::UpperToken>
    for Prepared<OuterT, L, FInner, E>
where
    OuterT: Take<&'u Token<'u, U>, target::UpperToken>,
{
    fn take_ref(&self) -> &&'u Token<'u, U> {
        self.inner.take_ref()
    }

    fn take_mut(&mut self) -> &mut &'u Token<'u, U> {
        self.inner.take_mut()
    }
}

impl<'u, 'l, OuterT, U, L, FInner, E> TakeOwned<&'u Token<'u, U>, target::UpperToken>
    for Prepared<OuterT, L, FInner, E>
where
    OuterT: TakeOwned<&'u Token<'u, U>, target::UpperToken>,
{
    fn take_owned(self) -> &'u Token<'u, U> {
        self.inner.take_owned()
    }
}

impl<OuterT, T, F, E> Prepared<OuterT, T, F, E> {
    pub fn new(outer: OuterT, f: F) -> Self {
        Self {
            inner: outer,
            f,
            _t: PhantomData,
            _err: PhantomData,
        }
    }
    pub fn chain<A2>(self, a2: A2) -> Chain<Self, A2>
where {
        Chain::new(self, a2)
    }
    pub fn upgrade<U>(self) -> Upgrade<Self, U, T, E> {
        Upgrade::new(self)
    }
}

impl<'t, OuterT, T, F, E> PartialApply<T, F, E> for Prepared<OuterT, T, F, E>
where
    OuterT: Take<T, target::Type> + Take<Token<'t, T>, target::Token>,
    F: FnOnce(&mut T) -> Result<(), E>,
    T: 't + Clone,
    OuterT: 't,
{
    fn get_next(&self) -> T {
        let next: &T = self.inner.take_ref();
        next.clone()
    }

    fn modify_next(mut next: T, f: F) -> Result<T, E> {
        (f)(&mut next)?;
        Ok(next)
    }

    fn replace(&mut self, next: T) {
        let current: &mut T = self.inner.take_mut();
        *current = next;
    }
}

impl<'t, OuterT, T, F, E> Apply<'t, T, F, E> for Prepared<OuterT, T, F, E>
where
    Self: PartialApply<T, F, E>,
    OuterT: Take<Token<'t, T>, target::Token> + TakeOwned<Token<'t, T>, target::Token>,
    T: 't,
    E: 't,
    F: 't + Clone,
    OuterT: 't,
{
    fn consume_token(self) -> ConsumedToken<'t, T> {
        let t = self.inner.take_owned();
        ConsumedToken::from(t)
    }
    fn apply(mut self) -> Result<ConsumedToken<'t, T>, E> {
        let next = self.get_next();
        let f = self.f.clone();
        let next = Self::modify_next(next, f)?;
        self.replace(next);
        //
        let t = self.inner.take_owned();
        let consumed = ConsumedToken::from(t);
        Ok(consumed)
    }
}
