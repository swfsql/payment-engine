use chain::Chain;
use prepared::Prepared;
use std::marker::PhantomData;

pub trait Prepare
where
    Self: Sized,
{
    /// Creates a lazy-executable for state change.
    fn prepare<E, F>(&mut self, f: F) -> Prepared<'_, Self, E, F>
    where
        F: FnOnce(&mut Self) -> Result<(), E>,
    {
        Prepared::new(self, f)
    }
}
impl<T> Prepare for T {}

pub trait PartialApply<'s, S, E, F>: Sized {
    /// Split information, such as the current state and the
    /// state-changing procedure.
    fn split(self) -> (&'s mut S, F);
    /// Calculates the next state.
    fn partial_apply(s: &S, f: F) -> Result<S, E>;
    /// Replaces the next state into `_self`.
    fn replace(_self: &mut S, next: S);
}

pub trait Apply<'s, S, E, F>: Sized {
    /// First executes all the preparations, and then applies the changes.
    fn apply(self, token: Token<S>) -> Result<(), E>;
}

#[derive(Debug)]
pub struct Token<'t, T>(PhantomData<&'t T>);

impl<'t, T> Token<'t, T> {
    pub fn new(t: &'t mut T) -> (Self, &'t mut T) {
        (Self(PhantomData), t)
    }
    pub fn then<'t2, 'tboth, T2>(self, _token2: Token<'t2, T2>) -> Token<'tboth, (T, T2)>
    where
        'tboth: 't + 't2,
    {
        Token(PhantomData)
    }
}

mod prepared {
    use super::{Apply, Chain, PartialApply, Token};
    use std::marker::PhantomData;

    /// Lazy execution of state changes.
    pub struct Prepared<'s, S, E, F> {
        pub _self: &'s mut S,
        pub f: F,
        err_ph: PhantomData<E>,
    }

    impl<'s1, S1, E, F1> Prepared<'s1, S1, E, F1> {
        pub fn new(_self: &'s1 mut S1, f: F1) -> Self {
            Self {
                _self,
                f,
                err_ph: PhantomData,
            }
        }
        pub fn chain<'s2, S2, F2>(
            self,
            a2: Prepared<'s2, S2, E, F2>,
        ) -> Chain<Prepared<'s1, S1, E, F1>, Prepared<'s2, S2, E, F2>> {
            Chain::new(self, a2)
        }
    }

    impl<'s, S, E, F> PartialApply<'s, S, E, F> for Prepared<'s, S, E, F>
    where
        S: Clone,
        F: FnOnce(&mut S) -> Result<(), E>,
    {
        fn split(self) -> (&'s mut S, F) {
            (self._self, self.f)
        }

        fn partial_apply(s: &S, f: F) -> Result<S, E> {
            let mut next: S = s.clone();
            (f)(&mut next)?;
            Ok(next)
        }

        fn replace(_self: &mut S, next: S) {
            *_self = next;
        }
    }

    impl<'s, S, E, F> Apply<'s, S, E, F> for Prepared<'s, S, E, F>
    where
        Self: PartialApply<'s, S, E, F>,
        S: 's,
    {
        fn apply(self, _token: Token<'_, S>) -> Result<(), E> {
            let (_self, f) = self.split();
            let next = Self::partial_apply(_self, f)?;
            Self::replace(_self, next);
            Ok(())
        }
    }
}

mod chain {
    use super::{Apply, PartialApply, Prepared, Token};

    /// Lazy execution of a chain of state changes.
    /// First all preparations are done, and then
    /// all state modifications are executed.
    pub struct Chain<A1, A2> {
        a1: A1,
        a2: A2,
    }

    impl<A1, A2> Chain<A1, A2> {
        pub fn new(a1: A1, a2: A2) -> Self {
            Self { a1, a2 }
        }
        pub fn chain<S3, E, F3>(
            self,
            a3: Prepared<'_, S3, E, F3>,
        ) -> Chain<Chain<A1, A2>, Prepared<'_, S3, E, F3>> {
            Chain { a1: self, a2: a3 }
        }
    }

    impl<'s1, 's2, 's, S1, S2, E, F1, F2, A1, A2> Apply<'s, (S1, S2), E, (F1, F2)> for Chain<A1, A2>
    where
        A1: PartialApply<'s1, S1, E, F1>,
        A2: PartialApply<'s2, S2, E, F2>,
        S1: 's1,
        S2: 's2,
    {
        fn apply(self, _token: Token<'_, (S1, S2)>) -> Result<(), E> {
            let (s1, f1) = self.a1.split();
            let s1_next = A1::partial_apply(s1, f1)?;

            let (s2, f2) = self.a2.split();
            let s2_next = A2::partial_apply(s2, f2)?;

            // replacements are done at the end,
            // after all preparations
            A1::replace(s1, s1_next);
            A2::replace(s2, s2_next);

            Ok(())
        }
    }
}
