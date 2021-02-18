macro_rules! impl_filter{
    () => {
        impl Filter<Extract = (Response,), Error = Rejection> + Clone + Send + Sync + 'static
    }
}
