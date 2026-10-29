//! Battery level of a wireless headset.
//!
//! Vendors expose it as a HID feature report on the USB receiver, not on the
//! audio endpoint. The receiver and the endpoint share a PnP container, so the
//! report is looked up by that container instead of by vendor/product ids:
//! any wireless model that follows the same convention shows up, and wired
//! headsets simply report nothing.

use crate::models::BatteryInfo;

pub fn read(output_name: &str) -> Option<BatteryInfo> {
    imp::read(output_name)
}

#[cfg(windows)]
mod imp {
    use super::BatteryInfo;
    use crate::audio::win_endpoint::Session;
    use std::ffi::c_void;
    use std::mem;
    use std::ptr;
    use windows_sys::core::GUID;
    use windows_sys::Win32::Devices::DeviceAndDriverInstallation::{
        SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces, SetupDiGetClassDevsW,
        SetupDiGetDeviceInterfaceDetailW, SetupDiGetDevicePropertyW, DIGCF_DEVICEINTERFACE,
        DIGCF_PRESENT, HDEVINFO, SP_DEVICE_INTERFACE_DATA, SP_DEVICE_INTERFACE_DETAIL_DATA_W,
        SP_DEVINFO_DATA,
    };
    use windows_sys::Win32::Devices::HumanInterfaceDevice::HidD_GetFeature;
    use windows_sys::Win32::Devices::Properties::{DEVPKEY_Device_ContainerId, DEVPROP_TYPE_GUID};
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
    };

    const HID_INTERFACE: GUID = GUID::from_u128(0x4d1e55b2_f16f_11cf_88cb_001111000030);
    /// Vendor report whose second byte is the charge percentage.
    const REPORT_BATTERY: u8 = 0x49;

    pub fn read(output_name: &str) -> Option<BatteryInfo> {
        let session = Session::new().ok()?;
        let container = session.container_by_name(output_name).ok().flatten()?;
        let percent = probe_container(&container)?;
        Some(BatteryInfo {
            percent,
            device: output_name.to_string(),
        })
    }

    fn probe_container(container: &GUID) -> Option<u8> {
        for path in interface_paths(container) {
            if let Some(percent) = unsafe { probe(&path) } {
                return Some(percent);
            }
        }
        None
    }

    struct DeviceSet(HDEVINFO);

    impl Drop for DeviceSet {
        fn drop(&mut self) {
            unsafe { SetupDiDestroyDeviceInfoList(self.0) };
        }
    }

    /// Wide paths of every present HID interface that belongs to the same PnP
    /// container as the playback endpoint.
    fn interface_paths(container: &GUID) -> Vec<Vec<u16>> {
        let mut paths = Vec::new();
        unsafe {
            let raw = SetupDiGetClassDevsW(
                &HID_INTERFACE,
                ptr::null(),
                ptr::null_mut(),
                DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
            );
            if raw == -1isize {
                return paths;
            }
            let set = DeviceSet(raw);
            let mut index = 0;
            loop {
                let mut interface: SP_DEVICE_INTERFACE_DATA = mem::zeroed();
                interface.cbSize = mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32;
                if SetupDiEnumDeviceInterfaces(
                    set.0,
                    ptr::null(),
                    &HID_INTERFACE,
                    index,
                    &mut interface,
                ) == 0
                {
                    break;
                }
                index += 1;

                let mut required = 0u32;
                SetupDiGetDeviceInterfaceDetailW(
                    set.0,
                    &interface,
                    ptr::null_mut(),
                    0,
                    &mut required,
                    ptr::null_mut(),
                );
                if required < 6 {
                    continue;
                }
                // cbSize must be the struct size (8 on x64, 6 on x86); the
                // path itself always starts right after the u32.
                let mut buffer = vec![0u8; required as usize];
                (buffer.as_mut_ptr() as *mut u32)
                    .write(mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32);
                let mut info: SP_DEVINFO_DATA = mem::zeroed();
                info.cbSize = mem::size_of::<SP_DEVINFO_DATA>() as u32;
                let ok = SetupDiGetDeviceInterfaceDetailW(
                    set.0,
                    &interface,
                    buffer.as_mut_ptr() as *mut SP_DEVICE_INTERFACE_DETAIL_DATA_W,
                    required,
                    &mut required,
                    &mut info,
                );
                if ok == 0 || !same_container(set.0, &info, container) {
                    continue;
                }
                let start = buffer.as_ptr().add(4) as *const u16;
                let mut len = 0usize;
                while *start.add(len) != 0 {
                    len += 1;
                }
                paths.push(std::slice::from_raw_parts(start, len + 1).to_vec());
            }
        }
        paths
    }

    unsafe fn same_container(set: HDEVINFO, info: &SP_DEVINFO_DATA, container: &GUID) -> bool {
        let mut kind = 0u32;
        let mut value = [0u8; mem::size_of::<GUID>()];
        let mut required = 0u32;
        let ok = SetupDiGetDevicePropertyW(
            set,
            info,
            &DEVPKEY_Device_ContainerId,
            &mut kind,
            value.as_mut_ptr(),
            value.len() as u32,
            &mut required,
            0,
        );
        if ok == 0 || kind != DEVPROP_TYPE_GUID {
            return false;
        }
        let candidate = ptr::read_unaligned(value.as_ptr() as *const GUID);
        candidate.data1 == container.data1
            && candidate.data2 == container.data2
            && candidate.data3 == container.data3
            && candidate.data4 == container.data4
    }

    unsafe fn probe(path: &[u16]) -> Option<u8> {
        // Access 0 is enough for feature reports and keeps the vendor app
        // running side by side with us.
        let handle = CreateFileW(
            path.as_ptr(),
            0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null(),
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        );
        if handle == INVALID_HANDLE_VALUE {
            return None;
        }

        let mut buffer = [0u8; 65];
        let battery = feature(handle, REPORT_BATTERY, &mut buffer);
        CloseHandle(handle);
        let battery = battery?;
        if battery.len() < 2 || battery[1] > 100 {
            return None;
        }
        Some(battery[1])
    }

    unsafe fn feature(handle: HANDLE, report: u8, buffer: &mut [u8; 65]) -> Option<Vec<u8>> {
        // HID drivers only accept the exact feature report length, and models
        // in the wild declare anything from 2 to 65 bytes.
        for len in [2usize, 3, 4, 8, 16, 32, 64, 65] {
            buffer.fill(0);
            buffer[0] = report;
            if HidD_GetFeature(handle, buffer.as_mut_ptr() as *mut c_void, len as u32) != 0 {
                return Some(buffer[..len].to_vec());
            }
        }
        None
    }
}

#[cfg(not(windows))]
mod imp {
    use crate::models::BatteryInfo;

    pub fn read(_output_name: &str) -> Option<BatteryInfo> {
        None
    }
}
