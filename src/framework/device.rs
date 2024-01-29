use core::mem;
use wdk_sys::{WDF_OBJECT_ATTRIBUTES, WDFDEVICE, WDFDEVICE__, WDFDEVICE_INIT, WDFOBJECT};
use wdk_sys::_WDF_EXECUTION_LEVEL::WdfExecutionLevelInheritFromParent;
use wdk_sys::_WDF_SYNCHRONIZATION_SCOPE::WdfSynchronizationScopeInheritFromParent;
use wdk_sys::macros::call_unsafe_wdf_function_binding;
use crate::framework::{Context, ErrorCode, NtStatusError, Result};
use crate::{dbg, init_object};

#[derive(Debug)]
pub struct DeviceBuilder<'a> {
    device_init: &'a mut WDFDEVICE_INIT,
    attrs: WDF_OBJECT_ATTRIBUTES,
}

impl<'a> DeviceBuilder<'a> {
    pub fn new(device_init: &'a mut WDFDEVICE_INIT) -> Self {
        let mut attrs = init_object!(WDF_OBJECT_ATTRIBUTES);
        attrs.ExecutionLevel = WdfExecutionLevelInheritFromParent;
        attrs.SynchronizationScope = WdfSynchronizationScopeInheritFromParent;

        Self {
            device_init: device_init,
            attrs,
        }
    }

    pub fn as_filter_device(&mut self) -> &mut Self {
        unsafe {
            call_unsafe_wdf_function_binding!(
            WdfFdoInitSetFilter,
            self.device_init
            )
        }

        self
    }

    pub fn with_device_type(&mut self, device_type: u32) -> &mut Self {
        unsafe {
            call_unsafe_wdf_function_binding!(
            WdfDeviceInitSetDeviceType,
            self.device_init,
            device_type
            );
        }

        self
    }

    pub fn build_with_context<T: Context>(&mut self) -> Result<Device<T>> {
        self.attrs.ContextTypeInfo = T::get_context_type_info();

        let mut device = core::ptr::null_mut();
        unsafe {
            call_unsafe_wdf_function_binding!(
                WdfDeviceCreate,
                &mut (self.device_init as *mut WDFDEVICE_INIT) as *mut *mut WDFDEVICE_INIT,
                (&mut self.attrs) as *mut WDF_OBJECT_ATTRIBUTES,
                &mut device,
            )
        }.check_status(ErrorCode::DeviceCreationFailed).map(|_| {
            Device::<T>::new(unsafe {device.as_mut().expect("Device is null")})
        })
    }
}


#[derive(Debug)]
pub struct Device<'a, T: Context> {
    pub device: &'a mut WDFDEVICE__,
    // context is a phantom field to ensure that the type parameter T is used
    // in the struct
    context: core::marker::PhantomData<T>,
}

impl<'a, T: Context> Device<'a, T> {
    pub fn new(device: &'a mut WDFDEVICE__) -> Self {
        Self {
            device,
            context: core::marker::PhantomData,
        }
    }

    pub fn context(&self) -> &T {
        unsafe {
            T::get_context(self.device as *const _  as WDFOBJECT)
                .as_ref()
        }.expect("Context is null")
    }

    pub fn context_mut(&mut self) -> &mut T {
        unsafe {
            T::get_context(dbg!(self.device as *mut _ as WDFOBJECT))
                .as_mut()
        }.expect("Context is null")
    }

    pub fn handle(&mut self) -> WDFDEVICE {
        self.device as WDFDEVICE
    }

    pub fn save(self) -> WDFDEVICE {
        let device = self.device as WDFDEVICE;

        mem::forget(self);

        device
    }
}

impl<'a, T: Context> Drop for Device<'a, T> {
    fn drop(&mut self) {
        dbg!(unsafe {
            call_unsafe_wdf_function_binding!(
                WdfObjectDelete,
                self.device as *mut _ as WDFOBJECT
            )
        })
    }
}
