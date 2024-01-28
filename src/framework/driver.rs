use wdk_sys::{DRIVER_OBJECT, PCUNICODE_STRING, PFN_WDF_DRIVER_DEVICE_ADD, PUNICODE_STRING, WDF_DRIVER_CONFIG, WDF_NO_HANDLE, WDF_NO_OBJECT_ATTRIBUTES, WDFDRIVER};
use wdk_sys::macros::call_unsafe_wdf_function_binding;
use crate::{dbg, init_object};
use crate::framework::error::{Result, NtStatusError, ErrorCode};

#[derive(Debug,)]
pub struct DriverInit<'a> {
    pub driver: &'a mut DRIVER_OBJECT,
    pub config: WDF_DRIVER_CONFIG,
}
impl<'a> DriverInit<'a> {
    pub fn new(driver: &'a mut DRIVER_OBJECT) -> Self {
        Self {
            driver,
            config: init_object!(WDF_DRIVER_CONFIG),
        }
    }

    pub fn device_add(&mut self, device_add: PFN_WDF_DRIVER_DEVICE_ADD) -> &mut Self {
        self.config.EvtDriverDeviceAdd = device_add;
        self
    }

    pub fn create(&mut self, registry_path: wdk_sys::PCUNICODE_STRING) -> Result<WDFDRIVER> {
        let mut driver_handle_output = WDF_NO_HANDLE as WDFDRIVER;

        dbg!("WdfDriverCreate");

        unsafe {
            call_unsafe_wdf_function_binding!(
            WdfDriverCreate,
            self.driver,
            registry_path,
            WDF_NO_OBJECT_ATTRIBUTES,
            &mut self.config,
            &mut driver_handle_output)
        }.check_status(ErrorCode::DriverEntryFailed).map(|_| driver_handle_output)?;

        dbg!("WdfDriverCreate succeeded");

        Ok(driver_handle_output)
    }
}


