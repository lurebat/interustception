use wdk_sys::{WDFDRIVER, *};
use wdk_sys::ntddk::KeGetCurrentIrql;
use crate::{dbg, driver_entry, kernel_callback};
use crate::framework::*;

extern crate alloc;

driver_entry!(fn (driver, registry_path) {
    dbg!(DriverInit::new(driver)
        .device_add(Some(device_add))
        .create(registry_path)
        .to_status())
    });

kernel_callback!(
    fn device_add(_driver: WDFDRIVER, device_init: PWDFDEVICE_INIT) -> NTSTATUS {
        dbg!(crate::device::device_create(
            unsafe { device_init.as_mut() }.expect("device_init is null"),
        ).to_status())
    }
);
