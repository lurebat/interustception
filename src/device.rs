// Copyright (c) Microsoft Corporation.
// License: MIT OR Apache-2.0

use alloc::format;
use core::fmt::Debug;
use core::sync::atomic::AtomicU32;
use nt_string::nt_unicode_str;
use nt_string::unicode_string::{NtUnicodeStr, NtUnicodeString};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use wdk::{nt_success, paged_code, println};
use wdk_sys::{*};
use wdk_sys::_WDF_REQUEST_SEND_OPTIONS_FLAGS::WDF_REQUEST_SEND_OPTION_SEND_AND_FORGET;
use wdk_sys::_WDF_REQUEST_TYPE::WdfRequestTypeDeviceControlInternal;
use wdk_sys::macros::call_unsafe_wdf_function_binding;
use wdk_sys::ntddk::KeGetCurrentIrql;

use crate::{dbg, DeviceContext, get_pdo_context, GUID_DEVINTERFACE_INTERUSTCEPTION, PdoContext, wdf_object_get_device_context};
use crate::device::KeyboardIoctl::PdoKeyboardAttributes;
use crate::foreign::{ConnectData, GUID_CLASS_KEYBOARD, KeyboardAttributes, KeyboardInputData};
use crate::framework::{Device, DeviceBuilder, QueueBuilder, Result};
use crate::framework::pdo::PdoBuilder;
use crate::framework::utils::ctl_code;

static mut INSTANCES: AtomicU32 = AtomicU32::new(0);



pub(crate) fn device_create(device_init: &mut WDFDEVICE_INIT) -> Result<()> {

    dbg!("device_create");

    let mut builder = DeviceBuilder::new(device_init);
    let mut device = builder
        .as_filter_device()
        .with_device_type(FILE_DEVICE_KEYBOARD)
        .build_with_context::<DeviceContext>()?;

    dbg!("device_create - created device");

    let _default_queue = QueueBuilder::new()
        .default_queue()
        .parallel_dispatch()
        .internal_device_control(Some(internal_ioctl))
        .create(device.handle())?;

    dbg!("device_create - created default queue");

    let pdo_queue = QueueBuilder::new()
        .parallel_dispatch()
        .internal_device_control(Some(pdo_from_ioctl))
        .create(device.handle())?;

    dbg!("device_create - created pdo queue");

    let context = device.context_mut();
    context.raw_pdo_queue = pdo_queue.handle();

    let current = unsafe {
        INSTANCES.fetch_add(1, core::sync::atomic::Ordering::SeqCst)
    } + 1;

    dbg!("device_create - starting to create pdos");

    create_pdo(&mut device, current)?;

    dbg!("device_create - created pdos");

    dbg!(device.save());

    Ok(())

}

const DEVICE_ID: NtUnicodeStr<'static> = nt_unicode_str!("{A65C87F9-BE02-4ed9-92EC-012D416169FA}\\Interustception");

const DEVICE_LOCATION: NtUnicodeStr<'static> = nt_unicode_str!("Interustception");

fn create_pdo(device: &mut Device<DeviceContext>, current: u32) -> Result<()> {

    dbg!("create_pdo");

    let instance_id = NtUnicodeString::try_from(format!("{:02}", current)).unwrap();

    let device_description = NtUnicodeString::try_from(format!("Interustception PDO {:02}", current)).unwrap();

    dbg!("create_pdo - starting to create pdo");

    let mut builder = PdoBuilder::new(device.handle());
    let mut pdo = builder
        .with_class(GUID_CLASS_KEYBOARD)
        .with_device_id(DEVICE_ID)
        .with_instance_id(instance_id)
        .with_device_text(device_description, DEVICE_LOCATION, 0x409)
        .allow_forwarding_request_to_parent()
        .build_with_context::<PdoContext>()?;

    dbg!("create_pdo - created pdo");

    {
        let context = dbg!(pdo.context_mut());
        context.instance = current;
        context.queue = device.context().raw_pdo_queue;
    }


    dbg!("create_pdo - starting to create pdo queue");

    let _pdo_queue = QueueBuilder::new()
        .default_queue()
        .device_control(Some(pdo_to_ioctl))
        .create(pdo.handle())?;

    dbg!("create_pdo - created pdo queue");

    pdo.set_capabilities(
        true,
        true,
        current,
        current
    );

    pdo.create_interface(&GUID_DEVINTERFACE_INTERUSTCEPTION)?;

    dbg!("create_pdo - created interface");
    pdo.attach(device.handle())?;

    dbg!("create_pdo - attached pdo");

    pdo.save();

    Ok(())
}

