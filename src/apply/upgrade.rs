use super::{target, Apply, ConsumedToken, PartialApply, Take, TakeOwned, Token};
use std::marker::PhantomData;

pub struct Upgrade<OuterT, U, L, E> {
    pub inner: OuterT,
    _u: PhantomData<U>,
    _l: PhantomData<L>,
    _err: PhantomData<E>,
}

impl<OuterT, U, L, E> Upgrade<OuterT, U, L, E> {
    pub fn new(inner: OuterT) -> Self {
        Self {
            inner,
            _u: PhantomData,
            _l: PhantomData,
            _err: PhantomData,
        }
    }
}

impl<'t, OuterT, U, L, E, FInner> Take<FInner, target::Function> for Upgrade<OuterT, U, L, E>
where
    OuterT: Take<FInner, target::Function>,
{
    fn take_ref(&self) -> &FInner {
        self.inner.take_ref()
    }

    fn take_mut(&mut self) -> &mut FInner {
        self.inner.take_mut()
    }
}

impl<'l, OuterT, U, L, E> TakeOwned<Token<'l, L>, target::Token> for Upgrade<OuterT, U, L, E>
where
    OuterT: TakeOwned<Token<'l, L>, target::Token>,
{
    fn take_owned(self) -> Token<'l, L> {
        self.inner.take_owned()
    }
}

impl<'u, 'l, OuterT, U, L, F, E> PartialApply<L, F, E> for Upgrade<OuterT, U, L, E>
where
    OuterT: PartialApply<L, F, E>,
    F: FnOnce(&mut L) -> Result<(), E>,
    U: 'u,
    L: 'l,
{
    fn get_next(&self) -> L {
        OuterT::get_next(&self.inner)
    }

    fn modify_next(next: L, f: F) -> Result<L, E> {
        OuterT::modify_next(next, f)
    }

    fn replace(&mut self, next: L) {
        OuterT::replace(&mut self.inner, next)
    }
}

impl<'u, 'l, OuterT, U, L, F, E> Apply<'u, U, F, E> for Upgrade<OuterT, U, L, E>
where
    OuterT: Apply<'l, L, F, E> + Take<&'u Token<'u, U>, target::UpperToken>,
    U: 'u,
    L: 'l,
{
    fn consume_token(self) -> ConsumedToken<'u, U> {
        let upper_token = self.inner.take_ref();
        // safety: TODO
        let forced_consumed_upper_token = unsafe { upper_token.force_consume() };
        forced_consumed_upper_token
    }
    fn apply(self) -> Result<ConsumedToken<'u, U>, E> {
        let upper_token: &Token<U> = self.inner.take_ref();
        // safety: TODO
        // this will only be returned if no error occurs. And in that case,
        // the upper non-consumed token will be droped.
        //
        // so there will be no situation where both non-consumed and consumed
        // upper tokens will co-exist
        let forced_consumed_upper_token = unsafe { upper_token.force_consume() };

        let _inner_token = self.inner.apply()?;
        Ok(forced_consumed_upper_token)
    }
}
