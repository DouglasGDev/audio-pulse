//! Thin COM access to Windows playback endpoints: identity, master volume,
//! mute and the container id used to find a headset's HID interface.

#![cfg(windows)]

use std::ffi::c_void;
use std::mem;
use std::ptr;
use windows_sys::core::{GUID, HRESULT, PCWSTR, PWSTR};
use windows_sys::Win32::Media::Audio::MMDeviceEnumerator;
use windows_sys::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED,
};

const IID_IMM_DEVICE_ENUMERATOR: GUID =
    GUID::from_u128(0xa95664d2_9614_4f35_a746_de8db63617e6);
const IID_IAUDIO_ENDPOINT_VOLUME: GUID =
    GUID::from_u128(0x5cdf2c82_841e_4546_9722_0cf74078229a);
const CLSID_POLICY_CONFIG_CLIENT: GUID =
    GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);
const IID_IPOLICY_CONFIG: GUID = GUID::from_u128(0xf8679f50_850a_41cf_9c72_430f290290c8);

const E_RENDER: i32 = 0;
const E_CONSOLE: i32 = 0;
const ROLES: [i32; 3] = [0, 1, 2];
const DEVICE_STATE_ACTIVE: u32 = 0x1;
const STGM_READ: u32 = 0;

const VT_LPWSTR: u16 = 31;
const VT_CLSID: u16 = 72;

// PKEY_Device_FriendlyName / PKEY_Device_DeviceDesc / PKEY_Device_ContainerId.
const FMTID_DEVICE: GUID = GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0);
const PKEY_FRIENDLY_NAME_PID: u32 = 14;
const PKEY_DEVICE_DESC_PID: u32 = 2;
const FMTID_CONTAINER: GUID = GUID::from_u128(0x8c7ed206_3f8a_4827_b3ab_ae9e1faefc6c);
const PKEY_CONTAINER_PID: u32 = 2;

#[repr(C)]
struct PropertyKey {
    fmtid: GUID,
    pid: u32,
}

#[repr(C)]
#[derive(Default)]
struct PropVariant {
    vt: u16,
    reserved: [u16; 3],
    data: [usize; 2],
}

struct ComApartment;

thread_local! {
    /// COM is initialized once per thread and never torn down: interfaces
    /// created on the thread outlive individual calls, and uninitializing
    /// while one of them is still alive is undefined behavior.
    static COM_APARTMENT: ComApartment = ComApartment::enter();
}

impl ComApartment {
    fn enter() -> Self {
        let hr = unsafe { CoInitializeEx(ptr::null(), COINIT_MULTITHREADED as u32) };
        if hr < 0 && hr != 0x8001_0106u32 as HRESULT {
            log::warn!("CoInitializeEx failed: 0x{:08X}", hr as u32);
        }
        Self
    }

    fn ensure() {
        COM_APARTMENT.with(|_| ());
    }
}

pub struct ComPtr(*mut c_void);

impl Drop for ComPtr {
    fn drop(&mut self) {
        if self.0.is_null() {
            return;
        }
        unsafe {
            let release: unsafe extern "system" fn(*mut c_void) -> u32 = transmute_slot(self.0, 2);
            release(self.0);
        }
    }
}

unsafe fn transmute_slot<T: Copy>(this: *mut c_void, index: usize) -> T {
    let vtable = *(this as *const *const *const c_void);
    mem::transmute_copy(&*vtable.add(index))
}

fn check(hr: HRESULT, context: &str) -> Result<(), String> {
    if hr >= 0 {
        Ok(())
    } else {
        Err(format!("{context} (0x{:08X})", hr as u32))
    }
}

#[derive(Debug, Clone)]
pub struct Endpoint {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy)]
pub struct VolumeState {
    pub scalar: f32,
    pub muted: bool,
}

#[derive(Debug, Clone)]
pub struct DefaultState {
    pub endpoint: Endpoint,
    pub volume: VolumeState,
}

pub struct Session {
    enumerator: ComPtr,
}

impl Session {
    pub fn new() -> Result<Self, String> {
        ComApartment::ensure();
        let enumerator = unsafe { create_enumerator()? };
        Ok(Self { enumerator })
    }

    pub fn default_endpoint(&self) -> Result<Endpoint, String> {
        unsafe {
            let device = default_device(self.enumerator.0)?;
            endpoint_info(device.0)
                .ok_or_else(|| "Não foi possível identificar a saída de áudio padrão.".to_string())
        }
    }