#[derive(Debug, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
#[repr(u32)]
enum KeyboardIoctl {
    SetPrecedence = ctl_code(FILE_DEVICE_UNKNOWN, 0x801, METHOD_BUFFERED, FILE_ANY_ACCESS),
    GetPrecedence = ctl_code(FILE_DEVICE_UNKNOWN, 0x802, METHOD_BUFFERED, FILE_ANY_ACCESS),
    SetFiler = ctl_code(FILE_DEVICE_UNKNOWN, 0x804, METHOD_BUFFERED, FILE_ANY_ACCESS),
    GetFilter = ctl_code(FILE_DEVICE_UNKNOWN, 0x808, METHOD_BUFFERED, FILE_ANY_ACCESS),
    SetEvent = ctl_code(FILE_DEVICE_UNKNOWN, 0x810, METHOD_BUFFERED, FILE_ANY_ACCESS),
    Write = ctl_code(FILE_DEVICE_UNKNOWN, 0x820, METHOD_BUFFERED, FILE_ANY_ACCESS),
    Read = ctl_code(FILE_DEVICE_UNKNOWN, 0x840, METHOD_BUFFERED, FILE_ANY_ACCESS),
    GetHardwareId = ctl_code(FILE_DEVICE_UNKNOWN, 0x880, METHOD_BUFFERED, FILE_ANY_ACCESS),

    KeyboardConnect = ctl_code(FILE_DEVICE_KEYBOARD, 0x80, METHOD_NEITHER, FILE_ANY_ACCESS),
    KeyboardDisconnect = ctl_code(FILE_DEVICE_KEYBOARD, 0x100, METHOD_NEITHER, FILE_ANY_ACCESS),
    KeyboardQueryAttributes = 720896u32,

    PdoKeyboardAttributes = ctl_code(FILE_DEVICE_KEYBOARD, 0x800, METHOD_BUFFERED, FILE_READ_DATA),
}


