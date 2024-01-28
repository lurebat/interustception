use nt_string::unicode_string::NtUnicodeStr;
use wdk_sys::{GUID, PWDFDEVICE_INIT, WDFDEVICE, WDFDEVICE_INIT};
use wdk_sys::macros::call_unsafe_wdf_function_binding;
use crate::framework::{ErrorCode, NtStatusError};

pub(crate) struct PdoBuilder {
    init: PWDFDEVICE_INIT,
    class: Option<GUID>,
    device_id: Option<NtUnicodeStr<'static>>,
    instance_id: Option<NtUnicodeStr<'static>>,
    device_text: Option<NtUnicodeStr<'static>>,
    locale: Option<u32>,
}

impl PdoBuilder {
    pub(crate) fn new(device: WDFDEVICE) -> Self {
        let init = call_unsafe_wdf_function_binding!(
            WdfPdoInitAllocate,
            device,
        );

        Self {
            init,
            class: None,
            device_id: None,
            instance_id: None,
            device_text: None,
            locale: None,
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

    pub(crate) fn with_instance_id(&mut self, instance_id: NtUnicodeStr<'static>) -> &mut Self {
        self.instance_id = Some(instance_id);
        self
    }

    pub(crate) fn with_device_text(&mut self, device_text: NtUnicodeStr<'static>) -> &mut Self {
        self.device_text = Some(device_text);
        self
    }

    pub(crate) fn with_locale(&mut self, locale: u32) -> &mut Self {
        self.locale = Some(locale);
        self
    }

    pub(crate) fn create(self) -> Result<WDFDEVICE> {
        if let Some(class) = self.class {
            unsafe {
                call_unsafe_wdf_function_binding!(
            WdfPdoInitAssignRawDevice,
            self.init,
            &class,
        )
            }.check_status(ErrorCode::PdoInitAssignRawDeviceFailed)?;
        }

        if let Some(device_id) = self.device_id {
            unsafe {
                call_unsafe_wdf_function_binding!(
            WdfPdoInitAssignDeviceID,
            self.init,
            device_id.as_pwstr(),
        )
            }.check_status(ErrorCode::PdoInitAssignDeviceIdFailed)?;
        }

        if let Some(instance_id) = self.instance_id {
            unsafe {
                call_unsafe_wdf_function_binding!(
            WdfPdoInitAssignInstanceID,
            self.init,
            instance_id.as_pwstr(),
        )
            }.check_status(ErrorCode::PdoInitAssignInstanceIdFailed)?;
        }

        if let Some(device_text) = self.device_text {
            unsafe {
                call_unsafe_wdf_function_binding!(
            WdfPdoInitAddDeviceText,
            self.device_init,
            device_text.as_pwstr(),
            self.locale.unwrap_or(0),
        )
            }.check_status(ErrorCode::PdoInitAddDeviceTextFailed)?;
        }
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
