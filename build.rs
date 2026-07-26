use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    if let Err(error) = configure() {
        eprintln!("cargo:error={error}");
        std::process::exit(1);
    }
}

fn configure() -> Result<(), String> {
    println!("cargo:rerun-if-changed=src/faraweave.exe.manifest");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return Ok(());
    }
    let manifest = Path::new("src/faraweave.exe.manifest")
        .canonicalize()
        .map_err(|error| format!("unable to resolve Windows manifest: {error}"))?;
    println!("cargo:rustc-link-arg-bin=faraweave=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-bin=faraweave=/MANIFESTINPUT:{}",
        manifest.display()
    );
    let out = PathBuf::from(
        env::var_os("OUT_DIR").ok_or_else(|| "Cargo did not provide OUT_DIR".to_owned())?,
    );
    let rc_source = out.join("faraweave-version.rc");
    let resource = out.join("faraweave-version.res");
    let version = env::var("CARGO_PKG_VERSION")
        .map_err(|_| "Cargo did not provide CARGO_PKG_VERSION".to_owned())?;
    let components: Vec<&str> = version.split('.').collect();
    if components.len() != 3
        || components
            .iter()
            .any(|component| component.parse::<u16>().is_err())
    {
        return Err("Faraweave version must have three numeric components".to_owned());
    }
    let contents = format!(
        "1 VERSIONINFO\nFILEVERSION {0},{1},{2},0\nPRODUCTVERSION {0},{1},{2},0\n\
         FILEFLAGSMASK 0x3fL\nFILEFLAGS 0\nFILEOS 0x40004L\nFILETYPE 0x1L\n\
         BEGIN\n BLOCK \"StringFileInfo\"\n BEGIN\n  BLOCK \"040904b0\"\n  BEGIN\n\
         VALUE \"CompanyName\", \"Faraweave\\0\"\n\
         VALUE \"FileDescription\", \"Faraweave\\0\"\n\
         VALUE \"FileVersion\", \"{3}\\0\"\n\
         VALUE \"InternalName\", \"faraweave\\0\"\n\
         VALUE \"OriginalFilename\", \"faraweave.exe\\0\"\n\
         VALUE \"ProductName\", \"Faraweave\\0\"\n\
         VALUE \"ProductVersion\", \"{3}\\0\"\n\
         END\n END\n BLOCK \"VarFileInfo\"\n BEGIN\n VALUE \"Translation\", 0x409, 1200\n END\nEND\n",
        components[0], components[1], components[2], version
    );
    fs::write(&rc_source, contents)
        .map_err(|error| format!("unable to write Windows version resource: {error}"))?;
    let rc = find_resource_compiler()
        .ok_or_else(|| "Windows SDK rc.exe is required for PE metadata".to_owned())?;
    let status = Command::new(rc)
        .arg("/nologo")
        .arg(format!("/fo{}", resource.display()))
        .arg(&rc_source)
        .status()
        .map_err(|error| format!("unable to launch rc.exe: {error}"))?;
    if !status.success() {
        return Err("rc.exe rejected Faraweave version metadata".to_owned());
    }
    println!("cargo:rustc-link-arg-bin=faraweave={}", resource.display());
    Ok(())
}

fn find_resource_compiler() -> Option<PathBuf> {
    if let Some(sdk) = env::var_os("WindowsSdkVerBinPath") {
        let candidate = PathBuf::from(sdk).join("x64/rc.exe");
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    let root = Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    let mut versions: Vec<PathBuf> = fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path().join("x64/rc.exe"))
        .filter(|path| path.is_file())
        .collect();
    versions.sort();
    versions.pop()
}
