use std::pin::Pin;

pub fn compose<A, B, C, F, G>(f: F, g: G) -> Pin<Box<dyn Fn(A) -> C>>
where
    F: Fn(A) -> B + 'static,
    G: Fn(B) -> C + 'static,
{
    Box::pin(move |a: A| g(f(a)))
}

#[cfg(test)]
mod tests {
    use super::compose;

    #[derive(Debug, PartialEq, Eq)]
    struct Thing {
        i: i32
    }

    impl Thing {
        fn new(i: i32) -> Self {
            Self { i }
        }
    }

    #[test]
    fn test_compose() {
        let create = compose(Thing::new, Some);
        let something = create(1);
        assert_eq!(something, Some(Thing { i: 1 }));
    }
}
