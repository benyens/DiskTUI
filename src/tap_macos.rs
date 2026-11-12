//! Tap de Core Audio por proceso: captura solo el audio de una app (macOS 14.4+).
//!
//! Pasos: buscar los procesos de audio cuyo bundle id empieza con el prefijo pedido, crear
//! un `CATapDescription` con esos procesos, y envolver el tap en un dispositivo agregado
//! privado. Ese dispositivo aparece como una entrada normal y `cpal` lo lee.
//!
//! La construcción del dispositivo agregado sigue la del loopback de cpal
//! (`src/host/coreaudio/macos/loopback.rs`), que captura todos los procesos.
//!
//! AVISO: este módulo se escribió sin poder ejecutarlo. `disktui --diagnostico` prueba cada
//! variante y dice cuál entrega audio. `--global` lo evita; `--no-default-features` compila
//! sin él.

use std::ffi::{CStr, c_void};
use std::ptr::NonNull;

use objc2::AnyThread;
use objc2::rc::Retained;
use objc2_core_audio::{
    AudioHardwareCreateAggregateDevice, AudioHardwareCreateProcessTap, AudioHardwareDestroyAggregateDevice,
    AudioHardwareDestroyProcessTap, AudioObjectGetPropertyData, AudioObjectGetPropertyDataSize, AudioObjectID,
    AudioObjectPropertyAddress, AudioObjectPropertySelector, CATapDescription, CATapMuteBehavior,
    kAudioAggregateDeviceIsStackedKey, kAudioAggregateDeviceMainSubDeviceKey, kAudioAggregateDeviceNameKey,
    kAudioAggregateDeviceSubDeviceListKey, kAudioAggregateDeviceTapAutoStartKey, kAudioAggregateDeviceTapListKey,
    kAudioAggregateDeviceUIDKey, kAudioDevicePropertyDeviceUID, kAudioEndPointDeviceIsPrivateKey,
    kAudioHardwarePropertyDefaultOutputDevice, kAudioHardwarePropertyProcessObjectList,
    kAudioObjectPropertyElementMain, kAudioObjectPropertyScopeGlobal, kAudioObjectSystemObject,
    kAudioProcessPropertyBundleID, kAudioSubDeviceUIDKey, kAudioSubTapDriftCompensationKey, kAudioSubTapUIDKey,
};
use objc2_core_foundation::{
    CFArray, CFDictionary, CFMutableDictionary, CFRetained, CFString, kCFAllocatorDefault, kCFTypeArrayCallBacks,
    kCFTypeDictionaryKeyCallBacks, kCFTypeDictionaryValueCallBacks,
};
use objc2_foundation::{NSArray, NSNumber, NSString};

/// Cómo se arma el dispositivo agregado que envuelve al tap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// Solo el tap (como el loopback de cpal).
    TapOnly,
    /// El tap más el dispositivo de salida como sub-dispositivo principal, que le da reloj
    /// (como el ejemplo AudioCap). En algunos macOS es el único que entrega audio.
    WithOutput,
}

/// Un tap activo. Al soltarlo se destruyen el dispositivo agregado y el tap.
pub struct ProcessTap {
    tap_id: AudioObjectID,
    aggregate_id: AudioObjectID,
    /// UID del dispositivo agregado: con esto `cpal` lo encuentra.
    pub device_uid: String,
}

