// Copyright (c) Microsoft Corporation.
// License: MIT OR Apache-2.0

use core::sync::atomic::AtomicU32;
use num_enum::TryFromPrimitive;
use wdk::{nt_success, paged_code, println};
use wdk_sys::{*};
use wdk_sys::_WDF_REQUEST_SEND_OPTIONS_FLAGS::WDF_REQUEST_SEND_OPTION_SEND_AND_FORGET;
use wdk_sys::_WDF_REQUEST_TYPE::WdfRequestTypeDeviceControlInternal;
use wdk_sys::macros::call_unsafe_wdf_function_binding;
use windows_sys::Win32::Devices::HumanInterfaceDevice::{IOCTL_KEYBOARD_QUERY_ATTRIBUTES, KEYBOARD_INPUT_DATA};

use crate::{
    wdf_object_context::*,
    DeviceContext,
    GUID_DEVINTERFACE_ECHO,
    *,
};

static mut INSTANCES: AtomicU32 = AtomicU32::new(0);


/// Worker routine called to create a device and its software resources.
///
/// # Arguments:
///
/// * `device_init` - Pointer to an opaque init structure. Memory for this
///   structure will be freed by the framework when the WdfDeviceCreate
///   succeeds. So don't access the structure after that point.
///
/// # Return value:
///
/// * `NTSTATUS`
#[link_section = "PAGE"]
pub(crate) fn echo_device_create(mut device_init: &mut WDFDEVICE_INIT) -> NTSTATUS {
    paged_code!();
    
    macros::call_unsafe_wdf_function_binding!(
        WdfFdoInitSetFilter,
        device_init
    );
    
    macros::call_unsafe_wdf_function_binding!(
        WdfDeviceInitSetDeviceType,
        device_init,
        FILE_DEVICE_KEYBOARD
    );
    
    let mut attributes = WDF_OBJECT_ATTRIBUTES {
        ..WDF_OBJECT_ATTRIBUTES::default()
    };
    
    macros::call_unsafe_wdf_function_binding!(
        WDF_OBJECT_ATTRIBUTES_INIT,
        &mut device_init
    );
    
    attributes.ContextTypeInfo =  wdf_get_context_type_info!(DeviceContext);


    let mut device = WDF_NO_HANDLE as WDFDEVICE;
    let mut nt_status = unsafe {
        macros::call_unsafe_wdf_function_binding!(
            WdfDeviceCreate,
            (core::ptr::addr_of_mut!(device_init)) as *mut *mut WDFDEVICE_INIT,
            &mut attributes,
            &mut device,
        )
    };
    
    if !nt_success(nt_status) {
        println!("Error: WdfDeviceCreate failed {nt_status:#010X}");
        return nt_status;
    }

    // Get the device context and initialize it. WdfObjectGet_DEVICE_CONTEXT is an
    // inline function generated by WDF_DECLARE_CONTEXT_TYPE macro in the
    // device.h header file. This function will do the type checking and return
    // the device context. If you pass a wrong object  handle
    // it will return NULL and assert if run under framework verifier mode.


    // Create a device interface so that application can find and talk
    // to us.
    nt_status = unsafe {
        macros::call_unsafe_wdf_function_binding!(
                WdfDeviceCreateDeviceInterface,
                device,
                &GUID_DEVINTERFACE_ECHO,
                core::ptr::null_mut(),
            )
    };
    
if !nt_success(nt_status) {
        println!("Error: WdfDeviceCreateDeviceInterface failed {nt_status:#010X}");
        return nt_status;
    }

    //
    // Configure the default queue to be Parallel. Do not use sequential queue
    // if this driver is going to be filtering PS2 ports because it can lead to
    // deadlock. The PS2 port driver sends a request to the top of the stack when it
    // receives an ioctl request and waits for it to be completed. If you use a
    // a sequential queue, this request will be stuck in the queue because of the 
    // outstanding ioctl request sent earlier to the port driver.
    //
    let mut queue_config = WDF_IO_QUEUE_CONFIG {
        Size: core::mem::size_of::<WDF_IO_QUEUE_CONFIG>() as ULONG,
        PowerManaged: _WDF_TRI_STATE::WdfUseDefault,
        DefaultQueue: true as u8,
        DispatchType: _WDF_IO_QUEUE_DISPATCH_TYPE::WdfIoQueueDispatchParallel,
        EvtIoInternalDeviceControl: Some(internal_ioctl),
        ..WDF_IO_QUEUE_CONFIG::default()
    };

    // Create queue.
    let mut nt_status = unsafe {
        macros::call_unsafe_wdf_function_binding!(
            WdfIoQueueCreate,
            device,
            &mut queue_config,
            WDF_NO_OBJECT_ATTRIBUTES,
            WDF_NO_HANDLE as *mut WDFQUEUE,
        )
    };

    if !nt_success(nt_status) {
        println!("WdfIoQueueCreate failed {nt_status:#010X}");
        return nt_status;
    }

    let mut pdo_queue_config = WDF_IO_QUEUE_CONFIG {
        Size: core::mem::size_of::<WDF_IO_QUEUE_CONFIG>() as ULONG,
        PowerManaged: _WDF_TRI_STATE::WdfUseDefault,
        DefaultQueue: true as u8,
        DispatchType: _WDF_IO_QUEUE_DISPATCH_TYPE::WdfIoQueueDispatchParallel,
        EvtIoInternalDeviceControl: Some(pdo_ioctl),
        ..WDF_IO_QUEUE_CONFIG::default()
    };
    
    let pdo_queue = WDFQUEUE::default();
    
    nt_status = unsafe {
        macros::call_unsafe_wdf_function_binding!(
            WdfIoQueueCreate,
            device,
            &mut pdo_queue_config,
            WDF_NO_OBJECT_ATTRIBUTES,
            &mut pdo_queue,
        )
    };
    
    if !nt_success(nt_status) {
        println!("WdfIoQueueCreate for pdo failed {nt_status:#010X}");
        return nt_status;
    }


    let device_context: *mut DeviceContext =
        unsafe { wdf_object_get_device_context(device as WDFOBJECT) };
    unsafe { (*device_context).raw_pdo_queue = pdo_queue };

    let current = unsafe {
        INSTANCES.fetch_add(1, core::sync::atomic::Ordering::SeqCst)
    } + 1;
    
    
    //nt_status = create_pdo(device, current);
    

    nt_status
}

