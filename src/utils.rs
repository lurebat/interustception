#[macro_export]
macro_rules! kernel_callback {
    (fn $fn_name:ident($($param_name:ident: $param_type:ty),*) -> $ret_type:ty {
        $($body:tt)*
    }) => {
        #[link_section = "PAGE"]
        $vis unsafe extern "C" fn $fn_name($($param_name: $param_type),*) -> $ret_type {
            paged_code!();
            $($body)*
        }
    };
}

#[macro_export]
macro_rules! driver_entry { (($driver:ident, $registry_path:ident) { $($body:tt)* }) => {
        #[link_section = "INIT"]
        #[export_name = "DriverEntry"] // WDF expects a symbol with the name DriverEntry
        extern "system" fn driver_entry(
            $driver: &mut wdk_sys::DRIVER_OBJECT,
            $registry_path: wdk_sys::PCUNICODE_STRING,
        ) -> wdk_sys::NTSTATUS {
            $($body)*
        }
    };
}


#[macro_export]
macro_rules! init_object {
    ($type:ty, { $($field:ident : $value:expr),* $(,)* }) => {{
        let mut object = <$type>::default();
        $(object.$field = $value;)*
        object.Size = core::mem::size_of::<$type>() as ULONG;
        object
    }};
}