impl ProcessTap {
    /// Crea un tap sobre todos los procesos de audio cuyo bundle id empieza con `prefix`
    /// (por ejemplo `com.spotify.client`, que incluye sus procesos auxiliares).
    pub fn for_bundle_prefix(prefix: &str, layout: Layout) -> Result<Self, String> {
        let processes: Vec<AudioObjectID> = audio_processes()?
            .into_iter()
            .filter(|id| bundle_id(*id).is_some_and(|b| b.starts_with(prefix)))
            .collect();
        if processes.is_empty() {
            return Err(format!("la app {prefix} no está abierta o aún no usa audio"));
        }

        let pid = std::process::id();
        let numbers: Vec<Retained<NSNumber>> = processes.iter().map(|id| NSNumber::new_u32(*id)).collect();
        let list = NSArray::from_retained_slice(&numbers);

        // 1. Descripción del tap: mezcla estéreo de esos procesos, sin silenciarlos.
        let description = unsafe {
            let d = CATapDescription::initStereoMixdownOfProcesses(CATapDescription::alloc(), &list);
            d.setMuteBehavior(CATapMuteBehavior::Unmuted);
            d.setName(&NSString::from_str(&format!("disktui tap {pid}")));
            d.setPrivate(true);
            d
        };

        // 2. Crear el tap.
        let mut tap_id: AudioObjectID = 0;
        let status = unsafe { AudioHardwareCreateProcessTap(Some(&description), &mut tap_id) };
        if status != 0 {
            return Err(format!("AudioHardwareCreateProcessTap falló (OSStatus {status})"));
        }
        let tap_uid = unsafe { description.UUID().UUIDString() };

        // 3. Dispositivo agregado privado que contiene el tap.
        // UID único por intento: se pueden crear varios taps seguidos en un mismo proceso.
        static COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let device_uid = format!("cl.disktui.tap.{pid}.{n}");
        let output_uid = match layout {
            Layout::TapOnly => None,
            Layout::WithOutput => match default_output_uid() {
                Ok(uid) => Some(uid),
                Err(e) => {
                    unsafe { AudioHardwareDestroyProcessTap(tap_id) };
                    return Err(e);
                }
            },
        };
        let properties =
            aggregate_properties(&tap_uid, &device_uid, &format!("DiskTUI tap {pid}.{n}"), output_uid.as_deref());
        let mut aggregate_id: AudioObjectID = 0;
        let status =
            unsafe { AudioHardwareCreateAggregateDevice(properties.as_ref(), NonNull::from(&mut aggregate_id)) };
        if status != 0 {
            unsafe { AudioHardwareDestroyProcessTap(tap_id) };
            return Err(format!("AudioHardwareCreateAggregateDevice falló (OSStatus {status})"));
        }

        Ok(ProcessTap { tap_id, aggregate_id, device_uid })
    }
}

impl Drop for ProcessTap {
    fn drop(&mut self) {
        unsafe {
            let _ = AudioHardwareDestroyAggregateDevice(self.aggregate_id);
            let _ = AudioHardwareDestroyProcessTap(self.tap_id);
        }
    }
}

fn address(selector: AudioObjectPropertySelector) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    }
}

/// Objetos de proceso que Core Audio conoce (apps que han abierto audio).
fn audio_processes() -> Result<Vec<AudioObjectID>, String> {
    let system = kAudioObjectSystemObject as AudioObjectID;
    let addr = address(kAudioHardwarePropertyProcessObjectList);
    let mut size: u32 = 0;
    let status = unsafe {
        AudioObjectGetPropertyDataSize(system, NonNull::from(&addr), 0, std::ptr::null(), NonNull::from(&mut size))
    };
    if status != 0 {
        return Err(format!("no se pudo listar procesos de audio (OSStatus {status})"));
    }
    let count = size as usize / std::mem::size_of::<AudioObjectID>();
    let mut ids: Vec<AudioObjectID> = vec![0; count];
    if count == 0 {
        return Ok(ids);
    }
    let status = unsafe {
        AudioObjectGetPropertyData(
            system,
            NonNull::from(&addr),
            0,
            std::ptr::null(),
            NonNull::from(&mut size),
            NonNull::new(ids.as_mut_ptr()).unwrap().cast(),
        )
    };
    if status != 0 {
        return Err(format!("no se pudo leer la lista de procesos de audio (OSStatus {status})"));
    }
    ids.truncate(size as usize / std::mem::size_of::<AudioObjectID>());
    Ok(ids)
}

/// Bundle ids de todos los procesos de audio. Para el diagnóstico.
pub fn audio_process_bundles() -> Result<Vec<String>, String> {
    Ok(audio_processes()?.into_iter().filter_map(bundle_id).collect())
}

/// UID del dispositivo de salida por defecto.
fn default_output_uid() -> Result<Retained<NSString>, String> {
    let system = kAudioObjectSystemObject as AudioObjectID;
    let addr = address(kAudioHardwarePropertyDefaultOutputDevice);
    let mut device: AudioObjectID = 0;
    let mut size = std::mem::size_of::<AudioObjectID>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            system,
            NonNull::from(&addr),
            0,
            std::ptr::null(),
            NonNull::from(&mut size),
            NonNull::from(&mut device).cast(),
        )
    };
    if status != 0 || device == 0 {
        return Err(format!("no se pudo leer el dispositivo de salida (OSStatus {status})"));
    }
    let addr = address(kAudioDevicePropertyDeviceUID);
    let mut value: *mut NSString = std::ptr::null_mut();
    let mut size = std::mem::size_of::<*mut NSString>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            device,
            NonNull::from(&addr),
            0,
            std::ptr::null(),
            NonNull::from(&mut size),
            NonNull::from(&mut value).cast(),
        )
    };
    if status != 0 || value.is_null() {
        return Err(format!("no se pudo leer el UID del dispositivo de salida (OSStatus {status})"));
    }
    unsafe { Retained::from_raw(value) }.ok_or_else(|| "UID de salida nulo".to_string())
}

