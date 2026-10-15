//! Web 字体和 Windows 应用图标均来自仓库内既有资源，无新增下载依赖。
use std::{env, fs, path::Path, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/fonts/NotoSansSC.ttf");
    println!("cargo:rerun-if-changed=assets/worldedit.ico");
    println!("cargo:rerun-if-env-changed=WORLDEDIT_WINDRES");
    let manifest = env::var("CARGO_MANIFEST_DIR").expect("Cargo must provide CARGO_MANIFEST_DIR");
    let output = env::var("OUT_DIR").expect("Cargo must provide OUT_DIR");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_windows_icon(Path::new(&manifest), Path::new(&output));
    }
    if env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        embed_web_font(Path::new(&manifest), Path::new(&output));
    }
}

fn embed_web_font(manifest: &Path, output: &Path) {
    let font = manifest.join("assets/fonts/NotoSansSC.ttf");
    let source = format!("pub const CJK: Option<&[u8]> = Some(include_bytes!({:?}));\npub const UI: Option<&[u8]> = None;\npub const CODE: Option<&[u8]> = None;\n", font);
    fs::write(output.join("web_fonts.rs"), source).expect("write bundled Web font constants");
}

fn embed_windows_icon(manifest: &Path, output: &Path) {
    let icon = fs::read(manifest.join("assets/worldedit.ico")).expect("read bundled Windows icon");
    let resource = output.join("worldedit-icon.res");
    fs::write(
        &resource,
        icon_resource(&icon).expect("valid bundled Windows icon"),
    )
    .expect("write Windows icon resource");
    let linked = match env::var("CARGO_CFG_TARGET_ENV").as_deref() {
        // MSVC LINK (and lld-link) natively consume the standard Win32 .res format.
        // No rc.exe discovery, SDK path guessing, or third-party build crate is needed.
        Ok("msvc") => resource,
        Ok("gnu") => {
            let object = output.join("worldedit-icon.o");
            convert_gnu_resource(&resource, &object);
            object
        }
        other => panic!("unsupported Windows linker environment: {other:?}"),
    };
    // Restrict the resource to this GUI binary; companion wl.exe remains a console CLI.
    println!("cargo:rustc-link-arg-bin=worldedit={}", linked.display());
}

fn convert_gnu_resource(resource: &Path, object: &Path) {
    let arch = env::var("CARGO_CFG_TARGET_ARCH").expect("Cargo must provide target architecture");
    let (prefix, format) = match arch.as_str() {
        "x86_64" => ("x86_64", "pe-x86-64"),
        "x86" => ("i686", "pe-i386"),
        "aarch64" => ("aarch64", "pe-aarch64-little"),
        _ => panic!("unsupported Windows GNU architecture: {arch}"),
    };
    let candidates = match env::var_os("WORLDEDIT_WINDRES") {
        Some(program) => vec![program],
        None => vec![
            format!("{prefix}-w64-mingw32-windres").into(),
            "windres".into(),
        ],
    };
    for program in candidates {
        let result = Command::new(&program)
            .args(["-J", "res", "-O", "coff", "--target", format, "-i"])
            .arg(resource)
            .arg("-o")
            .arg(object)
            .output();
        match result {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Ok(result) if result.status.success() => return,
            Ok(result) => panic!(
                "windres failed: {}",
                String::from_utf8_lossy(&result.stderr)
            ),
            Err(error) => panic!("cannot run windres: {error}"),
        }
    }
    panic!("Windows GNU builds require windres; set WORLDEDIT_WINDRES to its executable path");
}

