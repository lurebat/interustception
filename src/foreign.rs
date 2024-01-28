use wdk_sys::{GUID, PDEVICE_OBJECT, PVOID};

/*DEFINE_GUID( CLASS_KEYBOARD,            0x4d36e96bL, 0xe325, 0x11ce, 0xbf, 0xc1, 0x08, 0x00, 0x2b, 0xe1, 0x03, 0x18 );*/
pub static GUID_CLASS_KEYBOARD: GUID = GUID {
    Data1: 0x4d36_e96bu64 as u32,
    Data2: 0xe325,
    Data3: 0x11ce,
    Data4: [0xbf, 0xc1, 0x08, 0x00, 0x2b, 0xe1, 0x03, 0x18],
};

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct KeyboardTypematicParameters {
    pub unit_id: u16,
    pub rate: u16,
    pub delay: u16,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct KeyboardId {
    pub r#type: u8,
    pub subtype: u8,
}


#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct KeyboardInputData {
    pub unit_id: u16,
    pub make_code: u16,
    pub flags: u16,
    pub reserved: u16,
    pub extra_information: u32,
}


#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct KeyboardAttributes {
    pub keyboard_identifier: KeyboardId,
    pub keyboard_mode: u16,
    pub number_of_function_keys: u16,
    pub number_of_indicators: u16,
    pub number_of_keys_total: u16,
    pub input_data_queue_length: u32,
    pub key_repeat_minimum: KeyboardTypematicParameters,
    pub key_repeat_maximum: KeyboardTypematicParameters,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct ConnectData {
    pub class_device_object: PDEVICE_OBJECT,
    pub class_service: PVOID,
}
