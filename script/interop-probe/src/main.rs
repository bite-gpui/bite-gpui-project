//! The packed Windows interop probe: P6 (adapter LUID identity, DXGI + wgpu) and
//! P5 (the D3D12 -> D3D11 shared-texture fence loop).
//!
//! Each check prints a `PASS`/`FAIL`/`SKIP` line with its evidence and the adapter
//! identity it ran on, so a partial answer is still recorded.
//!
//! Cross-compile:
//!
//!     cargo build --release --target x86_64-pc-windows-gnu
//!
//! See `spi/rendering/probe-windows-packed.md`, `probe-p6-adapter-luid.md` and
//! `probe-p5-fence-loop.md`.

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

use windows::core::{Interface, PCWSTR};
use windows::Win32::Foundation::{CloseHandle, LUID};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL, D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_11_1,
};
use windows::Win32::Graphics::Direct3D11::*;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIAdapter, IDXGIAdapter1, IDXGIDevice, IDXGIFactory1,
    DXGI_ADAPTER_DESC, DXGI_ADAPTER_DESC1, DXGI_ADAPTER_FLAG_SOFTWARE,
};
use windows::Win32::System::Threading::{CreateEventW, WaitForSingleObject, INFINITE};

const GENERIC_ALL: u32 = 0x1000_0000;

const TEX_W: u32 = 64;
const TEX_H: u32 = 64;
/// A colour whose UNORM bytes are exact: RGBA (0, 255, 0, 255).
const CLEAR_RGBA: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
const EXPECTED_RGBA: [u8; 4] = [0, 255, 0, 255];

fn luid_str(l: LUID) -> String {
    format!("{:08X}:{:08X}", l.HighPart as u32, l.LowPart)
}

fn same_luid(a: LUID, b: LUID) -> bool {
    a.LowPart == b.LowPart && a.HighPart == b.HighPart
}

