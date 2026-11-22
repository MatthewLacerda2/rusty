use std::any::{Any, TypeId};

#[derive(Default)]
pub(crate) struct Targets(Vec<Box<dyn Any + Send + Sync>>);

impl Targets {
    pub(crate) fn get<T: Any>(&self) -> Option<&T> {
        self.0.iter().find_map(|s| s.downcast_ref())
    }

    pub(crate) fn get_mut<T: Any>(&mut self) -> Option<&mut T> {
        self.0.iter_mut().find_map(|s| s.downcast_mut())
    }

    pub(crate) fn get_or<T: Any + Send + Sync>(&mut self, make: impl FnOnce() -> T) -> &mut T {
        let at = match self.0.iter().position(|s| s.is::<T>()) {
            Some(at) => at,
            None => {
                self.0.push(Box::new(make()));
                self.0.len() - 1
            }
        };
        self.0[at].downcast_mut().expect("found by its type")
    }

    pub(crate) fn remove<T: Any>(&mut self) {
        self.0.retain(|s| (**s).type_id() != TypeId::of::<T>());
    }

    pub(crate) fn clear(&mut self) {
        self.0.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::Targets;

    #[test]
    fn a_slot_per_type_made_once_and_dropped_alone() {
        let mut t = Targets::default();
        *t.get_or(|| 1u32) += 1;
        assert_eq!(*t.get_or(|| 9u32), 2, "made once");
        t.get_or(|| 0.5f32);
        t.remove::<u32>();
        assert_eq!((t.get::<u32>(), t.get::<f32>()), (None, Some(&0.5)));
        t.clear();
        assert!(t.get::<f32>().is_none());
    }
}
