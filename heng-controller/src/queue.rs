pub struct Queue<T> {
    tx: async_channel::Sender<T>,
    rx: async_channel::Receiver<T>,
}

impl<T: Send> Queue<T> {
    pub fn unbounded() -> Self {
        let (tx, rx) = async_channel::unbounded();
        Self { tx, rx }
    }

    pub async fn push(&self, value: T) {
        self.tx.send(value).await.unwrap();
    }

    pub async fn pop(&self) -> T {
        self.rx.recv().await.unwrap()
    }
}
