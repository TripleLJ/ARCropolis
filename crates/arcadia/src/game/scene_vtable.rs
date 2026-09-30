use std::{ffi::CStr, marker::PhantomData};

use super::scene::{FactoryHeader, FactoryVtable, Scene, SceneChanger, SceneInitContext, SceneTableEntry};

#[repr(C)]
#[derive(Clone, Copy)]
pub struct SceneVtable {
    pub dtor: unsafe extern "C" fn(*mut Scene),
    pub deleting_dtor: unsafe extern "C" fn(*mut Scene),
    pub initialize: unsafe extern "C" fn(*mut Scene, *const SceneInitContext),
    pub request_exit: unsafe extern "C" fn(*mut Scene),
    pub is_exit_finished: unsafe extern "C" fn(*mut Scene) -> bool,
    pub update: unsafe extern "C" fn(*mut Scene, *mut SceneChanger),
    pub draw: unsafe extern "C" fn(*mut Scene),
    pub resume: unsafe extern "C" fn(*mut Scene, *mut SceneChanger, u32),
    pub handle_message: unsafe extern "C" fn(*mut Scene, *mut SceneChanger, *mut ()),

    pub uses_async_initialize: unsafe extern "C" fn(*mut Scene) -> bool,
    pub on_enter: unsafe extern "C" fn(*mut Scene, *const SceneInitContext),
    pub on_exit: unsafe extern "C" fn(*mut Scene),
    pub tick: unsafe extern "C" fn(*mut Scene, *mut SceneChanger),
    pub pre_handle_message: unsafe extern "C" fn(*mut Scene, *mut SceneChanger, *mut ()) -> bool,
    pub forward_message: unsafe extern "C" fn(*mut Scene, *mut SceneChanger, *mut ()) -> bool,
    pub on_message: unsafe extern "C" fn(*mut Scene, *mut SceneChanger, *mut ()) -> bool,
    pub slot16: unsafe extern "C" fn(*mut Scene) -> u64,
    pub slot17: unsafe extern "C" fn(*mut Scene) -> u64,
    pub slot18: unsafe extern "C" fn(*mut Scene, *mut SceneChanger),
}

const _: () = assert!(size_of::<SceneVtable>() == 19 * 8);

pub unsafe trait SceneImpl: Sized + 'static {
    const NAME: &'static CStr;

    fn create() -> Box<Self>;
    fn initialize(&mut self, ctx: &SceneInitContext);
    fn update(&mut self, changer: &mut SceneChanger);
    fn request_exit(&mut self);
    fn is_exit_finished(&mut self) -> bool;
    fn on_enter(&mut self, _ctx: &SceneInitContext) {}
    fn on_exit(&mut self) {}
    fn resume(&mut self, _code: u32) {}
    fn on_message(&mut self, _message: *mut ()) -> bool {
        false
    }
}

unsafe extern "C" fn dtor<T: SceneImpl>(_this: *mut Scene) {}

unsafe extern "C" fn deleting_dtor<T: SceneImpl>(this: *mut Scene) {
    if !this.is_null() {
        drop(Box::from_raw(this.cast::<T>()))
    }
}

unsafe extern "C" fn initialize<T: SceneImpl>(this: *mut Scene, ctx: *const SceneInitContext) {
    if let (Some(scene), Some(ctx)) = (this.cast::<T>().as_mut(), ctx.as_ref()) {
        scene.initialize(ctx)
    }
}

unsafe extern "C" fn request_exit<T: SceneImpl>(this: *mut Scene) {
    if let Some(scene) = this.cast::<T>().as_mut() {
        scene.request_exit()
    }
}

unsafe extern "C" fn is_exit_finished<T: SceneImpl>(this: *mut Scene) -> bool {
    match this.cast::<T>().as_mut() {
        Some(scene) => scene.is_exit_finished(),
        None => true,
    }
}

unsafe extern "C" fn update<T: SceneImpl>(this: *mut Scene, changer: *mut SceneChanger) {
    if let (Some(scene), Some(changer)) = (this.cast::<T>().as_mut(), changer.as_mut()) {
        scene.update(changer)
    }
}

unsafe extern "C" fn draw<T: SceneImpl>(_this: *mut Scene) {}

unsafe extern "C" fn resume<T: SceneImpl>(this: *mut Scene, _changer: *mut SceneChanger, code: u32) {
    if let Some(scene) = this.cast::<T>().as_mut() {
        scene.resume(code)
    }
}

unsafe extern "C" fn handle_message<T: SceneImpl>(_this: *mut Scene, _changer: *mut SceneChanger, _message: *mut ()) {}

unsafe extern "C" fn uses_async_initialize<T: SceneImpl>(_this: *mut Scene) -> bool {
    false
}