#[link_section = "PAGE"]
unsafe extern "C" fn internal_ioctl(queue: WDFQUEUE, request: WDFREQUEST, _output_buffer_length: usize, _input_buffer_length: usize, io_control_code: ULONG) {
    paged_code!();

    println!("WAWA internal_ioctl 1");

    let device = call_unsafe_wdf_function_binding!(
        WdfIoQueueGetDevice,
        queue
    );

    println!("WAWA internal_ioctl 2");

    let device_context_ptr = unsafe { wdf_object_get_device_context(device as WDFOBJECT) };

    println!("WAWA internal_ioctl 3");

    println!("WAWA internal_ioctl 4");

    let mut forward_request = false;

    let mut status = STATUS_SUCCESS;
    let mut completion_context = WDF_NO_CONTEXT;


    match KeyboardIoctl::try_from(io_control_code as u32) {
        Ok(KeyboardIoctl::KeyboardConnect) => {
            println!("WAWA internal_ioctl 5.1");

            // Only allow one connection at a time. (for now)
            if !(unsafe { (*device_context_ptr).upper_connect_data}.class_service.is_null()) {
                println!("WAWA internal_ioctl 5.2");
                status = STATUS_SHARING_VIOLATION;
            } else {
                let mut connect_data = &mut ConnectData::default();
                println!("WAWA internal_ioctl 5.3");

                // Get the input buffer from the request.
                status = call_unsafe_wdf_function_binding!(
                        WdfRequestRetrieveInputBuffer,
                        request,
                        core::mem::size_of::<ConnectData>(),
                        (&mut connect_data) as *mut _ as *mut _,
                        core::ptr::null_mut(),
                    );

                println!("WAWA internal_ioctl 5.4");

                if !nt_success(status) {
                    println!("WAWAWA WdfRequestRetrieveInputBuffer failed {status:#010X}");
                } else {
                    println!("WAWA internal_ioctl 5.5");
                    unsafe {
                        (*device_context_ptr).upper_connect_data = *connect_data;
                    }

                    connect_data.class_device_object = call_unsafe_wdf_function_binding!(
                            WdfDeviceWdmGetDeviceObject,
                            device
                        );
                    connect_data.class_service = service_callback as PVOID;
                }
            }
        }
        Ok(KeyboardIoctl::KeyboardDisconnect) => {
            println!("WAWA internal_ioctl 5.6");
            // Disconnect. This is allowed even if there is no outstanding connect.
            // TODO - do we need to free anything?
            unsafe{ (*device_context_ptr).upper_connect_data = ConnectData::default(); }
        }
        Ok(KeyboardIoctl::KeyboardQueryAttributes) => {
            println!("WAWA internal_ioctl 5.7");
            // Get keyboard attributes
            forward_request = true;
            completion_context = device_context_ptr as PVOID;
        }
        _ => {}
    }

    println!("WAWA internal_ioctl 6");
    if !nt_success(status) {
        println!("WAWA internal_ioctl 6.1");
        unsafe {
            call_unsafe_wdf_function_binding!(
                WdfRequestComplete,
                request,
                status
            );
        }
        return;
    }

    if forward_request {
        println!("WAWA internal_ioctl 7.1");
        let mut output_memory = core::ptr::null_mut() as WDFMEMORY;

        status = unsafe {
            call_unsafe_wdf_function_binding!(
                WdfRequestRetrieveOutputMemory,
                request,
                &mut output_memory
            )
        };

        println!("WAWA internal_ioctl 7.2");

        if !nt_success(status) {
            println!("WAWAWA WdfRequestRetrieveOutputMemory failed {status:#010X}");
            unsafe {
                call_unsafe_wdf_function_binding!(
                    WdfRequestComplete,
                    request,
                    status
                );
            }
            return;
        }


        println!("WAWA internal_ioctl 7.3");
        status = unsafe {
            call_unsafe_wdf_function_binding!(
                WdfIoTargetFormatRequestForInternalIoctl,
                macros::call_unsafe_wdf_function_binding!(
                    WdfDeviceGetIoTarget,
                    device
                ),
                request,
                io_control_code,
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                output_memory,
                core::ptr::null_mut(),
            )
        };

        if !nt_success(status) {
            println!("WAWAWA WdfIoTargetFormatRequestForInternalIoctl failed {status:#010X}");
            unsafe {
                call_unsafe_wdf_function_binding!(
                    WdfRequestComplete,
                    request,
                    status
                );
            }
            return;
        }

        println!("WAWA internal_ioctl 7.4");

        unsafe {
            call_unsafe_wdf_function_binding!(
                WdfRequestSetCompletionRoutine,
                request,
                Some(completion_routine),
                completion_context,
            );
        }
        println!("WAWA internal_ioctl 7.5");

        let ret = unsafe {
            call_unsafe_wdf_function_binding!(
                WdfRequestSend,
                request,
                macros::call_unsafe_wdf_function_binding!(
                    WdfDeviceGetIoTarget,
                    device
                ),
                WDF_NO_SEND_OPTIONS as *mut WDF_REQUEST_SEND_OPTIONS,
            )
        };

        println!("WAWA internal_ioctl 7.6");

        if ret == 0 {
            status = unsafe {
                call_unsafe_wdf_function_binding!(
                    WdfRequestGetStatus,
                    request
                )
            };
            println!("WAWAWA WdfRequestSend failed {status:#010X}");
            unsafe {
                call_unsafe_wdf_function_binding!(
                    WdfRequestComplete,
                    request,
                    status
                );
            }
        }
    } else {
        println!("WAWA internal_ioctl 8.1");
        let options = WDF_REQUEST_SEND_OPTIONS {
            Size: core::mem::size_of::<WDF_REQUEST_SEND_OPTIONS>() as ULONG,
            Flags: WDF_REQUEST_SEND_OPTION_SEND_AND_FORGET as ULONG,
            ..WDF_REQUEST_SEND_OPTIONS::default()
        };

        println!("WAWA internal_ioctl 8.2");

        let ret = unsafe {
            call_unsafe_wdf_function_binding!(
                WdfRequestSend,
                request,
                macros::call_unsafe_wdf_function_binding!(
                    WdfDeviceGetIoTarget,
                    device
                ),
                &options as *const _ as *mut _,
            )
        };

        println!("WAWA internal_ioctl 8.3");

        if ret == 0 {
            status = unsafe {
                call_unsafe_wdf_function_binding!(
                    WdfRequestGetStatus,
                    request
                )
            };
            println!("WAWAWA WdfRequestSend failed {status:#010X}");
            unsafe {
                call_unsafe_wdf_function_binding!(
                    WdfRequestComplete,
                    request,
                    status
                );
            }
        }
    }
}

