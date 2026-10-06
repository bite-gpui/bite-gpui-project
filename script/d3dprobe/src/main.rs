//! Report the DXGI adapters in the guest and the Direct3D 11/12 feature level each one
//! supports.  Dependency-free (raw FFI) so it cross-compiles with only the mingw CRT.
//!
//!     cargo build --release --target x86_64-pc-windows-gnu
//!
//! Prints, per adapter: the name, vendor/device ids, the DXGI LUID and dedicated VRAM,
//! then the highest D3D11 feature level and whether D3D12 is available.

use std::ffi::c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct Guid {
    d1: u32,
    d2: u16,
    d3: u16,
    d4: [u8; 8],
}

const IID_IDXGI_FACTORY1: Guid = Guid {
    d1: 0x770aae78,
    d2: 0xf26f,
    d3: 0x4dba,
    d4: [0xa8, 0x29, 0x25, 0x3c, 0x83, 0xd1, 0xb3, 0x87],
};
const IID_ID3D12_DEVICE: Guid = Guid {
    d1: 0x189819f1,
    d2: 0x1db6,
    d3: 0x4b57,
    d4: [0xbe, 0x54, 0x18, 0x21, 0x33, 0x9b, 0x85, 0xf7],
};

// DXGI_ADAPTER_DESC1: Description[128] comes FIRST, then the integer fields.
#[repr(C)]
struct AdapterDesc1 {
    description: [u16; 128],
    vendor_id: u32,
    device_id: u32,
    subsys_id: u32,
    revision: u32,
    dedicated_video_memory: usize,
    dedicated_system_memory: usize,
    shared_system_memory: usize,
    luid_low: u32,
    luid_high: i32,
    flags: u32,
}

#[link(name = "dxgi")]
extern "system" {
    fn CreateDXGIFactory1(riid: *const Guid, out: *mut *mut c_void) -> i32;
}

#[link(name = "d3d11")]
extern "system" {
    fn D3D11CreateDevice(
        adapter: *mut c_void,
        driver_type: i32,
        software: *mut c_void,
        flags: u32,
        feature_levels: *const i32,
        num_feature_levels: u32,
        sdk_version: u32,
        device: *mut *mut c_void,
        feature_level: *mut i32,
        context: *mut *mut c_void,
    ) -> i32;
}

#[link(name = "d3d12")]
extern "system" {
    fn D3D12CreateDevice(
        adapter: *mut c_void,
        minimum_feature_level: i32,
        riid: *const Guid,
        device: *mut *mut c_void,
    ) -> i32;
}

type EnumAdapters1Fn = unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32;
type GetDesc1Fn = unsafe extern "system" fn(*mut c_void, *mut AdapterDesc1) -> i32;

/// COM vtable slot `index` of `obj` (IDXGIAdapter1::GetDesc1 is 10, IDXGIFactory1::EnumAdapters1 is 12).
unsafe fn vtable_fn(obj: *mut c_void, index: usize) -> *const c_void {
    let vtbl = *(obj as *mut *const *const c_void);
    *vtbl.add(index)
}

const D3D11_LEVELS: [(i32, &str); 5] = [
    (0xB100, "11_1"),
    (0xB000, "11_0"),
    (0xA100, "10_1"),
    (0xA000, "10_0"),
    (0x9300, "9_3"),
];
const D3D12_LEVELS: [(i32, &str); 3] = [(0xC100, "12_1"), (0xC000, "12_0"), (0xB000, "11_0")];

fn main() {
    unsafe {
        println!("d3dprobe start");
        {
            let levels = [0xB100i32, 0xB000, 0xA100, 0xA000, 0x9300];
            let (mut dev, mut ctx): (*mut c_void, *mut c_void) =
                (std::ptr::null_mut(), std::ptr::null_mut());
            let mut lvl = 0i32;
            let hr = D3D11CreateDevice(
                std::ptr::null_mut(),
                5, // D3D_DRIVER_TYPE_WARP
                std::ptr::null_mut(),
                0,
                levels.as_ptr(),
                levels.len() as u32,
                7, // D3D11_SDK_VERSION
                &mut dev,
                &mut lvl,
                &mut ctx,
            );
            println!(
                "D3D11 WARP (null adapter): hr=0x{:08X} level=0x{:04X}",
                hr as u32, lvl
            );
        }

        let mut factory: *mut c_void = std::ptr::null_mut();
        let hr = CreateDXGIFactory1(&IID_IDXGI_FACTORY1, &mut factory);
        if hr != 0 {
            println!("CreateDXGIFactory1 failed 0x{:08X}", hr as u32);
            return;
        }
        let enum_adapters1: EnumAdapters1Fn = std::mem::transmute(vtable_fn(factory, 12));

        let mut index = 0u32;
        loop {
            let mut adapter: *mut c_void = std::ptr::null_mut();
            if enum_adapters1(factory, index, &mut adapter) != 0 {
                break;
            }
            let get_desc1: GetDesc1Fn = std::mem::transmute(vtable_fn(adapter, 10));
            let mut desc: AdapterDesc1 = std::mem::zeroed();
            get_desc1(adapter, &mut desc);
            let name = String::from_utf16_lossy(&desc.description)
                .trim_end_matches('\0')
                .to_string();

            println!(
                "adapter {}: {}  [{:04X}:{:04X}]  luid {:08X}:{:08X}  {} MiB dedicated ({} shared)",
                index,
                name,
                desc.vendor_id,
                desc.device_id,
                desc.luid_high,
                desc.luid_low,
                desc.dedicated_video_memory / 1048576,
                desc.shared_system_memory / 1048576,
            );

            let levels: Vec<i32> = D3D11_LEVELS.iter().map(|(l, _)| *l).collect();
            let (mut dev, mut ctx): (*mut c_void, *mut c_void) =
                (std::ptr::null_mut(), std::ptr::null_mut());
            let mut level: i32 = 0;
            let hr = D3D11CreateDevice(
                adapter,
                0, // D3D_DRIVER_TYPE_UNKNOWN (required with a non-NULL adapter)
                std::ptr::null_mut(),
                0,
                levels.as_ptr(),
                levels.len() as u32,
                7, // D3D11_SDK_VERSION
                &mut dev,
                &mut level,
                &mut ctx,
            );
            if hr == 0 {
                let label = D3D11_LEVELS
                    .iter()
                    .find(|(l, _)| *l == level)
                    .map(|(_, s)| *s)
                    .unwrap_or("?");
                println!("    D3D11 : OK, feature level {label} (0x{level:04X})");
            } else {
                println!("    D3D11 : failed 0x{:08X}", hr as u32);
            }

            let mut d3d12 = "not supported".to_string();
            for (min, label) in D3D12_LEVELS {
                let mut dev12: *mut c_void = std::ptr::null_mut();
                if D3D12CreateDevice(adapter, min, &IID_ID3D12_DEVICE, &mut dev12) == 0 {
                    d3d12 = format!("OK, feature level {label} (0x{min:04X})");
                    break;
                }
            }
            println!("    D3D12 : {d3d12}");
            println!();

            index += 1;
        }
        println!("done");
    }
}
