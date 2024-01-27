// Copyright (c) Microsoft Corporation.
// License: MIT OR Apache-2.0

use core::mem;
use core::mem::transmute;
use wdk::{nt_success, paged_code};
use wdk_sys::{macros, ntddk::KeGetCurrentIrql, NTSTATUS, WDFDRIVER, *};
use wdk_sys::_WDF_IO_QUEUE_DISPATCH_TYPE::WdfIoQueueDispatchParallel;
use wdk_sys::_WDF_REQUEST_SEND_OPTIONS_FLAGS::WDF_REQUEST_SEND_OPTION_SEND_AND_FORGET;
use wdk_sys::_WDF_TRI_STATE::WdfUseDefault;
use wdk_sys::macros::call_unsafe_wdf_function_binding;
use crate::wdf_object_context::wdf_get_context_type_info;
use crate::{dbg, driver_entry, kernel_callback, WDF_DEVICE_CONTEXT_TYPE_INFO, wdf_object_get_device_context};
use crate::foreign::{ConnectData, KeyboardInputData};
use crate::utils::ctl_code;

extern crate alloc;

driver_entry!((driver, registry_path) {
    dbg!();

    let mut driver_config = WDF_DRIVER_CONFIG {
        Size: core::mem::size_of::<WDF_DRIVER_CONFIG>() as ULONG,
        EvtDriverDeviceAdd: Some(device_add),
        ..WDF_DRIVER_CONFIG::default()
    };
    dbg!();
    let mut driver_handle_output = WDF_NO_HANDLE as WDFDRIVER;

    dbg!();
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
    dbg!();

    if !nt_success(nt_status) {
        dbg!();
        return nt_status;
    }

    dbg!();

    nt_status
});

kernel_callback!(fn device_add(_driver: WDFDRIVER, mut device_init: PWDFDEVICE_INIT) -> NTSTATUS {
    paged_code!();

    dbg!();

    dbg!();

    unsafe {
        call_unsafe_wdf_function_binding!(
        WdfFdoInitSetFilter,
        device_init);
    }
    dbg!();

    unsafe {
        call_unsafe_wdf_function_binding!(
        WdfDeviceInitSetDeviceType,
        device_init,
        FILE_DEVICE_KEYBOARD);
    }
    dbg!();

    let mut attributes = WDF_OBJECT_ATTRIBUTES {
        Size: core::mem::size_of::<WDF_OBJECT_ATTRIBUTES>() as ULONG,
        ExecutionLevel: _WDF_EXECUTION_LEVEL::WdfExecutionLevelInheritFromParent,
        SynchronizationScope: _WDF_SYNCHRONIZATION_SCOPE::WdfSynchronizationScopeInheritFromParent,
        ..WDF_OBJECT_ATTRIBUTES::default()
    };
    dbg!();

    attributes.ContextTypeInfo = wdf_get_context_type_info!(DeviceContext);

    dbg!();

    let mut device_handle = WDF_NO_HANDLE as WDFDEVICE;

    dbg!();

    let nt_status = unsafe {
        call_unsafe_wdf_function_binding!(
            WdfDeviceCreate,
            &mut device_init,
            &mut attributes,
            &mut device_handle,
        )
    };

    dbg!(nt_status);

    if !nt_success(nt_status) {
        dbg!("WdfDeviceCreate failed with status: {:x}", nt_status);
        return nt_status;
    }

    let mut queue_config = WDF_IO_QUEUE_CONFIG {
        Size: core::mem::size_of::<WDF_IO_QUEUE_CONFIG>() as ULONG,
        PowerManaged: WdfUseDefault,
        DefaultQueue: true as BOOLEAN,
        DispatchType: WdfIoQueueDispatchParallel,
        ..WDF_IO_QUEUE_CONFIG::default()
    };
    queue_config.Settings.Parallel.NumberOfPresentedRequests = ULONG::MAX;
    queue_config.EvtIoInternalDeviceControl = Some(queue_evt_io_internal_device_control);


    dbg!();

    let nt_status = unsafe {
        call_unsafe_wdf_function_binding!(
            WdfIoQueueCreate,
            device_handle,
            &mut queue_config,
            WDF_NO_OBJECT_ATTRIBUTES,
            WDF_NO_HANDLE as *mut WDFQUEUE,
        )
    };

    dbg!(nt_status);

    nt_status
});

const fn keyboard_trap_ctl_code(function: u32) -> u32 {
    ctl_code(FILE_DEVICE_KEYBOARD, function, METHOD_NEITHER, FILE_ANY_ACCESS)
}

const IOCTL_KEYBOARD_TRAP_CONNECT: u32 = keyboard_trap_ctl_code(0x0080);
const IOCTL_KEYBOARD_TRAP_DISCONNECT: u32 = keyboard_trap_ctl_code(0x0100);