#[link_section = "PAGE"]
extern "C" fn pdo_from_ioctl(queue: WDFQUEUE, request: WDFREQUEST, output_buffer_length: usize, _input_buffer_length: usize, io_control_code: ULONG) {
    paged_code!();
    println!("WAWAWA pdo_from_ioctl 1");

    let mut status = STATUS_NOT_IMPLEMENTED;
    let mut bytes_transferred: ULONG_PTR = 0;
    if io_control_code == PdoKeyboardAttributes as u32 {
        if output_buffer_length < core::mem::size_of::<KeyboardAttributes>() {
            status = STATUS_BUFFER_TOO_SMALL;
        }
        else {
            let mut output_memory = core::ptr::null_mut() as WDFMEMORY;
            status = unsafe {
                call_unsafe_wdf_function_binding!(
                    WdfRequestRetrieveOutputMemory,
                    request,
                    &mut output_memory
                )
            };

            if !nt_success(status) {
                println!("WAWAWA WdfRequestRetrieveOutputMemory failed {status:#010X}");
            } else {
                let handle = queue as WDFOBJECT;
                let device_context: &mut DeviceContext = unsafe { wdf_object_get_device_context(handle).as_mut().unwrap() }; // TODO: Handle this better.
                status = unsafe {
                    call_unsafe_wdf_function_binding!(
                        WdfMemoryCopyFromBuffer,
                        output_memory,
                        0,
                        (&mut device_context.keyboard_attributes) as *mut _ as *mut _,
                        core::mem::size_of::<KeyboardAttributes>(),
                    )
                };

                if !nt_success(status) {
                    println!("WAWAWA WdfMemoryCopyFromBuffer failed {status:#010X}");
                } else {
                    status = STATUS_SUCCESS;
                    bytes_transferred = core::mem::size_of::<KeyboardAttributes>() as ULONG_PTR;
                }
            }

        }
    }

    unsafe {
        call_unsafe_wdf_function_binding!(
            WdfRequestCompleteWithInformation,
            request,
            status,
            bytes_transferred,
        )
    };
}

