//! Windows GPU detection via WMI Win32_VideoController.

use anyhow::Result;
use decentgpu_common::types::{GpuBackend, GpuInfo};

/// Detect GPUs on Windows using WMI.
pub fn detect_gpus() -> Result<Vec<GpuInfo>> {
    use windows::{
        core::BSTR,
        Win32::System::{
            Com::{
                CoCreateInstance, CoInitializeEx, CoInitializeSecurity,
                CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, RPC_C_AUTHN_LEVEL_DEFAULT,
                RPC_C_IMP_LEVEL_IMPERSONATE,
            },
            Ole::EOAC_NONE,
            Wmi::{IWbemLocator, WbemLocator},
        },
    };

    let mut gpus = Vec::new();

    // Initialise COM.
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED)?;
        CoInitializeSecurity(
            None,
            -1,
            None,
            None,
            RPC_C_AUTHN_LEVEL_DEFAULT,
            RPC_C_IMP_LEVEL_IMPERSONATE,
            None,
            EOAC_NONE,
            None,
        )?;
    }

    let locator: IWbemLocator =
        unsafe { CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER)? };

    let server = unsafe {
        locator.ConnectServer(
            &BSTR::from("ROOT\\CIMV2"),
            None,
            None,
            None,
            0,
            None,
            None,
        )?
    };

    let query = BSTR::from("SELECT Caption, AdapterRAM FROM Win32_VideoController");
    let enumerator = unsafe {
        server.ExecQuery(
            &BSTR::from("WQL"),
            &query,
            windows::Win32::System::Wmi::WBEM_FLAG_FORWARD_ONLY
                | windows::Win32::System::Wmi::WBEM_FLAG_RETURN_IMMEDIATELY,
            None,
        )?
    };

    loop {
        let mut objects = [None; 1];
        let mut returned = 0u32;
        unsafe { enumerator.Next(0, &mut objects, &mut returned)? };
        if returned == 0 {
            break;
        }

        if let Some(obj) = &objects[0] {
            let name = get_string_property(obj, "Caption").unwrap_or_default();
            let adapter_ram = get_u64_property(obj, "AdapterRAM").unwrap_or(0);
            let vram_mb = adapter_ram / 1_048_576;

            let backend = classify_backend(&name);
            gpus.push(GpuInfo {
                name,
                vram_mb,
                backend,
            });
        }
    }

    if gpus.is_empty() {
        gpus.push(GpuInfo {
            name: "CPU".into(),
            vram_mb: 0,
            backend: GpuBackend::CpuOnly,
        });
    }

    Ok(gpus)
}

fn classify_backend(name: &str) -> GpuBackend {
    let lower = name.to_lowercase();
    if lower.contains("nvidia") {
        GpuBackend::Cuda
    } else if lower.contains("amd") || lower.contains("radeon") {
        GpuBackend::Rocm
    } else {
        GpuBackend::CpuOnly
    }
}

fn get_string_property(
    obj: &windows::Win32::System::Wmi::IWbemClassObject,
    name: &str,
) -> Option<String> {
    let mut variant = windows::Win32::System::Variant::VARIANT::default();
    unsafe {
        obj.Get(
            &windows::core::BSTR::from(name),
            0,
            &mut variant,
            None,
            None,
        )
        .ok()?;
    }
    // Extract string from VARIANT.
    let bstr = unsafe { windows::Win32::System::Variant::VariantToStringAlloc(&variant).ok()? };
    Some(bstr.to_string())
}

fn get_u64_property(
    obj: &windows::Win32::System::Wmi::IWbemClassObject,
    name: &str,
) -> Option<u64> {
    let mut variant = windows::Win32::System::Variant::VARIANT::default();
    unsafe {
        obj.Get(
            &windows::core::BSTR::from(name),
            0,
            &mut variant,
            None,
            None,
        )
        .ok()?;
    }
    let n = unsafe { windows::Win32::System::Variant::VariantToUInt64WithDefault(&variant, 0) };
    Some(n)
}
