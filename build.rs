use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const BRIDGE_FILES: &[&str] = &[
    "sensor-bridge.exe",
    "sensor-bridge.dll",
    "sensor-bridge.deps.json",
    "sensor-bridge.runtimeconfig.json",
    "LibreHardwareMonitorLib.dll",
    "HidSharp.dll",
    "System.Management.dll",
    "System.CodeDom.dll",
    "System.IO.Ports.dll",
    "sensor-bridge.sys",
];

fn main() {
    println!("cargo:rerun-if-changed=sensor-bridge/Program.cs");
    println!("cargo:rerun-if-changed=sensor-bridge/sensor-bridge.csproj");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=be-aced.toml");
    println!("cargo:rerun-if-changed=assets/images/logo/logo.png");

    // Same as s3-manager: derive logo-256.png for logo_helper from the master PNG.
    if let Err(err) = generate_logo_256() {
        panic!("failed to generate logo-256.png: {err}");
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let payload_dir = out_dir.join("payload");
    let _ = fs::create_dir_all(&payload_dir);

    if env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows") {
        embed_admin_manifest(&out_dir);
        build_and_stage_sensor_bridge(&payload_dir);
    }

    // Always generate the include module (may be empty if bridge build failed).
    write_embedded_module(&out_dir, &payload_dir);
}

/// Resize the master logo into the 256² asset used by `logo_helper`.
fn generate_logo_256() -> Result<(), Box<dyn std::error::Error>> {
    const SRC: &str = "assets/images/logo/logo.png";
    const DST: &str = "assets/images/logo/logo-256.png";

    let src = image::open(SRC)?.to_rgba8();
    let resized =
        image::imageops::resize(&src, 256, 256, image::imageops::FilterType::Lanczos3);
    resized.save(DST)?;
    println!("cargo:rerun-if-changed={DST}");
    Ok(())
}

fn write_ico(png_path: &str, ico_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Multi-size classic ICO (not PNG-compressed alone) so Explorer / taskbar
    // get a usable RT_GROUP_ICON under mingw windres.
    let src = image::open(png_path)?.to_rgba8();
    let mut dir = ico::IconDir::new(ico::ResourceType::Icon);
    for size in [16_u32, 32, 48, 256] {
        let resized =
            image::imageops::resize(&src, size, size, image::imageops::FilterType::Lanczos3);
        let entry = ico::IconImage::from_rgba_data(size, size, resized.into_raw());
        dir.add_entry(ico::IconDirEntry::encode(&entry)?);
    }
    let mut file = fs::File::create(ico_path)?;
    dir.write(&mut file)?;
    Ok(())
}

fn embed_admin_manifest(out_dir: &Path) {
    let mut res = winres::WindowsResource::new();

    let ico_path = out_dir.join("app_icon.ico");
    match write_ico("assets/images/logo/logo.png", &ico_path) {
        Ok(()) => {
            // windres is picky about separators in the generated .rc path.
            let icon_path = ico_path.to_string_lossy().replace('/', "\\");
            res.set_icon(&icon_path);
            // Also keep a stable copy for Inno Setup (installer/be-aced.iss).
            let installer_ico = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
                .join("installer")
                .join("app.ico");
            if let Some(parent) = installer_ico.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if let Err(err) = fs::copy(&ico_path, &installer_ico) {
                println!("cargo:warning=failed to copy installer app.ico: {err}");
            }
        }
        Err(err) => {
            println!("cargo:warning=failed to build app icon from logo.png: {err}");
        }
    }

    res.set("ProductName", "BeAced");
    res.set("FileDescription", "BeAced — Segotep digital display companion");

    // asInvoker: allow start without an immediate OS UAC dialog so we can
    // relaunch via ShellExecute "runas" (or show our own popup if declined).
    // Launch-on-startup uses a Highest-privileges scheduled task (no UAC at login).
    res.set_manifest(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}"/>
      <supportedOS Id="{1f676c76-80e1-4239-95bb-83d0f6d0da78}"/>
      <supportedOS Id="{4a2f28e3-53b9-4441-ba9c-d69d4a4a6e38}"/>
      <supportedOS Id="{35138b9a-5d96-4fbd-8e2d-a2440225f93a}"/>
      <supportedOS Id="{e2011457-1546-43c5-a5fe-008deee3d3f0}"/>
    </application>
  </compatibility>
</assembly>"#,
    );
    if let Err(err) = res.compile() {
        println!("cargo:warning=winres manifest embed failed: {err}");
        return;
    }

    // MSVC: winres emits resource.lib and already prints cargo:rustc-link-* for it.
    // GNU: ld drops `.rsrc` from a static `.a` unless we force-link resource.o / whole-archive.
    let env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if env == "msvc" {
        return;
    }

    let resource_obj = out_dir.join("resource.o");
    if resource_obj.is_file() {
        let path = resource_obj.to_string_lossy().replace('/', "\\");
        println!("cargo:rustc-link-arg={path}");
    } else {
        let lib = out_dir.join("libresource.a");
        if lib.is_file() {
            println!("cargo:rustc-link-arg=-Wl,--whole-archive");
            println!("cargo:rustc-link-arg={}", lib.display());
            println!("cargo:rustc-link-arg=-Wl,--no-whole-archive");
        } else {
            println!(
                "cargo:warning=resource.o / libresource.a missing after winres (icon may not embed): {}",
                out_dir.display()
            );
        }
    }
}

