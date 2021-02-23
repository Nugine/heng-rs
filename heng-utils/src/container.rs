use std::any::{self, Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

use once_cell::sync::OnceCell;

#[derive(Debug, Default)]
pub struct Container {
    map: HashMap<TypeId, Arc<dyn Any + Send + Sync + 'static>>,
}

static GLOBAL_CONTAINER: OnceCell<Container> = OnceCell::new();

impl Container {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn register<T>(&mut self, value: Arc<T>) -> Option<Arc<T>>
    where
        T: Any + Send + Sync + 'static,
    {
        let id = TypeId::of::<T>();
        let prev = self.map.insert(id, value)?;
        Some(Arc::downcast(prev).unwrap())
    }

    pub fn inject<T>(&self) -> Option<Arc<T>>
    where
        T: Any + Send + Sync + 'static,
    {
        let id = TypeId::of::<T>();
        let value = self.map.get(&id)?.clone();
        Some(Arc::downcast(value).unwrap())
    }

    pub fn install_global(self) {
        if GLOBAL_CONTAINER.set(self).is_err() {
            panic!("global container has been installed")
        }
    }

    pub fn global() -> &'static Self {
        GLOBAL_CONTAINER.get().unwrap()
    }
}

pub fn inject<T>() -> Arc<T>
where
    T: Any + Send + Sync + 'static,
{
    match Container::global().inject::<T>() {
        Some(x) => x,
        None => panic!("failed to inject type {:?}", any::type_name::<T>()),
    }
}
