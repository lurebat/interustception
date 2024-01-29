use nt_string::unicode_string::{NtUnicodeStr, NtUnicodeString};
use wdk_sys::{GUID, PWDFDEVICE_INIT, UNICODE_STRING, WDF_DEVICE_PNP_CAPABILITIES, WDF_OBJECT_ATTRIBUTES, WDFDEVICE, WDFDEVICE__, WDFDEVICE_INIT};
use wdk_sys::_WDF_EXECUTION_LEVEL::WdfExecutionLevelInheritFromParent;
use wdk_sys::_WDF_SYNCHRONIZATION_SCOPE::WdfSynchronizationScopeInheritFromParent;
use wdk_sys::_WDF_TRI_STATE::WdfUseDefault;
use wdk_sys::macros::call_unsafe_wdf_function_binding;
use crate::framework::{Context, Device, ErrorCode, NtStatusError, Result};
use crate::{dbg, init_object};

pub(crate) struct PdoBuilder {
    init: PWDFDEVICE_INIT,
    class: Option<GUID>,
    device_id: Option<NtUnicodeStr<'static>>,
    instance_id: Option<NtUnicodeString>,
    device_text: Option<(NtUnicodeString, NtUnicodeStr<'static>, u32)>,
    locale: Option<u32>,
    allow_forwarding_request_to_parent: bool,
}

impl PdoBuilder {
    pub(crate) fn new(device: WDFDEVICE) -> Self {
        let init = unsafe {call_unsafe_wdf_function_binding!(
            WdfPdoInitAllocate,
            device,
        )};

        Self {
            init,
            class: None,
            device_id: None,
            instance_id: None,
            device_text: None,
            locale: None,
            allow_forwarding_request_to_parent: false,
        }
    }

    pub(crate) fn with_class(&mut self, class: GUID) -> &mut Self {
        self.class = Some(class);
        self
    }

    pub(crate) fn with_device_id(&mut self, device_id: NtUnicodeStr<'static>) -> &mut Self {
        self.device_id = Some(device_id);
        self
    }

    pub(crate) fn with_instance_id(&mut self, instance_id: NtUnicodeString) -> &mut Self {
        self.instance_id = Some(instance_id);
        self
    }

    pub(crate) fn with_device_text(&mut self, device_description: NtUnicodeString, device_location: NtUnicodeStr<'static>, locale: u32) -> &mut Self {
        self.device_text = Some((device_description, device_location, locale));
        self
    }

    pub(crate) fn allow_forwarding_request_to_parent(&mut self) -> &mut Self {
        self.allow_forwarding_request_to_parent = true;
        self
    }

    pub fn build_with_context<T: Context>(&mut self) -> Result<PdoDevice<T>> {
        dbg!(self.handle_class()?);

        dbg!(self.handle_device_id()?);

        dbg!(self.handle_instance_id()?);

        dbg!(self.handle_device_text()?);

        if self.allow_forwarding_request_to_parent {
            dbg!(unsafe {
                call_unsafe_wdf_function_binding!(
                WdfPdoInitAllowForwardingRequestToParent,
                self.init,
            )
            });
        }

        let mut attrs = init_object!(WDF_OBJECT_ATTRIBUTES);
        attrs.ExecutionLevel = WdfExecutionLevelInheritFromParent;
        attrs.SynchronizationScope = WdfSynchronizationScopeInheritFromParent;

        let mut device_ptr = core::ptr::null_mut();
        let device = unsafe {
            dbg!(call_unsafe_wdf_function_binding!(
            WdfDeviceCreate,
            &mut (self.init as *mut WDFDEVICE_INIT) as *mut *mut WDFDEVICE_INIT,
            (&mut attrs) as *mut WDF_OBJECT_ATTRIBUTES,
            &mut device_ptr,
        )) }.check_status(ErrorCode::DeviceCreationFailed).map(|_| {
            Device::<T>::new(unsafe { device_ptr.as_mut().expect("Device is null")})
        })?;

        self.init = core::ptr::null_mut();

        Ok(PdoDevice{device})
    }
    fn handle_device_text(&mut self) -> Result<()> {
        if let Some((device_description, device_location, locale)) = &self.device_text {
            unsafe {
                call_unsafe_wdf_function_binding!(
            WdfPdoInitAddDeviceText,
            self.init,
            device_description.as_ptr() as *const UNICODE_STRING,
            device_location.as_ptr() as *const UNICODE_STRING,
            *locale,
        )
            }.check_status(ErrorCode::PdoInitAddDeviceTextFailed)?;

            unsafe {
                call_unsafe_wdf_function_binding!(
                WdfPdoInitSetDefaultLocale,
                self.init,
                *locale,
                )
            };
        }
        Ok(())
    }

