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

use crate::{dbg, DeviceContext, get_pdo_context, GUID_DEVINTERFACE_INTERUSTCEPTION, kernel_callback, PdoContext, wdf_object_get_device_context};
use crate::device::KeyboardIoctl::PdoKeyboardAttributes;
use crate::foreign::{ConnectData, GUID_CLASS_KEYBOARD, KeyboardAttributes, KeyboardInputData};
use crate::framework::{Device, DeviceBuilder, ErrorCode, NtStatusError, Queue, QueueBuilder, Result, KeyboardConnectRequest, Request};
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
        .internal_device_control(Some(main_device_default_queue_internal_ioctl))
        .create(device.handle())?;

    dbg!("device_create - created default queue");

    let pdo_queue = QueueBuilder::new()
        .parallel_dispatch()
        .device_control(Some(pdo_from_ioctl))
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

    Ok(())
}

const DEVICE_ID: NtUnicodeStr<'static> = nt_unicode_str!("{A65C87F9-BE02-4ed9-92EC-012D416169FA}\\Interustception");

const DEVICE_LOCATION: NtUnicodeStr<'static> = nt_unicode_str!("Interustception");

fn create_pdo(device: &mut Device<DeviceContext>, current: u32) -> Result<()> {
    dbg!("create_pdo");

    let instance_id = NtUnicodeString::try_from(format!("{current:02}")).unwrap();

    let device_description = NtUnicodeString::try_from(format!("Interustception PDO {current:02}")).unwrap();

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
        current,
    );

    pdo.create_interface(&GUID_DEVINTERFACE_INTERUSTCEPTION)?;

    dbg!("create_pdo - created interface");
    pdo.attach(device.handle())?;

    dbg!("create_pdo - attached pdo");

    dbg!(pdo.save());

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


kernel_callback!(
    fn main_device_default_queue_internal_ioctl(queue: WDFQUEUE, request: WDFREQUEST, _output_buffer_length: usize, _input_buffer_length: usize, io_control_code: ULONG) -> ()
    {
        internal_ioctl(queue, request, io_control_code)

    }
);


fn internal_ioctl(queue: WDFQUEUE, request: WDFREQUEST, io_control_code: ULONG) {
    dbg!("internal_ioctl");

    let queue = Queue::new(queue);
    let mut device = queue.get_device::<DeviceContext>();
    dbg!("internal_ioctl - got device");

    let res = match KeyboardIoctl::try_from(io_control_code) {
        Ok(KeyboardIoctl::KeyboardConnect) =>
            on_keyboard_connect(request, &mut device).map(|_| false),
        Ok(KeyboardIoctl::KeyboardDisconnect) => {
            dbg!("Keyboard disconnect");
            device.context_mut().upper_connect_data = ConnectData::default();
            Ok(false)
        }
        Ok(KeyboardIoctl::KeyboardQueryAttributes) => {
            dbg!("Keyboard query attributes");
            Ok(true)
        }
        _ => Ok(false),
    };

    let mut request = Request::new(unsafe { request.as_mut().expect("Request is null") });

    let forward_request = match res {
        Ok(forward_request) => forward_request,
        Err(e) => {
            request.complete(e.nt_status());
            return;
        }
    };

    if !forward_request {
        if let Err(e) = request.send(device.io_target(), WDF_REQUEST_SEND_OPTION_SEND_AND_FORGET as u32) {
            dbg!(&e, request.complete(e.nt_status()));
        }
        return;
    }

    let output_memory = match request.output_memory() {
        Ok(output_memory) => output_memory,
        Err(e) => {
            request.complete(e.nt_status());
            return;
        }
    };

    if let Err(e) = request.format_for_internal_ioctl(device.io_target(), io_control_code, output_memory) {
        dbg!(&e, request.complete(e.nt_status()));
        return;
    }

    request.set_completion_callback(Some(completion_routine), device.context_mut() as *mut _ as PVOID);

    if let Err(e) = request.send(device.io_target(), WDF_REQUEST_SEND_OPTION_SEND_AND_FORGET as u32) {
        dbg!(&e, request.complete(e.nt_status()));
    }
}

fn on_keyboard_connect(request: WDFREQUEST, device: &mut Device<DeviceContext>) -> Result<()> {
    dbg!("Keyboard connect");

    let mut request = KeyboardConnectRequest::new(unsafe { request.as_mut().expect("Request is null") });

    // Only allow one connection at a time. (for now)
    if !(device.context_mut().upper_connect_data.class_service.is_null()) {
        STATUS_SHARING_VIOLATION.check_status(ErrorCode::SharingViolation)?;
    }

    let mut connect_data = dbg!(request.connect_data())?;

    device.context_mut().upper_connect_data = connect_data;

    connect_data.class_device_object = dbg!(device.device_object());
    connect_data.class_service = service_callback as PVOID;

    Ok(())
}

extern "C" fn pdo_from_ioctl(queue: WDFQUEUE, request: WDFREQUEST, output_buffer_length: usize, _input_buffer_length: usize, io_control_code: ULONG) {
    println!("WAWAWA pdo_from_ioctl 1");

    let mut status = STATUS_NOT_IMPLEMENTED;
    let mut bytes_transferred: ULONG_PTR = 0;
    if io_control_code == PdoKeyboardAttributes as u32 {
        if output_buffer_length < core::mem::size_of::<KeyboardAttributes>() {
            status = STATUS_BUFFER_TOO_SMALL;
        } else {
            let mut output_memory = core::ptr::null_mut() as WDFMEMORY;
            status = unsafe {
                call_unsafe_wdf_function_binding!(
                    WdfRequestRetrieveOutputMemory,
                    request,
                    &mut output_memory
                )
            };

            if nt_success(status) {
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

                if nt_success(status) {
                    status = STATUS_SUCCESS;
                    bytes_transferred = core::mem::size_of::<KeyboardAttributes>() as ULONG_PTR;
                } else {
                    println!("WAWAWA WdfMemoryCopyFromBuffer failed {status:#010X}");
                }
            } else {
                println!("WAWAWA WdfRequestRetrieveOutputMemory failed {status:#010X}");
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

extern "C" fn pdo_to_ioctl(queue: WDFQUEUE, request: WDFREQUEST, _output_buffer_length: usize, _input_buffer_length: usize, io_control_code: ULONG) {
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
            )
        };

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

unsafe extern "C" fn completion_routine(request: WDFREQUEST, _handle: WDFIOTARGET, params: *mut WDF_REQUEST_COMPLETION_PARAMS, context: WDFCONTEXT) {
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