    pub fn default_state(&self) -> Result<DefaultState, String> {
        unsafe {
            let device = default_device(self.enumerator.0)?;
            let endpoint = endpoint_info(device.0)
                .ok_or_else(|| "Não foi possível identificar a saída de áudio padrão.".to_string())?;
            let volume = volume_state(device.0).unwrap_or(VolumeState {
                scalar: 1.0,
                muted: false,
            });
            Ok(DefaultState { endpoint, volume })
        }
    }

    pub fn endpoint_by_name(&self, name: &str) -> Result<Option<Endpoint>, String> {
        unsafe {
            let Some(device) = find_device_by_name(self.enumerator.0, name)? else {
                return Ok(None);
            };
            Ok(endpoint_info(device.0))
        }
    }

    pub fn endpoint_by_id(&self, id: &str) -> Option<Endpoint> {
        unsafe {
            let device = device_by_id(self.enumerator.0, id)?;
            endpoint_info(device.0)
        }
    }

    pub fn state_by_name(&self, name: &str) -> Result<Option<VolumeState>, String> {
        unsafe {
            let Some(device) = find_device_by_name(self.enumerator.0, name)? else {
                return Ok(None);
            };
            Ok(volume_state(device.0))
        }
    }

    pub fn apply_default_volume(
        &self,
        scalar: Option<f32>,
        mute: Option<bool>,
    ) -> Result<(), String> {
        unsafe {
            let device = default_device(self.enumerator.0)?;
            apply_volume(device.0, scalar, mute)
        }
    }

    /// Returns `false` when the endpoint is not present.
    pub fn apply_volume_by_name(
        &self,
        name: &str,
        scalar: Option<f32>,
        mute: Option<bool>,
    ) -> Result<bool, String> {
        unsafe {
            let Some(device) = find_device_by_name(self.enumerator.0, name)? else {
                return Ok(false);
            };
            apply_volume(device.0, scalar, mute)?;
            Ok(true)
        }
    }

    pub fn apply_volume_by_id(
        &self,
        id: &str,
        scalar: Option<f32>,
        mute: Option<bool>,
    ) -> Result<bool, String> {
        unsafe {
            let Some(device) = device_by_id(self.enumerator.0, id) else {
                return Ok(false);
            };
            apply_volume(device.0, scalar, mute)?;
            Ok(true)
        }
    }

    pub fn container_by_name(&self, name: &str) -> Result<Option<GUID>, String> {
        unsafe {
            let Some(device) = find_device_by_name(self.enumerator.0, name)? else {
                return Ok(None);
            };
            Ok(container_id(device.0))
        }
    }

    pub fn is_default(&self, id: &str) -> bool {
        unsafe {
            default_device(self.enumerator.0)
                .ok()
                .and_then(|device| endpoint_info(device.0))
                .map(|endpoint| endpoint.id == id)
                .unwrap_or(false)
        }
    }

    pub fn switch_default(&self, id: &str) -> Result<(), String> {
        unsafe {
            let policy = create_policy()?;
            // eConsole must succeed; the other roles are best effort on systems
            // that keep a separate communications endpoint.
            set_default(policy.0, id, ROLES[0])?;
            for role in &ROLES[1..] {
                if let Err(err) = set_default(policy.0, id, *role) {
                    log::warn!("{err}");
                }
            }
            if self.is_default(id) {
                Ok(())
            } else {
                Err("O Windows não confirmou a troca de saída padrão.".into())
            }
        }
    }
}

unsafe fn create_enumerator() -> Result<ComPtr, String> {
    let mut raw: *mut c_void = ptr::null_mut();
    let hr = CoCreateInstance(
        &MMDeviceEnumerator,
        ptr::null_mut(),
        CLSCTX_ALL,
        &IID_IMM_DEVICE_ENUMERATOR,
        &mut raw,
    );
    check(hr, "Falha ao consultar os dispositivos de áudio do Windows")?;
    if raw.is_null() {
        return Err("Falha ao consultar os dispositivos de áudio do Windows.".into());
    }
    Ok(ComPtr(raw))
}

unsafe fn create_policy() -> Result<ComPtr, String> {
    let mut raw: *mut c_void = ptr::null_mut();
    let hr = CoCreateInstance(
        &CLSID_POLICY_CONFIG_CLIENT,
        ptr::null_mut(),
        CLSCTX_ALL,
        &IID_IPOLICY_CONFIG,
        &mut raw,
    );
    check(hr, "O Windows recusou a troca de saída padrão")?;
    if raw.is_null() {
        return Err("O Windows recusou a troca de saída padrão.".into());
    }
    Ok(ComPtr(raw))
}

