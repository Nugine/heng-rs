pub trait ResultExt<T, E> {
    fn inspect_err(self, f: impl FnOnce(&mut E)) -> Self;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    fn inspect_err(mut self, f: impl FnOnce(&mut E)) -> Self {
        if let Err(ref mut err) = self {
            f(err)
        }
        self
    }
}
