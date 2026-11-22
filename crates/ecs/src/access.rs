use std::ops::{Deref, DerefMut};

use rusty_components::registry::first_class;
use rusty_components::{ImageComponent, MeshComponent, PrefabLink};

use super::world::{Ref, RefMut, World};

pub struct CompRef<'w, T: ?Sized>(Ref<'w, T>);

impl<'w, T: ?Sized> CompRef<'w, T> {
    pub(super) fn new(guard: Ref<'w, T>) -> Self {
        Self(guard)
    }
}

impl<T: ?Sized> Deref for CompRef<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

pub struct CompMut<'w, T: ?Sized>(RefMut<'w, T>);

impl<'w, T: ?Sized> CompMut<'w, T> {
    pub(super) fn new(guard: RefMut<'w, T>) -> Self {
        Self(guard)
    }
}

impl<T: ?Sized> Deref for CompMut<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: ?Sized> DerefMut for CompMut<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

macro_rules! optional_component_accessors {
    ($($ty:ty => $get:ident, $get_mut:ident, $has:ident, $set:ident, $take:ident, $ids_with:ident;)*) => {
        impl World {
            $(
                pub fn $get(&self, id: u32) -> Option<CompRef<'_, $ty>> {
                    self.component::<$ty>(id).map(CompRef::new)
                }

                pub fn $get_mut(&mut self, id: u32) -> Option<CompMut<'_, $ty>> {
                    self.component_mut::<$ty>(id).map(CompMut::new)
                }

                pub fn $has(&self, id: u32) -> bool {
                    self.has_component::<$ty>(id)
                }

                pub fn $set(&mut self, id: u32, value: Option<$ty>) -> bool {
                    self.set_component::<$ty>(id, value)
                }

                pub fn $take(&mut self, id: u32) -> Option<$ty> {
                    self.take_component::<$ty>(id)
                }

                pub fn $ids_with(&self) -> Vec<u32> {
                    self.ids_with_component::<$ty>()
                }
            )*
        }
    };
}

macro_rules! row_accessors {
    (collider: $ty:ty, $get_mut:ident $has:ident $set:ident $take:ident $ids_with:ident) => {
        impl World {
            pub fn $take(&mut self, id: u32) -> Option<$ty> {
                self.take_component::<$ty>(id)
            }
        }
    };
    ($get:ident: $ty:ty, $($acc:ident)*) => {
        optional_component_accessors! { $ty => $get, $($acc),*; }
    };
}

macro_rules! registry_accessors {
    ($($field:ident: $ty:ty => $kind:ident [$($acc:ident)*];)*) => {
        $(row_accessors! { $field: $ty, $($acc)* })*
    };
}

first_class!(registry_accessors);

optional_component_accessors! {
    PrefabLink => prefab_link, prefab_link_mut, has_prefab_link, set_prefab_link, take_prefab_link, ids_with_prefab_link;
}

impl World {
    pub fn each_image(&self, visit: impl FnMut(&ImageComponent)) {
        self.each_component(visit);
    }

    pub fn ids_with_mesh_where(&self, keep: impl Fn(&MeshComponent) -> bool) -> Vec<u32> {
        self.ids_with_component_where(keep)
    }

    pub fn set_pose_palette(&mut self, id: u32, palette: &[glam::Mat4]) {
        if let Some(mut mesh) = self.derived_mut::<MeshComponent>(id) {
            mesh.pose_palette.clear();
            mesh.pose_palette.extend_from_slice(palette);
        }
    }
}