fn build_and_stage_sensor_bridge(payload_dir: &Path) {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let csproj = manifest_dir.join("sensor-bridge/sensor-bridge.csproj");
    if !csproj.is_file() {
        println!("cargo:warning=sensor-bridge project missing; CPU temp/power will be unavailable");
        return;
    }

    let status = Command::new("dotnet")
        .args([
            "build",
            csproj.to_str().unwrap(),
            "-c",
            "Release",
            "--nologo",
            "-v",
            "q",
        ])
        .status();

    match status {
        Ok(s) if s.success() => {}
        Ok(s) => {
            println!(
                "cargo:warning=dotnet build sensor-bridge failed (exit {s}); install .NET 8 SDK for CPU temp/power"
            );
            return;
        }
        Err(err) => {
            println!(
                "cargo:warning=dotnet not found ({err}); install .NET 8 SDK for CPU temp/power"
            );
            return;
        }
    }

    let bridge_out = manifest_dir.join("sensor-bridge/bin/Release/net8.0-windows");
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".into());
    let dest = env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("target"))
        .join(profile);

    if let Err(err) = fs::create_dir_all(&dest) {
        println!("cargo:warning=cannot create {}: {err}", dest.display());
        return;
    }

    // Stop a previously launched bridge so .exe/.sys are not locked (os error 32).
    let _ = Command::new("taskkill")
        .args(["/IM", "sensor-bridge.exe", "/F"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    // Force-kill skips LHM Close(); unload the kernel driver so .sys can be restaged.
    let _ = Command::new("sc.exe")
        .args(["stop", "R0sensor-bridge"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    let _ = Command::new("sc.exe")
        .args(["delete", "R0sensor-bridge"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    // Ensure official LHM deps sit beside the bridge build output.
    let official = manifest_dir.join("official");
    for name in [
        "LibreHardwareMonitorLib.dll",
        "HidSharp.dll",
        "System.Management.dll",
        "System.CodeDom.dll",
        "System.IO.Ports.dll",
    ] {
        let src = official.join(name);
        let dst = bridge_out.join(name);
        if src.is_file() {
            let _ = fs::copy(&src, &dst);
        }
    }

    for name in BRIDGE_FILES {
        let src = bridge_out.join(name);
        if !src.is_file() {
            continue;
        }
        // Stage next to cargo target (dev convenience).
        let dst = dest.join(name);
        if let Err(err) = stage_file(&src, &dst) {
            if name.ends_with(".sys") {
                println!("cargo:warning=skipped locked {name} (already staged/in use); build OK");
            } else {
                println!(
                    "cargo:warning=failed to stage {} -> {}: {err}",
                    src.display(),
                    dst.display()
                );
            }
        }
        // Copy into OUT_DIR for include_bytes! embedding.
        let embed_dst = payload_dir.join(name);
        if let Err(err) = fs::copy(&src, &embed_dst) {
            println!(
                "cargo:warning=failed to embed {}: {err}",
                src.display()
            );
        } else {
            println!("cargo:rerun-if-changed={}", src.display());
        }
    }

    // Default config template (optional).
    let cfg = manifest_dir.join("be-aced.toml");
    if cfg.is_file() {
        let _ = fs::copy(&cfg, payload_dir.join("be-aced.toml"));
    }
}

fn write_embedded_module(out_dir: &Path, payload_dir: &Path) {
    let mut entries = Vec::new();
    for name in BRIDGE_FILES
        .iter()
        .copied()
        .chain(std::iter::once("be-aced.toml"))
    {
        let path = payload_dir.join(name);
        if path.is_file() {
            // Escape for a Rust string literal.
            entries.push(format!(
                "    (\"{name}\", include_bytes!(r\"{}\")),",
                path.display().to_string().replace('\\', "/")
            ));
        }
    }

    let body = format!(
        "// @generated by build.rs — do not edit\n\
         pub static EMBEDDED_FILES: &[(&str, &[u8])] = &[\n\
         {}\n\
         ];\n",
        entries.join("\n")
    );
    fs::write(out_dir.join("embedded_payload.rs"), body).expect("write embedded_payload.rs");
}

fn stage_file(src: &Path, dst: &Path) -> std::io::Result<u64> {
    if let (Ok(src_meta), Ok(dst_meta)) = (fs::metadata(src), fs::metadata(dst)) {
        if src_meta.len() == dst_meta.len() {
            return Ok(dst_meta.len());
        }
    }
    fs::copy(src, dst)
}