unsafe fn read_wide(ptr: *const u16) -> String {
    let mut len = 0usize;
    while *ptr.add(len) != 0 {
        len += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(ptr, len))
}

unsafe fn opens_store(device: *mut c_void) -> Option<ComPtr> {
    let open_store: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> HRESULT =
        transmute_slot(device, 4);
    let mut raw: *mut c_void = ptr::null_mut();
    if open_store(device, STGM_READ, &mut raw) < 0 || raw.is_null() {
        return None;
    }
    Some(ComPtr(raw))
}

unsafe fn get_property(store: *mut c_void, key: &PropertyKey) -> Option<PropVariant> {
    let get_value: unsafe extern "system" fn(
        *mut c_void,
        *const PropertyKey,
        *mut PropVariant,
    ) -> HRESULT = transmute_slot(store, 5);
    let mut value = PropVariant::default();
    if get_value(store, key, &mut value) < 0 {
        return None;
    }
    Some(value)
}

unsafe fn friendly_name(device: *mut c_void) -> Option<String> {
    let store = opens_store(device)?;
    for pid in [PKEY_FRIENDLY_NAME_PID, PKEY_DEVICE_DESC_PID] {
        let key = PropertyKey {
            fmtid: FMTID_DEVICE,
            pid,
        };
        let Some(value) = get_property(store.0, &key) else {
            continue;
        };
        if value.vt != VT_LPWSTR || value.data[0] == 0 {
            continue;
        }
        let name = read_wide(value.data[0] as *const u16).trim().to_string();
        CoTaskMemFree(value.data[0] as *const c_void);
        if !name.is_empty() {
            return Some(name);
        }
    }
    None
}

unsafe fn device_id(device: *mut c_void) -> Option<String> {
    let get_id: unsafe extern "system" fn(*mut c_void, *mut PWSTR) -> HRESULT =
        transmute_slot(device, 5);
    let mut raw = ptr::null_mut();
    if get_id(device, &mut raw) < 0 || raw.is_null() {
        return None;
    }
    let id = read_wide(raw as *const u16);
    CoTaskMemFree(raw as *const c_void);
    Some(id)
}

unsafe fn endpoint_info(device: *mut c_void) -> Option<Endpoint> {
    Some(Endpoint {
        id: device_id(device)?,
        name: friendly_name(device)?,
    })
}

unsafe fn container_id(device: *mut c_void) -> Option<GUID> {
    let store = opens_store(device)?;
    let key = PropertyKey {
        fmtid: FMTID_CONTAINER,
        pid: PKEY_CONTAINER_PID,
    };
    let value = get_property(store.0, &key)?;
    if value.vt != VT_CLSID || value.data[0] == 0 {
        return None;
    }
    let guid = *(value.data[0] as *const GUID);
    CoTaskMemFree(value.data[0] as *const c_void);
    Some(guid)
}

unsafe fn default_device(enumerator: *mut c_void) -> Result<ComPtr, String> {
    let get_default: unsafe extern "system" fn(
        *mut c_void,
        i32,
        i32,
        *mut *mut c_void,
    ) -> HRESULT = transmute_slot(enumerator, 4);
    let mut device: *mut c_void = ptr::null_mut();
    check(
        get_default(enumerator, E_RENDER, E_CONSOLE, &mut device),
        "Nenhuma saída de áudio padrão encontrada",
    )?;
    if device.is_null() {
        return Err("Nenhuma saída de áudio padrão encontrada.".into());
    }
    Ok(ComPtr(device))
}

unsafe fn find_device_by_name(
    enumerator: *mut c_void,
    target: &str,
) -> Result<Option<ComPtr>, String> {
    let enum_endpoints: unsafe extern "system" fn(
        *mut c_void,
        i32,
        u32,
        *mut *mut c_void,
    ) -> HRESULT = transmute_slot(enumerator, 3);
    let mut collection: *mut c_void = ptr::null_mut();
    check(
        enum_endpoints(
            enumerator,
            E_RENDER,
            DEVICE_STATE_ACTIVE,
            &mut collection,
        ),
        "Falha ao listar as saídas de áudio",
    )?;
    if collection.is_null() {
        return Ok(None);
    }
    let collection = ComPtr(collection);
    let get_count: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT =
        transmute_slot(collection.0, 3);
    let get_item: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> HRESULT =
        transmute_slot(collection.0, 4);
    let mut count = 0u32;
    check(
        get_count(collection.0, &mut count),
        "Falha ao listar as saídas de áudio",
    )?;

    let target_lower = target.to_lowercase();
    let mut fallback: Option<ComPtr> = None;
    for index in 0..count {
        let mut raw: *mut c_void = ptr::null_mut();
        if get_item(collection.0, index, &mut raw) < 0 || raw.is_null() {
            continue;
        }
        let device = ComPtr(raw);
        let Some(name) = friendly_name(device.0) else {
            continue;
        };
        let name_lower = name.to_lowercase();
        if name_lower == target_lower {
            return Ok(Some(device));
        }
        if fallback.is_none()
            && target_lower.contains("cable input")
            && name_lower.contains("cable input")
        {
            fallback = Some(device);
        }
    }
    Ok(fallback)
}

