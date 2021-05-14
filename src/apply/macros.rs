#[macro_export]
macro_rules! try_on {
    ( $id:ident, $( $token:expr ),+ ) => {
        match $id {
            Ok(id) => id,
            Err(e) => return Err((e, try_on!(@expand_tokens $($token),+ ) )),
        }
    };
    (@expand_tokens $last_token:expr) => {
        $crate::Token::from($last_token)
    };

    (@expand_tokens $first_token:expr, $( $tail_tokens:expr),+ ) => {
        $crate::Token::from($first_token).then( try_on!(@expand_tokens $($tail_tokens),+ ) )
    };
}

#[macro_export]
macro_rules! err {
    ( $err:expr, $( $token:expr ),+ ) => {
        Err(($err, try_on!(@expand_tokens $($token),+ ) ))
    };
}
