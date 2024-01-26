// Copyright (c) Microsoft Corporation.
// License: MIT OR Apache-2.0

use wdk::{nt_success, paged_code, println};
use wdk_sys::{macros, ntddk::KeGetCurrentIrql, NTSTATUS, WDFDRIVER, *};

use crate::{device};

extern crate alloc;

/// DriverEntry initializes the driver and is the first routine called by the
/// system after the driver is loaded. DriverEntry specifies the other entry
/// points in the function driver, such as EvtDevice and DriverUnload.
///
/// # Arguments
///
/// * `driver` - represents the instance of the function driver that is loaded
///   into memory. DriverEntry must initialize members of DriverObject before it
///   returns to the caller. DriverObject is allocated by the system before the
///   driver is loaded, and it is released by the system after the system
///   unloads the function driver from memory.
/// * `registry_path` - represents the driver specific path in the Registry. The
///   function driver can use the path to store driver related data between
///   reboots. The path does not store hardware instance specific data.
///
/// # Return value:
///
/// * `STATUS_SUCCESS` - if successful,
/// * `STATUS_UNSUCCESSFUL` - otherwise.
#[link_section = "INIT"]
#[export_name = "DriverEntry"] // WDF expects a symbol with the name DriverEntry
extern "system" fn driver_entry(
    driver: &mut DRIVER_OBJECT,
    registry_path: PCUNICODE_STRING,
) -> NTSTATUS {
    println!("WAWA DriverEntry 1 3");

    let mut driver_config = WDF_DRIVER_CONFIG {
        Size: core::mem::size_of::<WDF_DRIVER_CONFIG>() as ULONG,
        EvtDriverDeviceAdd: Some(device_add),
        ..WDF_DRIVER_CONFIG::default()
    };
    println!("WAWA DriverEntry 2");
    let mut driver_handle_output = WDF_NO_HANDLE as WDFDRIVER;

    println!("WAWA DriverEntry 3");
    let nt_status = unsafe {
        macros::call_unsafe_wdf_function_binding!(
            WdfDriverCreate,
            driver as PDRIVER_OBJECT,
            registry_path,
            WDF_NO_OBJECT_ATTRIBUTES,
            &mut driver_config,
            &mut driver_handle_output,
        )
    };
    println!("WAWA DriverEntry 4");

    if !nt_success(nt_status) {
        println!("WAWA DriverEntry 5");
        println!("WAWA Error: WdfDriverCreate failed {nt_status:#010X}");
        return nt_status;
    }

    println!("WAWA DriverEntry 6");

    nt_status
}

/// EvtDeviceAdd is called by the framework in response to AddDevice
/// call from the PnP manager. We create and initialize a device object to
/// represent a new instance of the device.
///
/// # Arguments:
///
/// * `_driver` - Handle to a framework driver object created in DriverEntry
/// * `device_init` - Pointer to a framework-allocated WDFDEVICE_INIT structure.
///
/// # Return value:
///
///   * `NTSTATUS`
#[link_section = "PAGE"]
extern "C" fn device_add(_driver: WDFDRIVER, device_init: PWDFDEVICE_INIT) -> NTSTATUS {
    paged_code!();

    println!("WAWA Enter  EchoEvtDeviceAdd");

    let device_init =
        // SAFETY: WDF should always be providing a pointer that is properly aligned, dereferencable per https://doc.rust-lang.org/std/ptr/index.html#safety, and initialized. For the lifetime of the resulting reference, the pointed-to memory is never accessed through any other pointer.
        unsafe {
            device_init
                .as_mut()
                .expect("WDF should never provide a null pointer for device_init")
        };

    unsafe { return device::echo_device_create(device_init) }
}