kernel_callback!(
fn queue_evt_io_internal_device_control(queue: WDFQUEUE, request: WDFREQUEST, _output_buffer_length: usize, _input_buffer_length: usize, io_control_code: ULONG) -> () {
    dbg!("queue_evt_io_internal_device_control");
    let device = unsafe { call_unsafe_wdf_function_binding!(WdfIoQueueGetDevice, queue) };
    dbg!();
    let device_context = unsafe { wdf_object_get_device_context(device as WDFOBJECT) };
    dbg!();
    let connect_data = unsafe { (*device_context).upper_connect_data.class_service };

    let status = match io_control_code {
        IOCTL_KEYBOARD_TRAP_CONNECT if !connect_data.is_null() => {
            dbg!();
            STATUS_SHARING_VIOLATION
        }
        IOCTL_KEYBOARD_TRAP_CONNECT => {
            dbg!();
            let mut connect_data = &mut ConnectData::default();
            let mut buffer_length = 0usize;

            dbg!();
            let status = unsafe {
                call_unsafe_wdf_function_binding!(
                    WdfRequestRetrieveInputBuffer,
                    request,
                    core::mem::size_of::<ConnectData>(),
                    &mut connect_data as *mut _ as *mut _,
                    &mut buffer_length,
                )
            };
            dbg!();

            if !nt_success(status) {
                dbg!(status);
                status
            } else {
                dbg!();
                unsafe {
                    dbg!();
                    (*device_context).upper_connect_data = *connect_data;

                    connect_data.class_device_object = unsafe { call_unsafe_wdf_function_binding!(WdfDeviceWdmGetDeviceObject, device) };
                    connect_data.class_service = keyboard_trap_service_callback as *mut _;

                }
                dbg!();
                STATUS_SUCCESS
            }
        },
        IOCTL_KEYBOARD_TRAP_DISCONNECT => dbg!(STATUS_NOT_IMPLEMENTED),
        _ => dbg!(STATUS_SUCCESS),
    };

    dbg!(status);
    if !nt_success(status) {
        dbg!();
        unsafe {
            call_unsafe_wdf_function_binding!(
                WdfRequestComplete,
                request,
                status,
            );
        }
        return;
    }

    dbg!();
    let mut options = WDF_REQUEST_SEND_OPTIONS {
        Size: core::mem::size_of::<WDF_REQUEST_SEND_OPTIONS>() as ULONG,
        Flags: WDF_REQUEST_SEND_OPTION_SEND_AND_FORGET as ULONG,
        ..WDF_REQUEST_SEND_OPTIONS::default()
    };

    dbg!();

    let io_target = unsafe { call_unsafe_wdf_function_binding!(WdfDeviceGetIoTarget, device) };

    dbg!();

    let result = call_unsafe_wdf_function_binding!(
        WdfRequestSend,
        request,
        io_target,
        &mut options,
    );

    dbg!(result);

    if result == 0 {
        let status = unsafe { call_unsafe_wdf_function_binding!(WdfRequestGetStatus, request) };
        dbg!(status);
        unsafe {
            call_unsafe_wdf_function_binding!(
                WdfRequestComplete,
                request,
                status,
            );
        }
    }

});

type ServiceCallback = extern "C" fn(device_object: PDEVICE_OBJECT, input_data_start: *const KeyboardInputData, input_data_end: *const KeyboardInputData, input_data_consumed: PULONG);


unsafe extern "C" fn keyboard_trap_service_callback(device_object: PDEVICE_OBJECT, input_data_start: *const KeyboardInputData, input_data_end: *const KeyboardInputData, input_data_consumed: PULONG) -> () {
    dbg!("keyboard_trap_service_callback");
    let device = unsafe { call_unsafe_wdf_function_binding!(WdfWdmDeviceGetWdfDeviceHandle, device_object) };
    dbg!();
    let device_context = unsafe { wdf_object_get_device_context(device as WDFOBJECT) };
    dbg!();

    /*	for(PKEYBOARD_INPUT_DATA current = inputDataStart; current != inputDataEnd; current++) {
		DebugPrint(("[KeyboardTrap] Code: %d\n", current->MakeCode));
	}*/

    for current in unsafe {core::slice::from_raw_parts(input_data_start, (input_data_end as usize - input_data_start as usize) / mem::size_of::<KeyboardInputData>()) } {
        dbg!(current);
    }

    let data = unsafe { (*device_context).upper_connect_data };

    if !data.class_service.is_null() {
        dbg!(data.class_service);
        let callback: ServiceCallback = unsafe {transmute(data.class_service as *const ()) };
        dbg!(callback);

        (callback)(data.class_device_object, input_data_start, input_data_end, input_data_consumed);
    }
}

