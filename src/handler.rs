//! State handlers determine how state should be created, modified, and shared.
use std::any::type_name;
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use yew::{
    format::Json,
    services::{storage::Area, StorageService},
};

pub(crate) type Reduction<T> = Rc<dyn Fn(&mut T)>;
pub(crate) type ReductionOnce<T> = Box<dyn FnOnce(&mut T)>;

/// Determines how state should be created, modified, and shared.
pub trait Handler {
    type Model;

    /// Create new state.
    fn new() -> Self;
    /// Apply changes to state.
    fn apply(&mut self, f: Reduction<Self::Model>);
    /// Apply changes to state once.
    fn apply_once(&mut self, f: ReductionOnce<Self::Model>);
    /// Return a reference to current state.
    fn state(&self) -> Rc<Self::Model>;
}

/// Handler for basic shared state.
#[derive(Default, Clone)]
pub struct SharedHandler<T> {
    state: Rc<T>,
}

impl<T> Handler for SharedHandler<T>
where
    T: Clone + Default,
{
    type Model = T;

    fn new() -> Self {
        Default::default()
    }

    fn apply(&mut self, f: Reduction<Self::Model>) {
        f(Rc::make_mut(&mut self.state));
    }

    fn apply_once(&mut self, f: ReductionOnce<Self::Model>) {
        f(Rc::make_mut(&mut self.state));
    }

    fn state(&self) -> Rc<Self::Model> {
        Rc::clone(&self.state)
    }
}

/// Allows state to be stored persistently in local or session storage.
pub trait Storable: Serialize + for<'a> Deserialize<'a> {
    /// The key used to save and load state from storage.
    fn key() -> &'static str {
        type_name::<Self>()
    }
    /// The area to store state.
    fn area() -> Area {
        Area::Local
    }
}

/// Handler for shared state with persistent storage.
///
/// If persistent storage is disabled it just behaves like a `SharedHandler`.
#[derive(Default)]
pub struct StorageHandler<T> {
    state: Rc<T>,
    storage: Option<StorageService>,
}

impl<T> StorageHandler<T>
where
    T: Storable,
{
    fn load_state(&mut self) {
        let result = self.storage.as_mut().map(|s| s.restore(T::key()));
        if let Some(Json(Ok(state))) = result {
            self.state = state;
        }
    }

    fn save_state(&mut self) {
        if let Some(storage) = &mut self.storage {
            storage.store(T::key(), Json(&self.state));
        }
    }
}

impl<T> Handler for StorageHandler<T>
where
    T: Default + Clone + Storable,
{
    type Model = T;

    fn new() -> Self {
        let mut this: Self = Default::default();
        this.storage = StorageService::new(T::area()).ok();
        this.load_state();
        this
    }

    fn apply(&mut self, f: Reduction<Self::Model>) {
        f(Rc::make_mut(&mut self.state));
        self.save_state();
    }

    fn apply_once(&mut self, f: ReductionOnce<Self::Model>) {
        f(Rc::make_mut(&mut self.state));
        self.save_state();
    }

    fn state(&self) -> Rc<Self::Model> {
        Rc::clone(&self.state)
    }
}

impl<T> Clone for StorageHandler<T>
where
    T: Default + Clone + Storable,
{
    fn clone(&self) -> Self {
        let mut new = Self::new();
        // State should already be correct because it's loaded from storage,
        // but it won't be if storage is disabled.
        new.state = self.state.clone();
        new
    }
}