/// Bundle id de un objeto de proceso (por ejemplo `com.spotify.client`).
fn bundle_id(process: AudioObjectID) -> Option<String> {
    let addr = address(kAudioProcessPropertyBundleID);
    let mut value: *mut NSString = std::ptr::null_mut();
    let mut size = std::mem::size_of::<*mut NSString>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            process,
            NonNull::from(&addr),
            0,
            std::ptr::null(),
            NonNull::from(&mut size),
            NonNull::from(&mut value).cast(),
        )
    };
    if status != 0 || value.is_null() {
        return None;
    }
    // Core Audio entrega el CFString con una referencia a nuestro cargo (regla "create").
    // CFString y NSString son intercambiables (toll-free bridging).
    let string = unsafe { Retained::from_raw(value) }?;
    Some(string.to_string())
}

fn cfstring(key: &'static CStr) -> CFRetained<CFString> {
    CFString::from_str(&key.to_string_lossy())
}

fn dict_set<K, V>(dict: &CFMutableDictionary, key: &K, value: &V) {
    unsafe {
        CFMutableDictionary::set_value(
            Some(dict),
            key as *const K as *const c_void,
            value as *const V as *const c_void,
        );
    }
}

fn new_dict(capacity: isize) -> CFRetained<CFMutableDictionary> {
    unsafe {
        CFMutableDictionary::new(
            kCFAllocatorDefault,
            capacity,
            &kCFTypeDictionaryKeyCallBacks,
            &kCFTypeDictionaryValueCallBacks,
        )
    }
    .expect("CFDictionaryCreateMutable")
}

/// Diccionario de propiedades del dispositivo agregado. Equivale a:
/// ```objc
/// @{ kAudioAggregateDeviceNameKey: name, kAudioAggregateDeviceUIDKey: uid,
///    kAudioAggregateDeviceTapListKey: @[ @{ kAudioSubTapUIDKey: tapUID,
///                                          kAudioSubTapDriftCompensationKey: @YES } ],
///    kAudioAggregateDeviceTapAutoStartKey: @YES, kAudioAggregateDeviceIsPrivateKey: @YES }
/// ```
/// Con `output_uid` se agregan además `MainSubDevice`, `SubDeviceList` e `IsStacked: @NO`.
fn aggregate_properties(
    tap_uid: &NSString,
    device_uid: &str,
    name: &str,
    output_uid: Option<&NSString>,
) -> CFRetained<CFDictionary> {
    let yes = NSNumber::new_bool(true);
    let no = NSNumber::new_bool(false);

    let sub_tap = new_dict(2);
    dict_set(&sub_tap, &*cfstring(kAudioSubTapUIDKey), tap_uid);
    dict_set(&sub_tap, &*cfstring(kAudioSubTapDriftCompensationKey), &*yes);
    let taps = array_of_one(&sub_tap);

    let dict = new_dict(8);
    dict_set(&dict, &*cfstring(kAudioAggregateDeviceNameKey), &*CFString::from_str(name));
    dict_set(&dict, &*cfstring(kAudioAggregateDeviceUIDKey), &*CFString::from_str(device_uid));
    dict_set(&dict, &*cfstring(kAudioAggregateDeviceTapListKey), &*taps);
    dict_set(&dict, &*cfstring(kAudioAggregateDeviceTapAutoStartKey), &*yes);
    dict_set(&dict, &*cfstring(kAudioEndPointDeviceIsPrivateKey), &*yes);
    if let Some(output) = output_uid {
        let sub_device = new_dict(1);
        dict_set(&sub_device, &*cfstring(kAudioSubDeviceUIDKey), output);
        let devices = array_of_one(&sub_device);
        dict_set(&dict, &*cfstring(kAudioAggregateDeviceMainSubDeviceKey), output);
        dict_set(&dict, &*cfstring(kAudioAggregateDeviceSubDeviceListKey), &*devices);
        dict_set(&dict, &*cfstring(kAudioAggregateDeviceIsStackedKey), &*no);
    }
    unsafe { CFRetained::cast_unchecked::<CFDictionary>(dict) }
}

/// `CFArray` con un solo diccionario (el arreglo lo retiene).
fn array_of_one(item: &CFMutableDictionary) -> CFRetained<CFArray> {
    let items = [item as *const CFMutableDictionary as *const c_void];
    unsafe { CFArray::new(kCFAllocatorDefault, items.as_ptr() as *mut *const c_void, 1, &kCFTypeArrayCallBacks) }
        .expect("CFArrayCreate")
}