unsafe fn device_by_id(enumerator: *mut c_void, id: &str) -> Option<ComPtr> {
    let wide: Vec<u16> = id.encode_utf16().chain(std::iter::once(0)).collect();
    let get_device: unsafe extern "system" fn(*mut c_void, PCWSTR, *mut *mut c_void) -> HRESULT =
        transmute_slot(enumerator, 5);
    let mut raw: *mut c_void = ptr::null_mut();
    if get_device(enumerator, wide.as_ptr(), &mut raw) < 0 || raw.is_null() {
        return None;
    }
    Some(ComPtr(raw))
}

unsafe fn activate_volume(device: *mut c_void) -> Result<ComPtr, String> {
    let activate: unsafe extern "system" fn(
        *mut c_void,
        *const GUID,
        u32,
        *const c_void,
        *mut *mut c_void,
    ) -> HRESULT = transmute_slot(device, 3);
    let mut raw: *mut c_void = ptr::null_mut();
    check(
        activate(
            device,
            &IID_IAUDIO_ENDPOINT_VOLUME,
            CLSCTX_ALL,
            ptr::null(),
            &mut raw,
        ),
        "Falha ao acessar o volume da saída de áudio",
    )?;
    if raw.is_null() {
        return Err("Falha ao acessar o volume da saída de áudio.".into());
    }
    Ok(ComPtr(raw))
}

unsafe fn volume_state(device: *mut c_void) -> Option<VolumeState> {
    let volume = activate_volume(device).ok()?;
    // IAudioEndpointVolume vtable after IUnknown: 5 GetChannelCount,
    // 6..9 master level (scalar getter at 9), 10..13 per-channel,
    // 14 SetMute, 15 GetMute.
    let get_scalar: unsafe extern "system" fn(*mut c_void, *mut f32) -> HRESULT =
        transmute_slot(volume.0, 9);
    let get_mute: unsafe extern "system" fn(*mut c_void, *mut i32) -> HRESULT =
        transmute_slot(volume.0, 15);
    let mut scalar = 1.0f32;
    if get_scalar(volume.0, &mut scalar) < 0 {
        return None;
    }
    let mut muted = 0i32;
    let muted_ok = get_mute(volume.0, &mut muted) >= 0;
    Some(VolumeState {
        scalar: scalar.clamp(0.0, 1.0),
        muted: muted_ok && muted != 0,
    })
}

unsafe fn apply_volume(
    device: *mut c_void,
    scalar: Option<f32>,
    mute: Option<bool>,
) -> Result<(), String> {
    let volume = activate_volume(device)?;
    let context = GUID::from_u128(0);
    if let Some(scalar) = scalar {
        let set_scalar: unsafe extern "system" fn(
            *mut c_void,
            f32,
            *const GUID,
        ) -> HRESULT = transmute_slot(volume.0, 7);
        check(
            set_scalar(volume.0, scalar.clamp(0.0, 1.0), &context),
            "Falha ao ajustar o volume da saída de áudio",
        )?;
    }
    if let Some(muted) = mute {
        let set_mute: unsafe extern "system" fn(*mut c_void, i32, *const GUID) -> HRESULT =
            transmute_slot(volume.0, 14);
        check(
            set_mute(volume.0, i32::from(muted), &context),
            "Falha ao ajustar o mudo da saída de áudio",
        )?;
    }
    Ok(())
}

unsafe fn set_default(policy: *mut c_void, id: &str, role: i32) -> Result<(), String> {
    let wide: Vec<u16> = id.encode_utf16().chain(std::iter::once(0)).collect();
    let set_endpoint: unsafe extern "system" fn(*mut c_void, PCWSTR, i32) -> HRESULT =
        transmute_slot(policy, 13);
    check(
        set_endpoint(policy, wide.as_ptr(), role),
        "Falha ao trocar a saída padrão do Windows",
    )
}