unsafe extern "C" fn on_enter<T: SceneImpl>(this: *mut Scene, ctx: *const SceneInitContext) {
    if let (Some(scene), Some(ctx)) = (this.cast::<T>().as_mut(), ctx.as_ref()) {
        scene.on_enter(ctx)
    }
}

unsafe extern "C" fn on_exit<T: SceneImpl>(this: *mut Scene) {
    if let Some(scene) = this.cast::<T>().as_mut() {
        scene.on_exit()
    }
}

unsafe extern "C" fn tick<T: SceneImpl>(_this: *mut Scene, _changer: *mut SceneChanger) {}

unsafe extern "C" fn pre_handle_message<T: SceneImpl>(_this: *mut Scene, _changer: *mut SceneChanger, _message: *mut ()) -> bool {
    false
}

unsafe extern "C" fn forward_message<T: SceneImpl>(this: *mut Scene, changer: *mut SceneChanger, message: *mut ()) -> bool {
    on_message::<T>(this, changer, message)
}

unsafe extern "C" fn on_message<T: SceneImpl>(this: *mut Scene, _changer: *mut SceneChanger, message: *mut ()) -> bool {
    match this.cast::<T>().as_mut() {
        Some(scene) => scene.on_message(message),
        None => false,
    }
}

unsafe extern "C" fn slot16<T: SceneImpl>(_this: *mut Scene) -> u64 {
    0
}

unsafe extern "C" fn slot17<T: SceneImpl>(_this: *mut Scene) -> u64 {
    0
}

unsafe extern "C" fn slot18<T: SceneImpl>(_this: *mut Scene, _changer: *mut SceneChanger) {}

impl SceneVtable {
    pub const fn of<T: SceneImpl>() -> SceneVtable {
        SceneVtable {
            dtor: dtor::<T>,
            deleting_dtor: deleting_dtor::<T>,
            initialize: initialize::<T>,
            request_exit: request_exit::<T>,
            is_exit_finished: is_exit_finished::<T>,
            update: update::<T>,
            draw: draw::<T>,
            resume: resume::<T>,
            handle_message: handle_message::<T>,
            uses_async_initialize: uses_async_initialize::<T>,
            on_enter: on_enter::<T>,
            on_exit: on_exit::<T>,
            tick: tick::<T>,
            pre_handle_message: pre_handle_message::<T>,
            forward_message: forward_message::<T>,
            on_message: on_message::<T>,
            slot16: slot16::<T>,
            slot17: slot17::<T>,
            slot18: slot18::<T>,
        }
    }
}

#[repr(C)]
pub struct Factory<T> {
    vtable: &'static FactoryVtable,
    _scene: PhantomData<T>,
}

unsafe impl<T> Sync for Factory<T> {}

unsafe extern "C" fn factory_dtor(_this: *mut FactoryHeader) {}

unsafe extern "C" fn factory_create<T: SceneImpl>(_this: *mut FactoryHeader) -> *mut Scene {
    Box::into_raw(T::create()).cast::<Scene>()
}

impl<T: SceneImpl> Factory<T> {
    pub const INSTANCE: Factory<T> = Factory {
        vtable: &Self::VTABLE,
        _scene: PhantomData,
    };
    const VTABLE: FactoryVtable = FactoryVtable {
        dtor: factory_dtor,
        deleting_dtor: factory_dtor,
        create: factory_create::<T>,
    };
}

#[repr(transparent)]
pub struct SceneTable<const N: usize>([SceneTableEntry; N]);

unsafe impl<const N: usize> Sync for SceneTable<N> {}

impl<const N: usize> SceneTable<N> {
    pub const fn new(rows: [SceneTableEntry; N]) -> SceneTable<N> {
        SceneTable(rows)
    }

    pub fn rows(&'static self) -> &'static [SceneTableEntry] {
        &self.0
    }
}

#[macro_export]
macro_rules! scene_table {
    ($vis:vis static $name:ident = [$($scene:ty),* $(,)?]) => {
        $vis static $name: $crate::game::scene_vtable::SceneTable<{ $crate::scene_table!(@count $($scene),*) + 1 }> =
            $crate::game::scene_vtable::SceneTable::new([
                $($crate::game::scene::SceneTableEntry {
                    name: <$scene as $crate::game::scene_vtable::SceneImpl>::NAME.as_ptr().cast::<u8>(),
                    factory: &$crate::game::scene_vtable::Factory::<$scene>::INSTANCE as *const $crate::game::scene_vtable::Factory<$scene>
                        as *const $crate::game::scene::FactoryHeader,
                },)*
                $crate::game::scene::SceneTableEntry::END,
            ]);
    };
    (@count) => { 0usize };
    (@count $head:ty $(, $tail:ty)*) => { 1usize + $crate::scene_table!(@count $($tail),*) };
}