const fn ctl_code(device_type: u32, function: u32, method: u32, access: u32) -> u32 {
    (device_type << 16) | (access << 14) | (function << 2) | method
}

#[derive(Debug, Eq, PartialEq, TryFromPrimitive)]
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
}


extern "C" fn internal_ioctl(queue: WDFQUEUE, request: WDFREQUEST, output_buffer_length: usize, input_buffer_length: usize, io_control_code: ULONG) {
    paged_code!();
    
    let device = macros::call_unsafe_wdf_function_binding!(
        WdfIoQueueGetDevice,
        queue
    );
    let device_context_ptr: *mut DeviceContext =
        unsafe { wdf_object_get_device_context(device as WDFOBJECT) };
    
    let device_context: &mut DeviceContext = unsafe { device_context_ptr.as_mut().unwrap() }; // TODO: Handle this better.
    
    let mut forward_request = false;
    
    let mut status = STATUS_SUCCESS;
    let mut completion_context = WDF_NO_CONTEXT;
    if (io_control_code == IOCTL_KEYBOARD_QUERY_ATTRIBUTES) {
        forward_request = true;
        completion_context = device_context_ptr as PVOID;
    }
    
    
    if let Ok(ioctl) = KeyboardIoctl::try_from(io_control_code as isize) {
       match ioctl {
                KeyboardIoctl::KeyboardConnect => {
                    // Only allow one connection at a time. (for now)
                    if !device_context.upper_connect_data.class_service.is_null() {
                        status = STATUS_SHARING_VIOLATION;
                    }
                    
                    let mut connect_data = ConnectData::default();
                    
                    // Get the input buffer from the request.
                    status = macros::call_unsafe_wdf_function_binding!(
                        WdfRequestRetrieveInputBuffer,
                        request,
                        core::mem::size_of::<ConnectData>(),
                        &mut connect_data as *mut _ as *mut _,
                        core::ptr::null_mut(),
                    );
                    
                    if !nt_success(status) {
                        println!("WdfRequestRetrieveInputBuffer failed {status:#010X}");
                    } else {
                        connect_data.class_device_object = call_unsafe_wdf_function_binding!(
                            WdfDeviceWdmGetDeviceObject,
                            device
                        );
                        connect_data.class_service = service_callback as PVOID;
                    }
                },
            KeyboardIoctl::KeyboardDisconnect => {
                // Disconnect. This is allowed even if there is no outstanding connect.
                // TODO - do we need to free anything?
                    device_context.upper_connect_data = ConnectData::default();
                },
           _ => {}
       }
        
    }
    
    if !nt_success(status) {
        unsafe {
            macros::call_unsafe_wdf_function_binding!(
                WdfRequestComplete,
                request,
                status
            );
        }
        return;
    }
    
    if forward_request {
        /*        
        Translate to rust:
        status = WdfRequestRetrieveOutputMemory(Request, &outputMemory); 

        if (!NT_SUCCESS(status)) {
            DebugPrint(("WdfRequestRetrieveOutputMemory failed: 0x%x\n", status));
            WdfRequestComplete(Request, status);
            return;
        }

        status = WdfIoTargetFormatRequestForInternalIoctl(WdfDeviceGetIoTarget(hDevice),
                                                         Request,
                                                         IoControlCode,
                                                         NULL,
                                                         NULL,
                                                         outputMemory,
                                                         NULL);

        if (!NT_SUCCESS(status)) {
            DebugPrint(("WdfIoTargetFormatRequestForInternalIoctl failed: 0x%x\n", status));
            WdfRequestComplete(Request, status);
            return;
        }
    
        // 
        // Set our completion routine with a context area that we will save
        // the output data into
        //
        WdfRequestSetCompletionRoutine(Request,
                                    KbFilterRequestCompletionRoutine,
                                    completionContext);

        ret = WdfRequestSend(Request,
                             WdfDeviceGetIoTarget(hDevice),
                             WDF_NO_SEND_OPTIONS);

        if (ret == FALSE) {
            status = WdfRequestGetStatus (Request);
            DebugPrint( ("WdfRequestSend failed: 0x%x\n", status));
            WdfRequestComplete(Request, status);
        }
       
         */
        
        status = unsafe {
            macros::call_unsafe_wdf_function_binding!(
                WdfRequestRetrieveOutputMemory,
                request,
                core::mem::size_of::<ConnectData>()
            )
        };
        
        if !nt_success(status) {
            println!("WdfRequestRetrieveOutputMemory failed {status:#010X}");
            unsafe {
                macros::call_unsafe_wdf_function_binding!(
                    WdfRequestComplete,
                    request,
                    status
                );
            }
            return;
        }
        
        let output_memory = WDFMEMORY::default();
        
        status = unsafe {
            macros::call_unsafe_wdf_function_binding!(
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
            println!("WdfIoTargetFormatRequestForInternalIoctl failed {status:#010X}");
            unsafe {
                macros::call_unsafe_wdf_function_binding!(
                    WdfRequestComplete,
                    request,
                    status
                );
            }
            return;
        }
        
        unsafe {
            macros::call_unsafe_wdf_function_binding!(
                WdfRequestSetCompletionRoutine,
                request,
                Some(completion_routine),
                completion_context,
            );
        }
        
        let ret = unsafe {
            macros::call_unsafe_wdf_function_binding!(
                WdfRequestSend,
                request,
                macros::call_unsafe_wdf_function_binding!(
                    WdfDeviceGetIoTarget,
                    device
                ),
                WDF_NO_SEND_OPTIONS as *mut WDF_REQUEST_SEND_OPTIONS,
            )
        };
        
        if !ret {
            status = unsafe {
                macros::call_unsafe_wdf_function_binding!(
                    WdfRequestGetStatus,
                    request
                )
            };
            println!("WdfRequestSend failed {status:#010X}");
            unsafe {
                macros::call_unsafe_wdf_function_binding!(
                    WdfRequestComplete,
                    request,
                    status
                );
            }
        }
    } else {
        let options = WDF_REQUEST_SEND_OPTIONS {
            Size: core::mem::size_of::<WDF_REQUEST_SEND_OPTIONS>() as ULONG,
            Flags: WDF_REQUEST_SEND_OPTION_SEND_AND_FORGET as ULONG,
            ..WDF_REQUEST_SEND_OPTIONS::default()
        };
        
        let ret = unsafe {
            macros::call_unsafe_wdf_function_binding!(
                WdfRequestSend,
                request,
                macros::call_unsafe_wdf_function_binding!(
                    WdfDeviceGetIoTarget,
                    device
                ),
                &options as *const _ as *mut _,
            );
        };
        
        if !ret {
            status = unsafe {
                macros::call_unsafe_wdf_function_binding!(
                    WdfRequestGetStatus,
                    request
                )
            };
            println!("WdfRequestSend failed {status:#010X}");
            unsafe {
                macros::call_unsafe_wdf_function_binding!(
                    WdfRequestComplete,
                    request,
                    status
                );
            }
        }
    }
}

extern "C" fn pdo_ioctl(queue: WDFQUEUE, request: WDFREQUEST, output_buffer_length: usize, input_buffer_length: usize, io_control_code: ULONG) {
    paged_code!();
    
}


/*
VOID
KbFilter_ServiceCallback(
    IN PDEVICE_OBJECT  DeviceObject,
    IN PKEYBOARD_INPUT_DATA InputDataStart,
    IN PKEYBOARD_INPUT_DATA InputDataEnd,
    IN OUT PULONG InputDataConsumed
    )
 */

type ServiceCallback = extern "C" fn(device_object: PDEVICE_OBJECT, input_data_start: *mut KEYBOARD_INPUT_DATA, input_data_end: *mut KEYBOARD_INPUT_DATA, input_data_consumed: PULONG);


extern "C" fn service_callback(device_object: PDEVICE_OBJECT, input_data_start: *mut KEYBOARD_INPUT_DATA, input_data_end: *mut KEYBOARD_INPUT_DATA, input_data_consumed: PULONG) {
    paged_code!();
    
    let device = macros::call_unsafe_wdf_function_binding!(
        WdfWdmDeviceGetWdfDeviceHandle,
        device_object
    );
    
    let device_context: &mut DeviceContext =
        unsafe { wdf_object_get_device_context(device as WDFOBJECT).as_mut().unwrap() }; // TODO: Handle this better.
    
    println!("Service callback called for device {:#010X}", device);
    let input_data_length = (input_data_end as usize - input_data_start as usize) / core::mem::size_of::<KEYBOARD_INPUT_DATA>();
    if input_data_length > 0 {
        let input_data_slice = unsafe { core::slice::from_raw_parts(input_data_start, input_data_length) };
        for (i, input_data) in input_data_slice.iter().enumerate() {
            println!("Input data: {}", i);
            println!("  UnitId: {}", input_data.UnitId);
            println!("  MakeCode: {}", input_data.MakeCode);
            println!("  Flags: {}", input_data.Flags);
            println!("  Reserved: {}", input_data.Reserved);
            println!("  ExtraInformation: {}", input_data.ExtraInformation);
        }
    }
    
    
    if !device_context.upper_connect_data.class_service.is_null() {
        let callback: ServiceCallback = unsafe { core::mem::transmute(device_context.upper_connect_data.class_service) };
        
        callback(device_object, input_data_start, input_data_end, input_data_consumed);
    }
}

/*
VOID
KbFilterRequestCompletionRoutine(
    WDFREQUEST                  Request,
    WDFIOTARGET                 Target,
    PWDF_REQUEST_COMPLETION_PARAMS CompletionParams,
    WDFCONTEXT                  Context
   )
/*++

Routine Description:

    Completion Routine

Arguments:

    Target - Target handle
    Request - Request handle
    Params - request completion params
    Context - Driver supplied context


Return Value:

    VOID

--*/
{
    WDFMEMORY   buffer = CompletionParams->Parameters.Ioctl.Output.Buffer;
    NTSTATUS    status = CompletionParams->IoStatus.Status;

    UNREFERENCED_PARAMETER(Target);
 
    //
    // Save the keyboard attributes in our context area so that we can return
    // them to the app later.
    //
    if (NT_SUCCESS(status) && 
        CompletionParams->Type == WdfRequestTypeDeviceControlInternal &&
        CompletionParams->Parameters.Ioctl.IoControlCode == IOCTL_KEYBOARD_QUERY_ATTRIBUTES) {

        if( CompletionParams->Parameters.Ioctl.Output.Length >= sizeof(KEYBOARD_ATTRIBUTES)) {
            
            status = WdfMemoryCopyToBuffer(buffer,
                                           CompletionParams->Parameters.Ioctl.Output.Offset,
                                           &((PDEVICE_EXTENSION)Context)->KeyboardAttributes,
                                            sizeof(KEYBOARD_ATTRIBUTES)
                                          );
        }
    }

    WdfRequestComplete(Request, status);

    return;
}
 */

extern "C" unsafe fn completion_routine(request: WDFREQUEST, handle: WDFIOTARGET, params: *mut WDF_REQUEST_COMPLETION_PARAMS, context: WDFCONTEXT) {
    let buffer = (*params).Parameters.Ioctl.Output.Buffer;
    let mut status = (*params).IoStatus.__bindgen_anon_1.Status;
    
    if nt_success(status) && (*params).Type == WdfRequestTypeDeviceControlInternal && (*params).Parameters.Ioctl.IoControlCode == IOCTL_KEYBOARD_QUERY_ATTRIBUTES {
        if (*params).Parameters.Ioctl.Output.Length >= core::mem::size_of::<KEYBOARD_ATTRIBUTES>() {
            let device_context: &mut DeviceContext = core::mem::transmute(context);
            status = macros::call_unsafe_wdf_function_binding!(
                WdfMemoryCopyToBuffer,
                buffer,
                (*params).Parameters.Ioctl.Output.Offset,
                (&mut device_context.keyboard_attributes) as *mut _ as *mut _,
                core::mem::size_of::<KEYBOARD_ATTRIBUTES>(),
            );
        }
    }
    
    macros::call_unsafe_wdf_function_binding!(
        WdfRequestComplete,
        request,
        status
    );
}