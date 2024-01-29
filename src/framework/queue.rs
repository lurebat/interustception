use core::ptr::null_mut;
use wdk_sys::_WDF_IO_QUEUE_DISPATCH_TYPE::{WdfIoQueueDispatchParallel, WdfIoQueueDispatchSequential};
use wdk_sys::_WDF_TRI_STATE::WdfUseDefault;
use wdk_sys::{BOOLEAN, PFN_WDF_IO_QUEUE_IO_DEVICE_CONTROL, PFN_WDF_IO_QUEUE_IO_INTERNAL_DEVICE_CONTROL, ULONG, WDF_IO_QUEUE_CONFIG, WDF_NO_OBJECT_ATTRIBUTES, WDFDEVICE, WDFQUEUE};
use wdk_sys::macros::call_unsafe_wdf_function_binding;
use crate::framework::{Result, ErrorCode, NtStatusError};
use crate::init_object;

pub struct QueueBuilder {
    pub config: WDF_IO_QUEUE_CONFIG
}

impl QueueBuilder {
    pub fn new() -> Self {
        let mut config = WDF_IO_QUEUE_CONFIG {
            ..init_object!(WDF_IO_QUEUE_CONFIG)
        };
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

    pub fn device_io_control(&mut self, callback: PFN_WDF_IO_QUEUE_IO_DEVICE_CONTROL) -> &mut Self {
        self.config.EvtIoDeviceControl = callback;
        self
    }

    pub fn default_queue(&mut self) -> &mut Self {
        self.config.DefaultQueue = true as BOOLEAN;
        self
    }

    pub fn parallel_dispatch(&mut self) -> &mut Self {
        self.config.DispatchType = WdfIoQueueDispatchParallel;
        unsafe { self.config.Settings.Parallel}.NumberOfPresentedRequests = ULONG::MAX;
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
    pub queue: WDFQUEUE
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
}
