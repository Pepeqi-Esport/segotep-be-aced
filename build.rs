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

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let payload_dir = out_dir.join("payload");
    let _ = fs::create_dir_all(&payload_dir);

    if env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows") {
        embed_admin_manifest();
        build_and_stage_sensor_bridge(&payload_dir);
    }

    // Always generate the include module (may be empty if bridge build failed).
    write_embedded_module(&out_dir, &payload_dir);
}

fn embed_admin_manifest() {
    let mut res = winres::WindowsResource::new();
    // asInvoker: allow start without UAC so we can show our own "must run as admin" popup.
    // Elevation is enforced in `admin::require_administrator()` at process start.
    // Launch-on-startup uses a Highest-privileges scheduled task instead.
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
