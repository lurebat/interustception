// Copyright (c) Microsoft Corporation.
// License: MIT OR Apache-2.0

use core::sync::atomic::AtomicU32;
use wdk::{nt_success, paged_code, println};
use wdk_sys::{*};
use wdk_sys::_WDF_REQUEST_SEND_OPTIONS_FLAGS::WDF_REQUEST_SEND_OPTION_SEND_AND_FORGET;
use wdk_sys::_WDF_REQUEST_TYPE::WdfRequestTypeDeviceControlInternal;
use wdk_sys::_WDF_TRI_STATE::{WdfTrue, WdfUseDefault};
use wdk_sys::macros::call_unsafe_wdf_function_binding;
use wdk_sys::ntddk::DbgBreakPointWithStatus;

use crate::{
    wdf_object_context::*,
    DeviceContext,
    GUID_DEVINTERFACE_ECHO,
    *,
};
use crate::device::KeyboardIoctl::PdoKeyboardAttributes;
use crate::foreign::{ConnectData, KeyboardAttributes, KeyboardInputData};

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
pub(crate) unsafe fn echo_device_create(mut device_init: &mut WDFDEVICE_INIT) -> NTSTATUS {
    paged_code!();
    
    println!("echo_device_create called");

    call_unsafe_wdf_function_binding!(
        WdfFdoInitSetFilter,
        device_init
    );
    
    println!("WdfFdoInitSetFilter called");

    call_unsafe_wdf_function_binding!(
        WdfDeviceInitSetDeviceType,
        device_init,
        FILE_DEVICE_KEYBOARD
    );
    
    println!("WdfDeviceInitSetDeviceType called");

    let mut attributes = WDF_OBJECT_ATTRIBUTES {
        Size: core::mem::size_of::<WDF_OBJECT_ATTRIBUTES>() as ULONG,
        ExecutionLevel: _WDF_EXECUTION_LEVEL::WdfExecutionLevelInheritFromParent,
        SynchronizationScope: _WDF_SYNCHRONIZATION_SCOPE::WdfSynchronizationScopeInheritFromParent,
        ..WDF_OBJECT_ATTRIBUTES::default()
    };

    attributes.ContextTypeInfo = wdf_get_context_type_info!(DeviceContext);
    
    println!("WdfDeviceInitSetDeviceType called");
    
    let mut device = WDF_NO_HANDLE as WDFDEVICE;
    let nt_status = unsafe {
        call_unsafe_wdf_function_binding!(
            WdfDeviceCreate,
            (core::ptr::addr_of_mut!(device_init)) as *mut *mut WDFDEVICE_INIT,
            &mut attributes,
            &mut device,
        )
    };
    
    println!("WdfDeviceCreate called");

    if !nt_success(nt_status) {
        println!("Error: WdfDeviceCreate failed {nt_status:#010X}");
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
    
    println!("WDF_IO_QUEUE_CONFIG initialized");

    // Create queue.
    let mut nt_status = unsafe {
        call_unsafe_wdf_function_binding!(
            WdfIoQueueCreate,
            device,
            &mut queue_config,
            WDF_NO_OBJECT_ATTRIBUTES,
            WDF_NO_HANDLE as *mut WDFQUEUE,
        )
    };
    
    println!("WdfIoQueueCreate called");

    if !nt_success(nt_status) {
        println!("WdfIoQueueCreate failed {nt_status:#010X}");
        return nt_status;
    }
    
    println!("WdfIoQueueCreate succeeded");

    let mut pdo_queue_config = WDF_IO_QUEUE_CONFIG {
        Size: core::mem::size_of::<WDF_IO_QUEUE_CONFIG>() as ULONG,
        PowerManaged: _WDF_TRI_STATE::WdfUseDefault,
        DefaultQueue: true as u8,
        DispatchType: _WDF_IO_QUEUE_DISPATCH_TYPE::WdfIoQueueDispatchParallel,
        EvtIoInternalDeviceControl: Some(pdo_from_ioctl),
        ..WDF_IO_QUEUE_CONFIG::default()
    };
    
    println!("WDF_IO_QUEUE_CONFIG initialized");

    let mut pdo_queue = null_mut() as WDFQUEUE;

    println!("WdfIoQueueCreate called");
    
    nt_status = unsafe {
        call_unsafe_wdf_function_binding!(
            WdfIoQueueCreate,
            device,
            &mut pdo_queue_config,
            WDF_NO_OBJECT_ATTRIBUTES,
            &mut pdo_queue,
        )
    };
    
    println!("WdfIoQueueCreate called");

    if !nt_success(nt_status) {
        println!("WdfIoQueueCreate for pdo failed {nt_status:#010X}");
        return nt_status;
    }
    
    println!("WdfIoQueueCreate for pdo succeeded");
    
    let device_context: *mut DeviceContext =
        unsafe { wdf_object_get_device_context(device as WDFOBJECT) };
    unsafe { (*device_context).raw_pdo_queue = pdo_queue };

    let current = unsafe {
        INSTANCES.fetch_add(1, core::sync::atomic::Ordering::SeqCst)
    } + 1;


    nt_status = create_pdo(device, current);


    nt_status
}

static GUID_CLASS_KEYBOARD: GUID = GUID {
    Data1: 0x4D36E96Bu32,
    Data2: 0xE325u16,
    Data3: 0x11CEu16,
    Data4: [
        0xBFu8, 0xC1u8, 0x08u8, 0x00u8, 0x2Bu8, 0xE1u8, 0x03u8, 0x18u8,
    ],
};

// the string is {A65C87F9-BE02-4ed9-92EC-012D416169FA}\\Interustception\0
const RAW_DEVICE_ID: &[u8] =b"\xff\xfe{\x00A\x006\x005\x00C\x008\x007\x00F\x009\x00-\x00B\x00E\x000\x002\x00-\x004\x00e\x00d\x009\x00-\x009\x002\x00E\x00C\x00-\x000\x001\x002\x00D\x004\x001\x006\x001\x006\x009\x00F\x00A\x00}\x00\\\x00I\x00n\x00t\x00e\x00r\x00u\x00s\x00t\x00c\x00e\x00p\x00t\x00i\x00o\x00n\x00\x00\x00";

const DEVICE_ID: UNICODE_STRING =
    UNICODE_STRING {
        Length: RAW_DEVICE_ID.len() as u16,
        MaximumLength: RAW_DEVICE_ID.len() as u16,
        Buffer: RAW_DEVICE_ID.as_ptr() as *mut _,
    };

fn create_pdo(device: WDFDEVICE, current: u32) -> NTSTATUS {
    /*
        NTSTATUS                    status;
    PWDFDEVICE_INIT             pDeviceInit = NULL;
    PRPDO_DEVICE_DATA           pdoData = NULL;
    WDFDEVICE                   hChild = NULL;
    WDF_OBJECT_ATTRIBUTES       pdoAttributes;
    WDF_DEVICE_PNP_CAPABILITIES pnpCaps;
    WDF_IO_QUEUE_CONFIG         ioQueueConfig;
    WDFQUEUE                    queue;
    WDF_DEVICE_STATE            deviceState;
    PDEVICE_EXTENSION           devExt;
    DECLARE_CONST_UNICODE_STRING(deviceId,KBFILTR_DEVICE_ID );
    DECLARE_CONST_UNICODE_STRING(hardwareId,KBFILTR_DEVICE_ID );
    DECLARE_CONST_UNICODE_STRING(deviceLocation,L"Keyboard Filter\0" );
    DECLARE_UNICODE_STRING_SIZE(buffer, MAX_ID_LEN);
     */
    
    println!("create_pdo called");

    let mut device_init = unsafe {
        call_unsafe_wdf_function_binding!(
            WdfPdoInitAllocate,
            device,
        )
    };

    if device_init.is_null() {
        println!("WdfPdoInitAllocate failed");
        return STATUS_INSUFFICIENT_RESOURCES;
    }

    /*
     //
    // Mark the device RAW so that the child device can be started
    // and accessed without requiring a function driver. Since we are
    // creating a RAW PDO, we must provide a class guid.
    //
    status = WdfPdoInitAssignRawDevice(pDeviceInit, &GUID_DEVCLASS_KEYBOARD);
    if (!NT_SUCCESS(status)) {
        goto Cleanup;
    }
     */

    // 4D36E96B-E325-11CE-BFC1-08002BE10318
    
    println!("WdfPdoInitAssignRawDevice called");


    let mut status = unsafe {
        call_unsafe_wdf_function_binding!(
            WdfPdoInitAssignRawDevice,
            device_init,
            &GUID_CLASS_KEYBOARD,
        )
    };
    
    if !nt_success(status) {
        println!("WdfPdoInitAssignRawDevice failed {status:#010X}");
        return status;
    }
    
    /*
        //
    // Assign DeviceID - This will be reported to IRP_MN_QUERY_ID/BusQueryDeviceID
    //
    status = WdfPdoInitAssignDeviceID(pDeviceInit, &deviceId);
    if (!NT_SUCCESS(status)) {
        goto Cleanup;
    }
     */
    
    println!("WdfPdoInitAssignRawDevice called");
    
    status = unsafe {
        call_unsafe_wdf_function_binding!(
            WdfPdoInitAssignDeviceID,
            device_init,
            &DEVICE_ID,
        ) };
    
    if !nt_success(status) {
        println!("WdfPdoInitAssignDeviceID failed {status:#010X}");
        return status;
    }
    
    let mut buffer_bytes = [0u16; 128];
    let mut buffer = UNICODE_STRING {
        Length: 0,
        MaximumLength: 128,
        Buffer: buffer_bytes.as_mut_ptr(),
    };
    
    println!("WdfPdoInitAssignDeviceID called");
    
    /*
        //
    // We could be enumerating more than one children if the filter attaches
    // to multiple instances of keyboard, so we must provide a
    // BusQueryInstanceID. If we don't, system will throw CA bugcheck.
    */
    
    // buffer_bytes - put in the instance number
    let current_first_digit = current / 10;
    let current_second_digit = current % 10;
    buffer_bytes[0] = '0' as u16 + current_first_digit as u16;
    buffer_bytes[1] = '0' as u16 + current_second_digit as u16;
    buffer.Length = 2u16 * core::mem::size_of::<u16>() as u16;
    
    status = unsafe {
        call_unsafe_wdf_function_binding!(
            WdfPdoInitAssignInstanceID,
            device_init,
            &buffer,
        )
    };
    
    if !nt_success(status) {
        println!("WdfPdoInitAssignInstanceID failed {status:#010X}");
        return status;
    }
    
    /*
     //
    // Provide a description about the device. This text is usually read from
    // the device. In the case of USB device, this text comes from the string
    // descriptor. This text is displayed momentarily by the PnP manager while
    // it's looking for a matching INF. If it finds one, it uses the Device
    // Description from the INF file to display in the device manager.
    // Since our device is raw device and we don't provide any hardware ID
    // to match with an INF, this text will be displayed in the device manager.
    //
    status = RtlUnicodeStringPrintf(&buffer,L"Keyboard_Filter_%02d", InstanceNo );
    if (!NT_SUCCESS(status)) {
        goto Cleanup;
    }
     */
    buffer_bytes[0] = 'K' as u16;
    buffer_bytes[1] = 'e' as u16;
    buffer_bytes[2] = 'y' as u16;
    buffer_bytes[3] = 'b' as u16;
    buffer_bytes[4] = 'o' as u16;
    buffer_bytes[5] = 'a' as u16;
    buffer_bytes[6] = 'r' as u16;
    buffer_bytes[7] = 'd' as u16;
    buffer_bytes[8] = '_' as u16;
    buffer_bytes[9] = 'F' as u16;
    buffer_bytes[10] = 'i' as u16;
    buffer_bytes[11] = 'l' as u16;
    buffer_bytes[12] = 't' as u16;
    buffer_bytes[13] = 'e' as u16;
        
    buffer_bytes[14] = 'r' as u16;
    buffer_bytes[15] = '_' as u16;
    buffer_bytes[16] = '0' as u16 + current_first_digit as u16;
    buffer_bytes[17] = '0' as u16 + current_second_digit as u16;
    buffer_bytes[18] = 0;
    buffer.Length = 18u16 * core::mem::size_of::<u16>() as u16;
    
    /*
        //
    // You can call WdfPdoInitAddDeviceText multiple times, adding device
    // text for multiple locales. When the system displays the text, it
    // chooses the text that matches the current locale, if available.
    // Otherwise it will use the string for the default locale.
    // The driver can specify the driver's default locale by calling
    // WdfPdoInitSetDefaultLocale.
    //
    status = WdfPdoInitAddDeviceText(pDeviceInit,
                                        &buffer,
                                        &deviceLocation,
                                        0x409
                                        );
    if (!NT_SUCCESS(status)) {
        goto Cleanup;
    }

    WdfPdoInitSetDefaultLocale(pDeviceInit, 0x409);
     */
    
    println!("add text");
    
    status = unsafe { call_unsafe_wdf_function_binding!(
        WdfPdoInitAddDeviceText,
        device_init,
        &buffer,
        &DEVICE_ID,
        0x409,
    ) };
    
    if !nt_success(status) {
        println!("WdfPdoInitAddDeviceText failed {status:#010X}");
        return status;
    }
    
    println!("set default locale");
    
    unsafe { call_unsafe_wdf_function_binding!(
        WdfPdoInitSetDefaultLocale,
        device_init,
        0x409,
    ) };
    


    let mut attributes = WDF_OBJECT_ATTRIBUTES {
        Size: core::mem::size_of::<WDF_OBJECT_ATTRIBUTES>() as ULONG,
        ExecutionLevel: _WDF_EXECUTION_LEVEL::WdfExecutionLevelInheritFromParent,
        SynchronizationScope: _WDF_SYNCHRONIZATION_SCOPE::WdfSynchronizationScopeInheritFromParent,
        ..WDF_OBJECT_ATTRIBUTES::default()
    };
    
    
    attributes.ContextTypeInfo = wdf_get_context_type_info!(PdoContext);
    
    /*
        //
    // Set up our queue to allow forwarding of requests to the parent
    // This is done so that the cached Keyboard Attributes can be retrieved
    //
    WdfPdoInitAllowForwardingRequestToParent(pDeviceInit);
    
        status = WdfDeviceCreate(&pDeviceInit, &pdoAttributes, &hChild);
    if (!NT_SUCCESS(status)) {
        goto Cleanup;
    }
     */
    
    println!("WdfPdoInitAllowForwardingRequestToParent");

    unsafe {
        call_unsafe_wdf_function_binding!(
        WdfPdoInitAllowForwardingRequestToParent,
        device_init,    )
    };
    
    println!("WdfDeviceCreate");
    
    let mut pdo = WDF_NO_HANDLE as WDFDEVICE;
    status = unsafe {
        call_unsafe_wdf_function_binding!(
            WdfDeviceCreate,
            &mut device_init,
            &mut attributes,
            &mut pdo,
        )
    };
    
    println!("WdfDeviceCreate called");
    
    if !nt_success(status) {
        println!("WdfDeviceCreate failed {status:#010X}");
        return status;
    }
    
    let pdo_context = unsafe { get_pdo_context(pdo as WDFOBJECT) };
    unsafe { (*pdo_context).instance = current };
    
    let device_context = unsafe { wdf_object_get_device_context(device as WDFOBJECT) };
    unsafe { (*pdo_context).queue = (*device_context).raw_pdo_queue };
    
    println!("WdfDeviceCreate succeeded");
    
    /*
     //
    // Configure the default queue associated with the control device object
    // to be Serial so that request passed to EvtIoDeviceControl are serialized.
    // A default queue gets all the requests that are not
    // configure-fowarded using WdfDeviceConfigureRequestDispatching.
    //

    WDF_IO_QUEUE_CONFIG_INIT_DEFAULT_QUEUE(&ioQueueConfig,
                                    WdfIoQueueDispatchSequential);

    ioQueueConfig.EvtIoDeviceControl = KbFilter_EvtIoDeviceControlForRawPdo;

    status = WdfIoQueueCreate(hChild,
                                        &ioQueueConfig,
                                        WDF_NO_OBJECT_ATTRIBUTES,
                                        &queue // pointer to default queue
                                        );
    if (!NT_SUCCESS(status)) {
        DebugPrint( ("WdfIoQueueCreate failed 0x%x\n", status));
        goto Cleanup;
    }

     */
    
    println!("WdfDeviceCreate succeeded");

    let mut queue_config = WDF_IO_QUEUE_CONFIG {
        Size: core::mem::size_of::<WDF_IO_QUEUE_CONFIG>() as ULONG,
        PowerManaged: _WDF_TRI_STATE::WdfUseDefault,
        DefaultQueue: true as u8,
        DispatchType: _WDF_IO_QUEUE_DISPATCH_TYPE::WdfIoQueueDispatchSequential,
        EvtIoInternalDeviceControl: Some(pdo_to_ioctl),
        ..WDF_IO_QUEUE_CONFIG::default()
    };
    
    println!("WDF_IO_QUEUE_CONFIG initialized");
    
    let mut queue : WDFQUEUE = null_mut() as WDFQUEUE;
    
    status = unsafe {
        call_unsafe_wdf_function_binding!(
            WdfIoQueueCreate,
            pdo,
            &mut queue_config,
            WDF_NO_OBJECT_ATTRIBUTES,
            &mut queue,
        )
    };
    
    println!("WdfIoQueueCreate called");
    
    if !nt_success(status) {
        println!("WdfIoQueueCreate failed {status:#010X}");
        return status;
    }
    
    let mut caps = WDF_DEVICE_PNP_CAPABILITIES {
        Size: core::mem::size_of::<WDF_DEVICE_PNP_CAPABILITIES>() as ULONG,
        LockSupported: WdfUseDefault,
        EjectSupported: WdfUseDefault,
        Removable: WdfTrue,
        DockDevice: WdfUseDefault,
        UniqueID: WdfUseDefault,
        SilentInstall: WdfUseDefault,
        SurpriseRemovalOK: WdfTrue,
        HardwareDisabled: WdfUseDefault,
        NoDisplayInUI: WdfUseDefault,
        Address: current,
        UINumber: current,
    };
    
    unsafe {
        call_unsafe_wdf_function_binding!(
            WdfDeviceSetPnpCapabilities,
            pdo,
            &mut caps,
        )
    };
    
    println!("WdfDeviceSetPnpCapabilities called");
    
    /*
     //
    // Tell the Framework that this device will need an interface so that
    // application can find our device and talk to it.
    //
    status = WdfDeviceCreateDeviceInterface(
                 hChild,
                 &GUID_DEVINTERFACE_KBFILTER,
                 NULL
             );

    if (!NT_SUCCESS (status)) {
        DebugPrint( ("WdfDeviceCreateDeviceInterface failed 0x%x\n", status));
        goto Cleanup;
    }
     */
    
    status = unsafe {
        call_unsafe_wdf_function_binding!(
            WdfDeviceCreateDeviceInterface,
            pdo,
            &GUID_DEVINTERFACE_ECHO,
            core::ptr::null_mut(),
        )
    };
    
    println!("WdfDeviceCreateDeviceInterface called");
    
    if !nt_success(status) {
        println!("WdfDeviceCreateDeviceInterface failed {status:#010X}");
        return status;
    }
    
    status
    
    // todo - cleanup
}

const fn ctl_code(device_type: u32, function: u32, method: u32, access: u32) -> u32 {
    (device_type << 16) | (access << 14) | (function << 2) | method
}

#[derive(Debug, Eq, PartialEq)]
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

impl KeyboardIoctl {
    fn try_from(value: u32) -> Result<Self, u32> {
        match value {
            0x801 => Ok(KeyboardIoctl::SetPrecedence),
            0x802 => Ok(KeyboardIoctl::GetPrecedence),
            0x804 => Ok(KeyboardIoctl::SetFiler),
            0x808 => Ok(KeyboardIoctl::GetFilter),
            0x810 => Ok(KeyboardIoctl::SetEvent),
            0x820 => Ok(KeyboardIoctl::Write),
            0x840 => Ok(KeyboardIoctl::Read),
            0x880 => Ok(KeyboardIoctl::GetHardwareId),
            0x80 => Ok(KeyboardIoctl::KeyboardConnect),
            0x100 => Ok(KeyboardIoctl::KeyboardDisconnect),
            720896u32 => Ok(KeyboardIoctl::KeyboardQueryAttributes),
            _ => Err(value),
        }
    }
}


#[link_section = "PAGE"]
unsafe extern "C" fn internal_ioctl(queue: WDFQUEUE, request: WDFREQUEST, _output_buffer_length: usize, _input_buffer_length: usize, io_control_code: ULONG) {
    paged_code!();

    let device = call_unsafe_wdf_function_binding!(
        WdfIoQueueGetDevice,
        queue
    );
    let device_context_ptr: *mut DeviceContext =
        unsafe { wdf_object_get_device_context(device as WDFOBJECT) };

    let device_context: &mut DeviceContext = unsafe { device_context_ptr.as_mut().unwrap() }; // TODO: Handle this better.

    let mut forward_request = false;

    let mut status = STATUS_SUCCESS;
    let mut completion_context = WDF_NO_CONTEXT;


    match KeyboardIoctl::try_from(io_control_code as u32) {
        Ok(KeyboardIoctl::KeyboardConnect) => {
            // Only allow one connection at a time. (for now)
            if !device_context.upper_connect_data.class_service.is_null() {
                status = STATUS_SHARING_VIOLATION;
            } else {
                let mut connect_data = ConnectData::default();

                // Get the input buffer from the request.
                status = call_unsafe_wdf_function_binding!(
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
            }
        }
        Ok(KeyboardIoctl::KeyboardDisconnect) => {
            // Disconnect. This is allowed even if there is no outstanding connect.
            // TODO - do we need to free anything?
            device_context.upper_connect_data = ConnectData::default();
        }
        Ok(KeyboardIoctl::KeyboardQueryAttributes) => {
            // Get keyboard attributes
            forward_request = true;
            completion_context = device_context_ptr as PVOID;
        }
        _ => {}
    }


    if !nt_success(status) {
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
        let mut output_memory = null_mut() as WDFMEMORY;

        status = unsafe {
            call_unsafe_wdf_function_binding!(
                WdfRequestRetrieveOutputMemory,
                request,
                &mut output_memory
            )
        };

        if !nt_success(status) {
            println!("WdfRequestRetrieveOutputMemory failed {status:#010X}");
            unsafe {
                call_unsafe_wdf_function_binding!(
                    WdfRequestComplete,
                    request,
                    status
                );
            }
            return;
        }


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
            println!("WdfIoTargetFormatRequestForInternalIoctl failed {status:#010X}");
            unsafe {
                call_unsafe_wdf_function_binding!(
                    WdfRequestComplete,
                    request,
                    status
                );
            }
            return;
        }

        unsafe {
            call_unsafe_wdf_function_binding!(
                WdfRequestSetCompletionRoutine,
                request,
                Some(completion_routine),
                completion_context,
            );
        }

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

        if ret == 0 {
            status = unsafe {
                call_unsafe_wdf_function_binding!(
                    WdfRequestGetStatus,
                    request
                )
            };
            println!("WdfRequestSend failed {status:#010X}");
            unsafe {
                call_unsafe_wdf_function_binding!(
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

        if ret == 0 {
            status = unsafe {
                call_unsafe_wdf_function_binding!(
                    WdfRequestGetStatus,
                    request
                )
            };
            println!("WdfRequestSend failed {status:#010X}");
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
    println!("pdo_from_ioctl called");
    
    let mut status = STATUS_NOT_IMPLEMENTED;
    let mut bytes_transferred: ULONG_PTR = 0;
    if io_control_code == PdoKeyboardAttributes as u32 {
        if output_buffer_length < core::mem::size_of::<KeyboardAttributes>() {
            status = STATUS_BUFFER_TOO_SMALL;
        }
        else {
            let mut output_memory = null_mut() as WDFMEMORY;
            status = unsafe {
                call_unsafe_wdf_function_binding!(
                    WdfRequestRetrieveOutputMemory,
                    request,
                    &mut output_memory
                )
            };

            if !nt_success(status) {
                println!("WdfRequestRetrieveOutputMemory failed {status:#010X}");
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
                    println!("WdfMemoryCopyFromBuffer failed {status:#010X}");
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
    println!("pdo_to_ioctl called");
    
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
            println!("WdfRequestForwardToParentDeviceIoQueue failed {status:#010X}");
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


#[link_section = "PAGE"]
unsafe extern "C" fn service_callback(device_object: PDEVICE_OBJECT, input_data_start: *mut KeyboardInputData, input_data_end: *mut KeyboardInputData, input_data_consumed: PULONG) {
    paged_code!();

    let device = call_unsafe_wdf_function_binding!(
        WdfWdmDeviceGetWdfDeviceHandle,
        device_object
    );

    let device_context: &mut DeviceContext =
        unsafe { wdf_object_get_device_context(device as WDFOBJECT).as_mut().unwrap() }; // TODO: Handle this better.

    println!("Service callback called for device");
    let input_data_length = (input_data_end as usize - input_data_start as usize) / core::mem::size_of::<KeyboardInputData>();
    if input_data_length > 0 {
        let input_data_slice = unsafe { core::slice::from_raw_parts(input_data_start, input_data_length) };
        for (i, input_data) in input_data_slice.iter().enumerate() {
            println!("Input data: {}", i);
            println!("  unit_id: {}", input_data.unit_id);
            println!("  make_code: {}", input_data.make_code);
            println!("  flags: {}", input_data.flags);
            println!("  reserved: {}", input_data.reserved);
            println!("  extra_information: {}", input_data.extra_information);
        }
    }


    if !device_context.upper_connect_data.class_service.is_null() {
        let callback: ServiceCallback = unsafe { core::mem::transmute(device_context.upper_connect_data.class_service) };

        callback(device_object, input_data_start, input_data_end, input_data_consumed);
    }
}

#[link_section = "PAGE"]
unsafe extern "C" fn completion_routine(request: WDFREQUEST, _handle: WDFIOTARGET, params: *mut WDF_REQUEST_COMPLETION_PARAMS, context: WDFCONTEXT) {
    paged_code!();
    
    let params_ioctl = unsafe { &mut (*params).Parameters.Ioctl };
    let params_status = unsafe { (*params).IoStatus.__bindgen_anon_1.Status };

    let buffer = params_ioctl.Output.Buffer;
    let mut status = params_status;

    if nt_success(status) && unsafe { (*params).Type } == WdfRequestTypeDeviceControlInternal && params_ioctl.IoControlCode == KeyboardIoctl::KeyboardQueryAttributes as u32 {
        if params_ioctl.Output.Length >= core::mem::size_of::<KeyboardAttributes>() {
            let device_context: &mut DeviceContext = unsafe { core::mem::transmute(context) };
            status = call_unsafe_wdf_function_binding!(
                WdfMemoryCopyToBuffer,
                buffer,
                params_ioctl.Output.Offset,
                (&mut device_context.keyboard_attributes) as *mut _ as *mut _,
                core::mem::size_of::<KeyboardAttributes>(),
            );
        }
    }

    call_unsafe_wdf_function_binding!(
        WdfRequestComplete,
        request,
        status
    );
}