/// A tiny executor: `wgpu::Instance::enumerate_adapters` is async, but resolves
/// without ever really parking, so a no-op waker and a yield loop suffice.
fn block_on<F: Future>(fut: F) -> F::Output {
    struct NoopWake;
    impl Wake for NoopWake {
        fn wake(self: Arc<Self>) {}
    }
    let waker = Waker::from(Arc::new(NoopWake));
    let mut cx = Context::from_waker(&waker);
    let mut fut = pin!(fut);
    loop {
        match fut.as_mut().poll(&mut cx) {
            Poll::Ready(v) => return v,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

/// What P6's DXGI half established, and what P5 needs from it.
struct DxgiResult {
    adapter_desc1: DXGI_ADAPTER_DESC1,
    adapter: IDXGIAdapter,
    device_luid: LUID,
}

struct WgpuResult {
    luid_match: bool,
    picked: String,
}

fn main() {
    println!("interop-probe: the packed Windows checklist (P6 adapter identity + P5 fence loop)");
    println!("adapter order and LUIDs come from DXGI; wgpu is DX12-only here");
    println!();

    let dxgi = check_p6_dxgi();
    let wgpu = check_p6_wgpu(dxgi.as_ref());
    let p5 = check_p5(dxgi.as_ref());

    println!();
    println!("SUMMARY");
    println!(
        "P6 DXGI+LUID : {}",
        dxgi.as_ref()
            .map(|_| "PASS")
            .unwrap_or("FAIL (see above)")
    );
    println!(
        "P6 wgpu      : {}",
        match &wgpu {
            Ok(w) if w.luid_match => format!("PASS — LUID match on {}", w.picked),
            Ok(_) => "FAIL — no wgpu LUID matched the D3D11 device".to_string(),
            Err(e) => format!("SKIP/FAIL — {e}"),
        }
    );
    println!(
        "P5 fence loop: {}",
        match &p5 {
            Ok(note) => format!("PASS — byte-exact via {note}"),
            Err(e) => format!("FAIL/SKIP — {e}"),
        }
    );
}

/// P6, first half: enumerate DXGI adapters, read each LUID, create a D3D11 device
/// and read *its* adapter's LUID through `IDXGIDevice::GetAdapter` -> `GetDesc`.
fn check_p6_dxgi() -> Result<DxgiResult, String> {
    println!("== P6 DXGI ==");
    unsafe {
        let factory: IDXGIFactory1 =
            CreateDXGIFactory1().map_err(|e| format!("CreateDXGIFactory1: {e:?}"))?;

        let mut adapters: Vec<(String, LUID, u32)> = Vec::new();
        let mut chosen: Option<(IDXGIAdapter1, DXGI_ADAPTER_DESC1)> = None;
        let mut index = 0u32;
        loop {
            let adapter1 = match factory.EnumAdapters1(index) {
                Ok(a) => a,
                Err(_) => break,
            };
            let desc1: DXGI_ADAPTER_DESC1 = adapter1
                .GetDesc1()
                .map_err(|e| format!("GetDesc1({index}): {e:?}"))?;
            let name = String::from_utf16_lossy(&desc1.Description)
                .trim_end_matches('\0')
                .to_string();
            let software = desc1.Flags & (DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0;
            println!(
                "adapter {index}: {name}  [{:04X}:{:04X}]  luid {}  {} MiB dedicated  flags 0x{:X}{}",
                desc1.VendorId,
                desc1.DeviceId,
                luid_str(desc1.AdapterLuid),
                desc1.DedicatedVideoMemory / 1048576,
                desc1.Flags,
                if software { " (software)" } else { "" }
            );
            adapters.push((name, desc1.AdapterLuid, desc1.Flags));
            if chosen.is_none() && !software {
                chosen = Some((adapter1, desc1));
            } else if chosen.is_none() {
                chosen = Some((adapter1, desc1));
            }
            index += 1;
        }

        let (adapter1, adapter_desc1) =
            chosen.ok_or_else(|| "no DXGI adapters".to_string())?;
        let adapter: IDXGIAdapter = adapter1
            .cast()
            .map_err(|e| format!("IDXGIAdapter1 -> IDXGIAdapter: {e:?}"))?;

        println!(
            "D3D11 device is created on adapter with luid {} ({} )",
            luid_str(adapter_desc1.AdapterLuid),
            String::from_utf16_lossy(&adapter_desc1.Description)
                .trim_end_matches('\0')
        );

        // Create the D3D11 device on that adapter and read the adapter's LUID
        // back through IDXGIAdapter::GetDesc (via IDXGIDevice::GetAdapter).
        let levels = [D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0];
        let mut dev: Option<ID3D11Device> = None;
        let mut flevel = D3D_FEATURE_LEVEL(0);
        let mut ctx: Option<ID3D11DeviceContext> = None;
        D3D11CreateDevice(
            &adapter,
            D3D_DRIVER_TYPE_UNKNOWN,
            Default::default(),
            D3D11_CREATE_DEVICE_FLAG(0),
            Some(&levels),
            D3D11_SDK_VERSION,
            Some(&mut dev),
            Some(&mut flevel),
            Some(&mut ctx),
        )
        .map_err(|e| format!("D3D11CreateDevice: {e:?}"))?;
        let device = dev.ok_or_else(|| "D3D11CreateDevice returned no device".to_string())?;
        let _context = ctx.ok_or_else(|| "D3D11CreateDevice returned no context".to_string())?;

        let dxgi_device: IDXGIDevice = device
            .cast()
            .map_err(|e| format!("ID3D11Device -> IDXGIDevice: {e:?}"))?;
        let dev_adapter: IDXGIAdapter = dxgi_device
            .GetAdapter()
            .map_err(|e| format!("IDXGIDevice::GetAdapter: {e:?}"))?;
        let desc: DXGI_ADAPTER_DESC = dev_adapter
            .GetDesc()
            .map_err(|e| format!("IDXGIAdapter::GetDesc: {e:?}"))?;
        let device_luid = desc.AdapterLuid;

        let d3d11_level = match flevel {
            D3D_FEATURE_LEVEL_11_1 => "11_1",
            D3D_FEATURE_LEVEL_11_0 => "11_0",
            other => return Err(format!("unexpected D3D11 feature level 0x{:X}", other.0)),
        };

        println!(
            "D3D11 device: adapter luid {}  feature level {d3d11_level}",
            luid_str(device_luid)
        );
        println!(
            "P6 DXGI+LUID : PASS — {} DXGI adapter(s); D3D11 device on luid {} via IDXGIAdapter::GetDesc",
            adapters.len(),
            luid_str(device_luid)
        );

        Ok(DxgiResult {
            adapter_desc1,
            adapter,
            device_luid,
        })
    }
}

/// P6, second half: enumerate wgpu's DX12 adapters, read each one's LUID through
/// `as_hal::<dx12::Api>()` -> `raw_adapter()` -> `GetDesc`, and report whether any
/// matches the D3D11 device's adapter LUID.
fn check_p6_wgpu(dxgi: Result<&DxgiResult, &String>) -> Result<WgpuResult, String> {
    println!();
    println!("== P6 wgpu ==");
    let dxgi = dxgi.map_err(|e| format!("no DXGI baseline: {e}"))?;

    let mut idesc = wgpu::InstanceDescriptor::new_without_display_handle();
    idesc.backends = wgpu::Backends::DX12;
    let instance = wgpu::Instance::new(idesc);

    let adapters = block_on(instance.enumerate_adapters(wgpu::Backends::DX12));
    println!("wgpu enumerated {} DX12 adapter(s)", adapters.len());

    let mut matched: Option<String> = None;
    for (i, adapter) in adapters.iter().enumerate() {
        let info = adapter.get_info();
        let mut luid = None;
        let hal = unsafe { adapter.as_hal::<wgpu::hal::dx12::Api>() };
        match hal {
            Some(guard) => {
                let raw = guard.raw_adapter();
                match unsafe { raw.GetDesc() } {
                    Ok(d) => luid = Some(d.AdapterLuid),
                    Err(e) => println!("wgpu adapter {i}: raw GetDesc failed: {e:?}"),
                }
            }
            None => println!("wgpu adapter {i}: no dx12 hal"),
        }
        let luid_s = luid.map(luid_str).unwrap_or_else(|| "<none>".to_string());
        println!(
            "wgpu adapter {i}: {}  backend={:?} type={:?}  luid {}",
            info.name, info.backend, info.device_type, luid_s
        );
        if let Some(l) = luid {
            if same_luid(l, dxgi.device_luid) && matched.is_none() {
                matched = Some(format!(
                    "wgpu adapter {i} ({}) luid {}",
                    info.name,
                    luid_str(l)
                ));
            }
        }
    }

    // Which physical adapter each API picked.
    println!(
        "picked: D3D11 device on luid {} ({})",
        luid_str(dxgi.device_luid),
        String::from_utf16_lossy(&dxgi.adapter_desc1.Description).trim_end_matches('\0')
    );

    match matched {
        Some(picked) => {
            println!(
                "P6 wgpu      : PASS — LUID match between the D3D11 device ({}) and {picked}",
                luid_str(dxgi.device_luid)
            );
            Ok(WgpuResult {
                luid_match: true,
                picked,
            })
        }
        None => {
            println!(
                "P6 wgpu      : FAIL — no wgpu adapter LUID matched the D3D11 device luid {}",
                luid_str(dxgi.device_luid)
            );
            Ok(WgpuResult {
                luid_match: false,
                picked: "<none>".to_string(),
            })
        }
    }
}

/// P5: D3D12 renders a known colour into a shared texture and signals a shared
/// fence; D3D11 opens both, orders on the fence, and reads the bytes back.
fn check_p5(dxgi: Result<&DxgiResult, &String>) -> Result<String, String> {
    println!();
    println!("== P5 fence loop ==");
    let dxgi = dxgi.map_err(|e| format!("no DXGI baseline: {e}"))?;
    let adapter = &dxgi.adapter;
    let adapter_luid = luid_str(dxgi.device_luid);

    unsafe {
        // --- D3D12 producer -------------------------------------------------
        let mut dev12: Option<ID3D12Device> = None;
        D3D12CreateDevice(adapter, D3D_FEATURE_LEVEL_11_0, &mut dev12)
            .map_err(|e| format!("D3D12CreateDevice: {e:?}"))?;
        let device12 = dev12.ok_or_else(|| "D3D12CreateDevice returned no device".to_string())?;
        println!("D3D12 device on adapter luid {adapter_luid}");

        // Shared fence, exported as an NT handle.
        let fence12: ID3D12Fence = device12
            .CreateFence(0, D3D12_FENCE_FLAG_SHARED)
            .map_err(|e| format!("D3D12 CreateFence(SHARED): {e:?}"))?;
        let fence_handle = device12
            .CreateSharedHandle(&fence12, None, GENERIC_ALL, PCWSTR::null())
            .map_err(|e| format!("D3D12 fence CreateSharedHandle: {e:?}"))?;
        println!("ID3D12Fence shared handle: {fence_handle:?}");

        // A committed texture on a D3D12_HEAP_FLAG_SHARED heap, RT-capable.
        // (A placed resource cannot be shared directly: CreateSharedHandle takes
        //  heaps and committed resources, per the d3d12 docs.)
        let heap_props = D3D12_HEAP_PROPERTIES {
            Type: D3D12_HEAP_TYPE_DEFAULT,
            CPUPageProperty: D3D12_CPU_PAGE_PROPERTY_UNKNOWN,
            MemoryPoolPreference: D3D12_MEMORY_POOL_UNKNOWN,
            CreationNodeMask: 1,
            VisibleNodeMask: 1,
        };

        let res_desc = D3D12_RESOURCE_DESC {
            Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
            Alignment: 0,
            Width: TEX_W as u64,
            Height: TEX_H,
            DepthOrArraySize: 1,
            MipLevels: 1,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Layout: D3D12_TEXTURE_LAYOUT_UNKNOWN,
            Flags: D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET,
        };
        let clear = D3D12_CLEAR_VALUE {
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            Anonymous: D3D12_CLEAR_VALUE_0 { Color: CLEAR_RGBA },
        };
        let mut res_opt: Option<ID3D12Resource> = None;
        device12
            .CreateCommittedResource(
                &heap_props,
                D3D12_HEAP_FLAG_SHARED,
                &res_desc,
                D3D12_RESOURCE_STATE_COMMON,
                Some(&clear),
                &mut res_opt,
            )
            .map_err(|e| format!("D3D12 CreateCommittedResource(heap SHARED): {e:?}"))?;
        let resource =
            res_opt.ok_or_else(|| "CreateCommittedResource returned no resource".to_string())?;

        let res_handle = device12
            .CreateSharedHandle(&resource, None, GENERIC_ALL, PCWSTR::null())
            .map_err(|e| format!("D3D12 resource CreateSharedHandle: {e:?}"))?;
        println!("shared texture handle: {res_handle:?}  ({TEX_W}x{TEX_H} R8G8B8A8_UNORM committed, heap flag SHARED)");

        // RTV descriptor.
        let rtv_heap_desc = D3D12_DESCRIPTOR_HEAP_DESC {
            Type: D3D12_DESCRIPTOR_HEAP_TYPE_RTV,
            NumDescriptors: 1,
            Flags: D3D12_DESCRIPTOR_HEAP_FLAG_NONE,
            NodeMask: 0,
        };
        let rtv_heap: ID3D12DescriptorHeap = device12
            .CreateDescriptorHeap(&rtv_heap_desc)
            .map_err(|e| format!("D3D12 CreateDescriptorHeap(RTV): {e:?}"))?;
        let rtv = rtv_heap.GetCPUDescriptorHandleForHeapStart();
        device12.CreateRenderTargetView(&resource, None, rtv);

        // Command queue / allocator / list.
        let queue_desc = D3D12_COMMAND_QUEUE_DESC {
            Type: D3D12_COMMAND_LIST_TYPE_DIRECT,
            Priority: 0,
            Flags: D3D12_COMMAND_QUEUE_FLAG_NONE,
            NodeMask: 0,
        };
        let queue: ID3D12CommandQueue = device12
            .CreateCommandQueue(&queue_desc)
            .map_err(|e| format!("D3D12 CreateCommandQueue: {e:?}"))?;
        let allocator: ID3D12CommandAllocator = device12
            .CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT)
            .map_err(|e| format!("D3D12 CreateCommandAllocator: {e:?}"))?;
        let list: ID3D12GraphicsCommandList = device12
            .CreateCommandList(
                0,
                D3D12_COMMAND_LIST_TYPE_DIRECT,
                &allocator,
                None::<&ID3D12PipelineState>,
            )
            .map_err(|e| format!("D3D12 CreateCommandList: {e:?}"))?;

        // COMMON -> RENDER_TARGET, clear, RENDER_TARGET -> COMMON (shared state).
        let to_rt = transition(&resource, D3D12_RESOURCE_STATE_COMMON, D3D12_RESOURCE_STATE_RENDER_TARGET);
        let to_common = transition(
            &resource,
            D3D12_RESOURCE_STATE_RENDER_TARGET,
            D3D12_RESOURCE_STATE_COMMON,
        );
        list.ResourceBarrier(&[to_rt]);
        list.ClearRenderTargetView(rtv, &CLEAR_RGBA, None);
        list.ResourceBarrier(&[to_common]);
        list.Close()
            .map_err(|e| format!("D3D12 list Close: {e:?}"))?;

        let cl: ID3D12CommandList = list
            .cast()
            .map_err(|e| format!("ID3D12GraphicsCommandList -> ID3D12CommandList: {e:?}"))?;
        queue.ExecuteCommandLists(&[Some(cl)]);
        queue
            .Signal(&fence12, 1)
            .map_err(|e| format!("D3D12 queue Signal: {e:?}"))?;
        println!("D3D12 cleared RGBA {CLEAR_RGBA:?}, queued signal value 1");

        // --- D3D11 consumer -------------------------------------------------
        let levels = [D3D_FEATURE_LEVEL_11_1, D3D_FEATURE_LEVEL_11_0];
        let mut dev11: Option<ID3D11Device> = None;
        let mut flevel = D3D_FEATURE_LEVEL(0);
        let mut ctx11: Option<ID3D11DeviceContext> = None;
        D3D11CreateDevice(
            adapter,
            D3D_DRIVER_TYPE_UNKNOWN,
            Default::default(),
            D3D11_CREATE_DEVICE_FLAG(0),
            Some(&levels),
            D3D11_SDK_VERSION,
            Some(&mut dev11),
            Some(&mut flevel),
            Some(&mut ctx11),
        )
        .map_err(|e| format!("D3D11CreateDevice: {e:?}"))?;
        let device11 = dev11.ok_or_else(|| "no D3D11 device".to_string())?;
        let context11 = ctx11.ok_or_else(|| "no D3D11 context".to_string())?;

        let device1: ID3D11Device1 = device11
            .cast()
            .map_err(|e| format!("ID3D11Device -> ID3D11Device1: {e:?}"))?;
        let texture: ID3D11Texture2D = device1
            .OpenSharedResource1(res_handle)
            .map_err(|e| format!("ID3D11Device1::OpenSharedResource1: {e:?}"))?;
        let mut srv_opt: Option<ID3D11ShaderResourceView> = None;
        device11
            .CreateShaderResourceView(&texture, None, Some(&mut srv_opt))
            .map_err(|e| format!("ID3D11 CreateShaderResourceView: {e:?}"))?;
        let _srv = srv_opt.ok_or_else(|| "no D3D11 SRV".to_string())?;
        println!("D3D11 opened the shared texture (OpenSharedResource1) and made an SRV");

        // Try the GPU-side order: ID3D11Fence + ID3D11DeviceContext4::Wait.
        let order_note: String;
        let gpu_fence = (|| -> Result<ID3D11Fence, String> {
            let device5: ID3D11Device5 = device11
                .cast()
                .map_err(|e| format!("ID3D11Device -> ID3D11Device5: {e:?}"))?;
            let mut f: Option<ID3D11Fence> = None;
            device5
                .OpenSharedFence(fence_handle, &mut f)
                .map_err(|e| format!("ID3D11Device5::OpenSharedFence: {e:?}"))?;
            let fence11 = f.ok_or_else(|| "OpenSharedFence returned nothing".to_string())?;
            let context4: ID3D11DeviceContext4 = context11
                .cast()
                .map_err(|e| format!("ID3D11DeviceContext -> ID3D11DeviceContext4: {e:?}"))?;
            context4
                .Wait(&fence11, 1)
                .map_err(|e| format!("ID3D11DeviceContext4::Wait: {e:?}"))?;
            Ok(fence11)
        })();

        match gpu_fence {
            Ok(_f) => {
                println!("order: ID3D12Fence shared handle opened as ID3D11Fence; context4.Wait(fence, 1) — GPU-side");
                order_note = "ID3D11Fence (GPU-side context4.Wait)".to_string();
            }
            Err(e) => {
                println!("order: GPU-side fence unavailable ({e}); falling back to a CPU wait");
                let event = CreateEventW(None, false, false, PCWSTR::null())
                    .map_err(|e| format!("CreateEventW: {e:?}"))?;
                fence12
                    .SetEventOnCompletion(1, event)
                    .map_err(|e| format!("ID3D12Fence::SetEventOnCompletion: {e:?}"))?;
                let w = WaitForSingleObject(event, INFINITE);
                let _ = CloseHandle(event);
                if w.0 != 0 {
                    return Err(format!("WaitForSingleObject returned {}", w.0));
                }
                println!("order: ID3D12Fence -> SetEventOnCompletion + WaitForSingleObject — CPU-side fallback, GPU-side fence not used");
                order_note = format!("CPU wait fallback ({e})");
            }
        }

        // --- read back byte-for-byte ---------------------------------------
        let staging_desc = D3D11_TEXTURE2D_DESC {
            Width: TEX_W,
            Height: TEX_H,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };
        let mut staging_opt: Option<ID3D11Texture2D> = None;
        device11
            .CreateTexture2D(&staging_desc, None, Some(&mut staging_opt))
            .map_err(|e| format!("D3D11 CreateTexture2D(staging): {e:?}"))?;
        let staging =
            staging_opt.ok_or_else(|| "CreateTexture2D returned no staging texture".to_string())?;

        context11.CopyResource(&staging, &texture);
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        context11
            .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
            .map_err(|e| format!("D3D11 Map(staging): {e:?}"))?;

        let pitch = mapped.RowPitch as usize;
        let base = mapped.pData as *const u8;
        let mut mismatches = 0usize;
        let mut first_bad: Option<(u32, u32, [u8; 4])> = None;
        for y in 0..TEX_H {
            for x in 0..TEX_W {
                let off = y as usize * pitch + x as usize * 4;
                let px = [
                    *base.add(off),
                    *base.add(off + 1),
                    *base.add(off + 2),
                    *base.add(off + 3),
                ];
                if px != EXPECTED_RGBA {
                    mismatches += 1;
                    if first_bad.is_none() {
                        first_bad = Some((x, y, px));
                    }
                }
            }
        }
        context11.Unmap(&staging, 0);

        let first = [
            *base,
            *base.add(1),
            *base.add(2),
            *base.add(3),
        ];
        println!(
            "readback: expected byte-exact RGBA {EXPECTED_RGBA:?}; first pixel {first:?}; {mismatches}/{} mismatched",
            TEX_W * TEX_H
        );
        if let Some((x, y, px)) = first_bad {
            println!("first mismatch at ({x},{y}): got {px:?}");
        }

        let _ = CloseHandle(res_handle);
        let _ = CloseHandle(fence_handle);

        if mismatches == 0 {
            println!(
                "P5 fence loop: PASS — shared texture read back byte-exact on D3D11 (adapter luid {adapter_luid})"
            );
            Ok(order_note)
        } else {
            Err(format!(
                "{mismatches} pixels did not match (first at {:?})",
                first_bad
            ))
        }
    }
}

/// A D3D12_TRANSITION barrier for subresource 0.
unsafe fn transition(
    resource: &ID3D12Resource,
    before: D3D12_RESOURCE_STATES,
    after: D3D12_RESOURCE_STATES,
) -> D3D12_RESOURCE_BARRIER {
    use std::mem::ManuallyDrop;
    let trans = D3D12_RESOURCE_TRANSITION_BARRIER {
        pResource: ManuallyDrop::new(Some(resource.clone())),
        Subresource: 0,
        StateBefore: before,
        StateAfter: after,
    };
    D3D12_RESOURCE_BARRIER {
        Type: D3D12_RESOURCE_BARRIER_TYPE_TRANSITION,
        Flags: D3D12_RESOURCE_BARRIER_FLAG_NONE,
        Anonymous: D3D12_RESOURCE_BARRIER_0 {
            Transition: ManuallyDrop::new(trans),
        },
    }
}