    fn handle_instance_id(&mut self) -> Result<()> {
        if let Some(instance_id) = &self.instance_id {
            unsafe {
                call_unsafe_wdf_function_binding!(
            WdfPdoInitAssignInstanceID,
            self.init,
            instance_id.as_ptr() as *const UNICODE_STRING,
        )
            }.check_status(ErrorCode::PdoInitAssignInstanceIdFailed)?;
        }
        Ok(())
    }

    fn handle_device_id(&mut self) -> Result<()> {
        if let Some(device_id) = self.device_id {
            unsafe {
                call_unsafe_wdf_function_binding!(
            WdfPdoInitAssignDeviceID,
            self.init,
            device_id.as_ptr() as *const UNICODE_STRING,
        )
            }.check_status(ErrorCode::PdoInitAssignDeviceIdFailed)?;
        }
        Ok(())
    }

    fn handle_class(&mut self) -> Result<()> {
        if let Some(class) = self.class {
            unsafe {
                call_unsafe_wdf_function_binding!(
            WdfPdoInitAssignRawDevice,
            self.init,
            &class,
        )
            }.check_status(ErrorCode::PdoInitAssignRawDeviceFailed)?;
        }
        Ok(())
    }
}

impl Drop for PdoBuilder {
    fn drop(&mut self) {
        if self.init.is_null() {
            return;
        }

        unsafe {
            call_unsafe_wdf_function_binding!(
                WdfDeviceInitFree,
                self.init as *mut WDFDEVICE_INIT
            );
        }
    }
}


#[derive(Debug)]
pub struct PdoDevice<'a, T: Context> {
    pub device: Device<'a, T>,
}

impl<'a, T: Context> PdoDevice<'a, T> {
    pub fn new(device: Device<'a, T>) -> Self {
        Self {
            device,
        }
    }

    pub fn handle(&mut self) -> WDFDEVICE {
        self.device.handle()
    }

    pub fn save(self) -> WDFDEVICE {
        self.device.save()
    }

    pub fn context(&self) -> &T {
        self.device.context()
    }

    pub fn context_mut(&mut self) -> &mut T {
        self.device.context_mut()
    }

    pub fn set_capabilities(&mut self,
                            removable: bool,
                            surprise_removal_ok: bool,
                            address: u32,
                            ui_number: u32) {
        let mut capabilities = init_object!(WDF_DEVICE_PNP_CAPABILITIES);
        capabilities.LockSupported = WdfUseDefault;
        capabilities.EjectSupported = WdfUseDefault;
        capabilities.Removable = removable as i32;
        capabilities.DockDevice = WdfUseDefault;
        capabilities.UniqueID = WdfUseDefault;
        capabilities.SilentInstall = WdfUseDefault;
        capabilities.SurpriseRemovalOK = surprise_removal_ok as i32;
        capabilities.HardwareDisabled = WdfUseDefault;
        capabilities.NoDisplayInUI = WdfUseDefault;
        capabilities.Address = address;
        capabilities.UINumber = ui_number;

        unsafe {
            call_unsafe_wdf_function_binding!(
            WdfDeviceSetPnpCapabilities,
            self.handle(),
            &mut capabilities,
        )
        };
    }

    pub fn create_interface(&mut self, interface: &GUID) -> Result<()> {
        unsafe {
            call_unsafe_wdf_function_binding!(
            WdfDeviceCreateDeviceInterface,
            self.handle(),
            interface,
            core::ptr::null_mut(),
        )
        }.check_status(ErrorCode::DeviceCreateDeviceInterfaceFailed)
    }

    pub fn attach(&mut self, parent: WDFDEVICE) -> Result<()> {
        unsafe {
            call_unsafe_wdf_function_binding!(
            WdfDeviceCreateDeviceInterface,
            self.handle(),
            &GUID::default(),
            core::ptr::null_mut(),
        )
        }.check_status(ErrorCode::DeviceCreateDeviceInterfaceFailed)?;

        unsafe {
            call_unsafe_wdf_function_binding!(
            WdfFdoAddStaticChild,
            parent,
            self.handle(),
        )
        }.check_status(ErrorCode::FdoAddStaticChildFailed)
    }
}