#[link_section = "PAGE"]
extern "C" fn pdo_to_ioctl(queue: WDFQUEUE, request: WDFREQUEST, _output_buffer_length: usize, _input_buffer_length: usize, io_control_code: ULONG) {
    paged_code!();
    println!("WAWAWA pdo_to_ioctl 1");

    if io_control_code == PdoKeyboardAttributes as u32 {
        let forward_options = WDF_REQUEST_SEND_OPTIONS {
            Size: core::mem::size_of::<WDF_REQUEST_SEND_OPTIONS>() as ULONG,
            Flags: WDF_REQUEST_SEND_OPTION_SEND_AND_FORGET as ULONG,
            ..WDF_REQUEST_SEND_OPTIONS::default()
        };
        /*
                status = WdfRequestForwardToParentDeviceIoQueue(Request, pdoData->ParentQueue, &forwardOptions);
        if (!NT_SUCCESS(status)) {
            WdfRequestComplete(Request, status);
        }
         */

        let parent = unsafe {
            call_unsafe_wdf_function_binding!(
                WdfIoQueueGetDevice,
                queue
            )
        };

        let pdo_context = unsafe { get_pdo_context(parent as WDFOBJECT) };

        /*        status = WdfRequestForwardToParentDeviceIoQueue(Request, pdoData->ParentQueue, &forwardOptions);
        if (!NT_SUCCESS(status)) {
            WdfRequestComplete(Request, status);
        }
         */

        let status = unsafe {
            call_unsafe_wdf_function_binding!(
                WdfRequestForwardToParentDeviceIoQueue,
                request,
                (*pdo_context).queue,
                &forward_options as *const _ as *mut _,
            )};

        if !nt_success(status) {
            println!("WAWAWA WdfRequestForwardToParentDeviceIoQueue failed {status:#010X}");
            unsafe {
                call_unsafe_wdf_function_binding!(
                    WdfRequestComplete,
                    request,
                    status
                );
            }
        }
    } else {
        unsafe {
            call_unsafe_wdf_function_binding!(
                WdfRequestComplete,
                request,
                STATUS_SUCCESS
            );
        }
    }
}


type ServiceCallback = extern "C" fn(device_object: PDEVICE_OBJECT, input_data_start: *mut KeyboardInputData, input_data_end: *mut KeyboardInputData, input_data_consumed: PULONG);


unsafe extern "C" fn service_callback(device_object: PDEVICE_OBJECT, input_data_start: *mut KeyboardInputData, input_data_end: *mut KeyboardInputData, input_data_consumed: PULONG) {
    println!("WAWAWA service_callback 1");

    let device = call_unsafe_wdf_function_binding!(
        WdfWdmDeviceGetWdfDeviceHandle,
        device_object
    );

    let device_context: &mut DeviceContext =
        unsafe { wdf_object_get_device_context(device as WDFOBJECT).as_mut().unwrap() }; // TODO: Handle this better.

    println!("WAWAWA Service callback called for device");
    let input_data_length = (input_data_end as usize - input_data_start as usize) / core::mem::size_of::<KeyboardInputData>();
    if input_data_length > 0 {
        let input_data_slice = unsafe { core::slice::from_raw_parts(input_data_start, input_data_length) };
        for (i, input_data) in input_data_slice.iter().enumerate() {
            dbg!((i, input_data));
        }
    }


    if !device_context.upper_connect_data.class_service.is_null() {
        let callback: ServiceCallback = unsafe { core::mem::transmute(device_context.upper_connect_data.class_service) };

        callback(device_context.upper_connect_data.class_device_object, input_data_start, input_data_end, input_data_consumed);
    }
}

#[link_section = "PAGE"]
unsafe extern "C" fn completion_routine(request: WDFREQUEST, _handle: WDFIOTARGET, params: *mut WDF_REQUEST_COMPLETION_PARAMS, context: WDFCONTEXT) {
    paged_code!();
    println!("WAWAWA completion_routine 1");

    let params_ioctl = unsafe { &mut (*params).Parameters.Ioctl };
    let params_status = unsafe { (*params).IoStatus.__bindgen_anon_1.Status };

    let buffer = params_ioctl.Output.Buffer;
    let mut status = params_status;

    if nt_success(status) && unsafe { (*params).Type } == WdfRequestTypeDeviceControlInternal && params_ioctl.IoControlCode == KeyboardIoctl::KeyboardQueryAttributes as u32 && params_ioctl.Output.Length >= core::mem::size_of::<KeyboardAttributes>() {
        let device_context: &mut DeviceContext = unsafe { core::mem::transmute(context) };
        status = call_unsafe_wdf_function_binding!(
            WdfMemoryCopyToBuffer,
            buffer,
            params_ioctl.Output.Offset,
            (&mut device_context.keyboard_attributes) as *mut _ as *mut _,
            core::mem::size_of::<KeyboardAttributes>(),
        );
        dbg!(device_context.keyboard_attributes);
    }

    call_unsafe_wdf_function_binding!(
        WdfRequestComplete,
        request,
        status
    );
}

