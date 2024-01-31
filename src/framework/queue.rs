use core::ptr::null_mut;
use wdk_sys::_WDF_IO_QUEUE_DISPATCH_TYPE::{WdfIoQueueDispatchParallel, WdfIoQueueDispatchSequential};
use wdk_sys::_WDF_TRI_STATE::WdfUseDefault;
use wdk_sys::{BOOLEAN, NTSTATUS, PFN_WDF_IO_QUEUE_IO_DEVICE_CONTROL, PFN_WDF_IO_QUEUE_IO_INTERNAL_DEVICE_CONTROL, PFN_WDF_REQUEST_COMPLETION_ROUTINE, PVOID, ULONG, WDF_IO_QUEUE_CONFIG, WDF_NO_HANDLE, WDF_NO_OBJECT_ATTRIBUTES, WDF_REQUEST_SEND_OPTIONS, WDFDEVICE, WDFIOTARGET, WDFMEMORY, WDFQUEUE, WDFREQUEST, WDFREQUEST__};
use wdk_sys::macros::call_unsafe_wdf_function_binding;
use crate::foreign::ConnectData;
use crate::framework::{Result, ErrorCode, NtStatusError, Device, Context};
use crate::init_object;

pub struct QueueBuilder {
    pub config: WDF_IO_QUEUE_CONFIG,
}

impl QueueBuilder {
    pub fn new() -> Self {
        let mut config = init_object!(WDF_IO_QUEUE_CONFIG);
        config.PowerManaged = WdfUseDefault;
        config.DispatchType = WdfIoQueueDispatchSequential;

        Self {
            config
        }
    }

    pub fn internal_device_control(&mut self, callback: PFN_WDF_IO_QUEUE_IO_INTERNAL_DEVICE_CONTROL) -> &mut Self {
        self.config.EvtIoInternalDeviceControl = callback;
        self
    }

    pub fn device_control(&mut self, callback: PFN_WDF_IO_QUEUE_IO_DEVICE_CONTROL) -> &mut Self {
        self.config.EvtIoDeviceControl = callback;
        self
    }

    pub fn default_queue(&mut self) -> &mut Self {
        self.config.DefaultQueue = 1;
        self
    }

    pub fn parallel_dispatch(&mut self) -> &mut Self {
        self.config.DispatchType = WdfIoQueueDispatchParallel;
        unsafe { self.config.Settings.Parallel }.NumberOfPresentedRequests = ULONG::MAX;
        self
    }

    pub fn create(&mut self, device: WDFDEVICE) -> Result<Queue> {
        let mut queue_handle = null_mut() as WDFQUEUE;
        unsafe {
            call_unsafe_wdf_function_binding!(
                WdfIoQueueCreate,
                device,
                &mut self.config,
                WDF_NO_OBJECT_ATTRIBUTES,
                &mut queue_handle,
            )
        }.check_status(ErrorCode::QueueCreationFailed).map(|_| {
            Queue::new(queue_handle)
        })
    }
}

pub struct Queue {
    pub queue: WDFQUEUE,
}

impl Queue {
    pub fn new(queue: WDFQUEUE) -> Self {
        Self {
            queue
        }
    }

    pub fn handle(&self) -> WDFQUEUE {
        self.queue
    }

    pub fn get_device<T: Context>(&self) -> Device<T> {
        let device = unsafe {call_unsafe_wdf_function_binding!(
        WdfIoQueueGetDevice,
        self.handle()
    )};

        Device::<T>::new(unsafe { device.as_mut().expect("Device can't be null") })
    }
}

pub struct Request<'a> {
    handle: &'a mut WDFREQUEST__,
}

impl<'a> Request<'a> {
    pub fn new(handle: &'a mut WDFREQUEST__) -> Self {
        Self {
            handle
        }
    }

    pub fn complete(&mut self, status: NTSTATUS) {
        unsafe {
            call_unsafe_wdf_function_binding!(
                WdfRequestComplete,
                self.handle,
                status
            )
        };
    }

    pub fn output_memory(&mut self) -> Result<WDFMEMORY> {
        let mut output_memory = null_mut();
        unsafe {
            call_unsafe_wdf_function_binding!(
                WdfRequestRetrieveOutputMemory,
                self.handle,
                &mut output_memory,
            )
        }.check_status(ErrorCode::RequestOutputMemoryRetrievalFailed).map(|_| output_memory)
    }

    pub fn format_for_internal_ioctl(&mut self, io_target: WDFIOTARGET, io_control_code: u32, output_memory: WDFMEMORY) -> Result<()> {
        unsafe {
            call_unsafe_wdf_function_binding!(
                WdfIoTargetFormatRequestForInternalIoctl,
                io_target,
                self.handle,
                io_control_code,
                null_mut(),
                null_mut(),
                output_memory,
                null_mut(),)
        }.check_status(ErrorCode::RequestFormatForInternalIoctlFailed).map(|_| ())
    }

    pub fn set_completion_callback(&mut self, callback: PFN_WDF_REQUEST_COMPLETION_ROUTINE, context: PVOID) {
        unsafe {
            call_unsafe_wdf_function_binding!(
                WdfRequestSetCompletionRoutine,
                self.handle,
                callback,
                context,
            );
        }
    }

    pub fn send(&mut self, io_target: WDFIOTARGET, flags: u32) -> Result<()> {
        let mut options = init_object!(WDF_REQUEST_SEND_OPTIONS);
        options.Flags = flags;

        let res = unsafe {
            call_unsafe_wdf_function_binding!(
                WdfRequestSend,
                self.handle,
                io_target,
                &mut options as *mut _ as *mut _,
            )
        };

        if res == 0 {
            unsafe {
                call_unsafe_wdf_function_binding!(
                    WdfRequestGetStatus,
                    self.handle
                )
            }.check_status(ErrorCode::RequestSendFailed).map(|_| ())
        } else {
            Ok(())
        }
    }
}


pub struct KeyboardConnectRequest<'a> {
    handle: &'a mut WDFREQUEST__,
}

impl<'a> KeyboardConnectRequest<'a> {
    pub fn new(handle: &'a mut WDFREQUEST__) -> Self {
        Self {
            handle
        }
    }

    pub fn connect_data(&mut self) -> Result<ConnectData> {
        let mut connect_data = ConnectData::default();
        let mut length = 0usize;
        unsafe {
            call_unsafe_wdf_function_binding!(
                        WdfRequestRetrieveInputBuffer,
                        self.handle,
                        core::mem::size_of::<ConnectData>(),
                        core::ptr::addr_of_mut!(connect_data).cast(),
                        &mut length,
                        )
        }.check_status(ErrorCode::KeyboardConnectRequestRetrievalFailed).map(|_| {
            let x = connect_data;
            x
        })
    }
}