// RESOURCEHEADER and RT_GROUP_ICON are defined by Microsoft's Resource File Formats.
// The ICO image payloads are copied verbatim, including all PNG-compressed sizes.
fn icon_resource(icon: &[u8]) -> Result<Vec<u8>, &'static str> {
    if icon.len() < 6 || icon[..4] != [0, 0, 1, 0] {
        return Err("expected an ICO file");
    }
    let count = u16::from_le_bytes([icon[4], icon[5]]);
    let directory_end = 6 + usize::from(count) * 16;
    if count == 0 || directory_end > icon.len() {
        return Err("missing ICO directory entries");
    }
    let mut resource = Vec::new();
    append_resource(&mut resource, 0, 0, 0, &[]);
    let mut group = icon[..6].to_vec();
    for index in 0..usize::from(count) {
        let entry = &icon[6 + index * 16..6 + (index + 1) * 16];
        let size = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
        let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
        let end = offset
            .checked_add(size)
            .ok_or("ICO image offset overflow")?;
        if entry[3] != 0 || size == 0 || offset < directory_end || end > icon.len() {
            return Err("invalid ICO image entry");
        }
        let id = (index + 1) as u16;
        append_resource(&mut resource, 3, id, 0x1030, &icon[offset..end]);
        group.extend_from_slice(&entry[..12]);
        group.extend_from_slice(&id.to_le_bytes());
    }
    append_resource(&mut resource, 14, 1, 0x1030, &group);
    Ok(resource)
}

fn append_resource(output: &mut Vec<u8>, kind: u16, id: u16, flags: u16, data: &[u8]) {
    output.extend_from_slice(&(data.len() as u32).to_le_bytes());
    output.extend_from_slice(&32_u32.to_le_bytes());
    for value in [0xffff, kind, 0xffff, id] {
        output.extend_from_slice(&value.to_le_bytes());
    }
    output.extend_from_slice(&0_u32.to_le_bytes());
    output.extend_from_slice(&flags.to_le_bytes());
    output.extend_from_slice(&0_u16.to_le_bytes()); // Language-neutral branding.
    output.extend_from_slice(&[0; 8]);
    output.extend_from_slice(data);
    while !output.len().is_multiple_of(4) {
        output.push(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_icon_preserves_all_seven_payloads_and_group_entries() {
        let icon = include_bytes!("assets/worldedit.ico");
        let resource = icon_resource(icon).unwrap();
        assert_eq!(
            &resource[..32],
            &[
                0, 0, 0, 0, 32, 0, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0, 0, 0
            ]
        );
        let mut position = 32;
        for index in 0..7 {
            let entry = &icon[6 + index * 16..6 + (index + 1) * 16];
            let size = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
            let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
            assert_eq!(&resource[position..position + 4], &entry[8..12]);
            assert_eq!(
                &resource[position + 10..position + 12],
                &3_u16.to_le_bytes()
            );
            assert_eq!(
                &resource[position + 14..position + 16],
                &((index + 1) as u16).to_le_bytes()
            );
            assert_eq!(
                &resource[position + 32..position + 32 + size],
                &icon[offset..offset + size]
            );
            position = (position + 32 + size + 3) & !3;
        }
        assert_eq!(
            &resource[position + 10..position + 12],
            &14_u16.to_le_bytes()
        );
        assert_eq!(&resource[position + 32..position + 38], &[0, 0, 1, 0, 7, 0]);
        for index in 0..7 {
            let entry = position + 38 + index * 14;
            assert_eq!(
                &resource[entry..entry + 12],
                &icon[6 + index * 16..18 + index * 16]
            );
            assert_eq!(
                &resource[entry + 12..entry + 14],
                &((index + 1) as u16).to_le_bytes()
            );
        }
        assert_eq!(resource.len(), position + 32 + 6 + 7 * 14);
    }

    #[test]
    fn malformed_icons_fail_instead_of_building_an_unbranded_executable() {
        for icon in [&[][..], &[0, 0, 1, 0, 0, 0], &[0, 0, 1, 0, 1, 0]] {
            assert!(icon_resource(icon).is_err());
        }
        let mut icon = include_bytes!("assets/worldedit.ico").to_vec();
        icon[18..22].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(icon_resource(&icon).is_err());
    }
}
