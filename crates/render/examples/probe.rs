//! Sonda diagnostica: scopre fin dove arriva wgpu su questa macchina.
//!
//! ```bash
//! cargo run --offline -p pizzart-render --example probe
//! ```
//!
//! Ogni passo stampa prima di eseguire, così se il processo muore si sa
//! esattamente quale chiamata ha fallito.

use pizzart_render::block_on;

fn main() {
    println!("1. loading wgpu (nessun backend ancora toccato)");

    // Backend selezionabile: `probe dx12` / `probe vulkan` / `probe all`.
    let which = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "dx12".to_string());
    let backends = match which.as_str() {
        "all" => wgpu::Backends::all(),
        "vulkan" => wgpu::Backends::VULKAN,
        "gl" => wgpu::Backends::GL,
        _ => wgpu::Backends::DX12,
    };
    println!("   backends richiesti: {backends:?}");

    println!("2. creazione instance");
    let desc = wgpu::InstanceDescriptor {
        backends,
        ..Default::default()
    };
    let instance = wgpu::Instance::new(desc);
    println!("   instance creata");

    println!("3. enumerate_adapters");
    let adapters = instance.enumerate_adapters(wgpu::Backends::all());
    println!("   adapter trovati: {}", adapters.len());
    for a in &adapters {
        let info = a.get_info();
        println!(
            "   - {} | {:?} | {:?} | driver {}",
            info.name, info.device_type, info.backend, info.driver
        );
    }

    println!("4. request_adapter (async)");
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::default(),
        force_fallback_adapter: false,
        compatible_surface: None,
    }));
    match adapter {
        Some(a) => println!("   adapter scelto: {}", a.get_info().name),
        None => {
            println!("   nessun adapter disponibile: la GPU non è raggiungibile da qui");
            return;
        }
    }

    println!("5. request_device");
    let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
        .expect("adapter già trovato al passo 4");
    match block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None)) {
        Ok((device, queue)) => {
            let _ = (device, queue);
            println!("   device creato: la pipeline GPU può partire");
        }
        Err(e) => println!("   device non creato: {e}"),
    }
}
