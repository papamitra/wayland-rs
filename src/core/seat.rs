use std::cell::{Cell, RefCell};
use std::ffi::CStr;
use std::rc::Rc;

use libc::{c_void, c_char, uint32_t};

use super::{From, Registry, Pointer, WSurface};

use ffi::interfaces::seat::{wl_seat, wl_seat_destroy, wl_seat_listener, wl_seat_add_listener};
use ffi::enums::wl_seat_capability;
use ffi::{FFI, Bind, abi};

struct SeatData {
    name: RefCell<Vec<u8>>,
    pointer: Cell<bool>,
    keyboard: Cell<bool>,
    touch: Cell<bool>
}

impl SeatData {
    fn new() -> SeatData {
        SeatData {
            name: RefCell::new(Vec::new()),
            pointer: Cell::new(false),
            keyboard: Cell::new(false),
            touch: Cell::new(false)
        }
    }

    fn set_caps(&self, caps: u32) {
        self.pointer.set((caps & wl_seat_capability::WL_SEAT_CAPABILITY_POINTER as u32) != 0);
        self.keyboard.set((caps & wl_seat_capability::WL_SEAT_CAPABILITY_KEYBOARD as u32) != 0);
        self.touch.set((caps & wl_seat_capability::WL_SEAT_CAPABILITY_TOUCH as u32) != 0);
    }

    fn set_name(&self, name: &[u8]) {
        *(self.name.borrow_mut()) = name.to_owned();
    }
}

/// The data used by the listener callbacks.
struct SeatListener {
    /// Handler of the "new global object" event
    capabilities_handler: Box<Fn(u32, &SeatData)>,
    /// Handler of the "removed global handler" event
    name_handler: Box<Fn(&[u8], &SeatData)>,
    /// access to the data
    pub data: SeatData
}

impl SeatListener {
    fn default_handlers(data: SeatData) -> SeatListener {
        SeatListener {
            capabilities_handler: Box::new(move |caps, data| {
                data.set_caps(caps);
            }),
            name_handler: Box::new(move |name, data| {
                data.set_name(name);
            }),
            data: data
        }
    }
}

struct InternalSeat {
    _registry: Registry,
    ptr: *mut wl_seat,
    listener: Box<SeatListener>
}

/// A global wayland Seat.
///
/// This structure is a handle to a wayland seat, which can up to a pointer, a keyboard
/// and a touch device.
///
/// Like other global objects, this handle can be cloned.
#[derive(Clone)]
pub struct Seat {
    internal: Rc<InternalSeat>
}

impl Seat {
    pub fn get_pointer(&self) -> Option<Pointer<WSurface>> {
        if self.internal.listener.data.pointer.get() {
            Some(From::from(self.clone()))
        } else {
            None
        }
    }
}


impl Bind<Registry> for Seat {
    fn interface() -> &'static abi::wl_interface {
        abi::WAYLAND_CLIENT_HANDLE.wl_seat_interface
    }

    unsafe fn wrap(ptr: *mut wl_seat, registry: Registry) -> Seat {
        let listener_data = SeatListener::default_handlers(SeatData::new());
        let s = Seat {
            internal: Rc::new(InternalSeat {
                _registry: registry,
                ptr: ptr,
                listener: Box::new(listener_data)
            })
        };
        wl_seat_add_listener(
            s.internal.ptr,
            &SEAT_LISTENER as *const _,
            &*s.internal.listener as *const _ as *mut _
        );
        s
    }
}

impl Drop for InternalSeat {
    fn drop(&mut self) {
        unsafe { wl_seat_destroy(self.ptr) };
    }
}

impl FFI for Seat {
    type Ptr = wl_seat;

    fn ptr(&self) -> *const wl_seat {
        self.internal.ptr as *const wl_seat
    }

    unsafe fn ptr_mut(&self) -> *mut wl_seat {
        self.internal.ptr
    }
}


//
// C-wrappers for the callback closures, to send to wayland
//
extern "C" fn seat_capabilities_handler(data: *mut c_void,
                                        _registry: *mut wl_seat,
                                        capabilities: uint32_t,
                                       ) {
    let listener = unsafe { &*(data as *const SeatListener) };
    (listener.capabilities_handler)(capabilities, &listener.data);
}

extern "C" fn seat_name_handler(data: *mut c_void,
                                _registry: *mut wl_seat,
                                name: *const c_char
                               ) {
    let listener = unsafe { &*(data as *const SeatListener) };
    let name_str = unsafe { CStr::from_ptr(name) };
    (listener.name_handler)(name_str.to_bytes(), &listener.data);
}

static SEAT_LISTENER: wl_seat_listener = wl_seat_listener {
    capabilities: seat_capabilities_handler,
    name: seat_name_handler
};