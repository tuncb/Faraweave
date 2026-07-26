use crate::{Error, ErrorKind, SourceLocation};
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativePlatform {
    GccLike,
    WindowsMsvc,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompilerConfiguration {
    ExplicitOption,
    Environment,
    Fallback,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilerSelection {
    pub executable: OsString,
    pub configuration: CompilerConfiguration,
}

#[derive(Clone, Debug)]
pub struct NativeBuildRequest<'a> {
    pub c_source: &'a str,
    pub output_path: &'a Path,
    pub explicit_compiler: Option<&'a str>,
    pub environment_compiler: Option<&'a str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeBuildResult {
    pub compiler: CompilerSelection,
}

pub const fn native_platform() -> NativePlatform {
    if cfg!(windows) {
        NativePlatform::WindowsMsvc
    } else {
        NativePlatform::GccLike
    }
}

pub fn select_c_compiler(
    explicit: Option<&str>,
    environment: Option<&str>,
    platform: NativePlatform,
) -> Result<CompilerSelection, Error> {
    if let Some(compiler) = explicit {
        if compiler.is_empty() {
            return Err(native_error("--cc requires a nonempty compiler"));
        }
        return Ok(CompilerSelection {
            executable: OsString::from(compiler),
            configuration: CompilerConfiguration::ExplicitOption,
        });
    }
    if let Some(compiler) = environment
        && !compiler.is_empty()
    {
        return Ok(CompilerSelection {
            executable: OsString::from(compiler),
            configuration: CompilerConfiguration::Environment,
        });
    }
    Ok(CompilerSelection {
        executable: OsString::from(match platform {
            NativePlatform::GccLike => "cc",
            NativePlatform::WindowsMsvc => "cl.exe",
        }),
        configuration: CompilerConfiguration::Fallback,
    })
}

pub fn make_c_compiler_arguments(
    platform: NativePlatform,
    source: &Path,
    output: &Path,
) -> Vec<OsString> {
    match platform {
        NativePlatform::GccLike => vec![
            "-std=c11".into(),
            "-frounding-math".into(),
            "-ffp-contract=off".into(),
            "-fno-fast-math".into(),
            "-Wall".into(),
            "-Wextra".into(),
            "-Werror".into(),
            "-pedantic-errors".into(),
            source.as_os_str().to_owned(),
            "-o".into(),
            output.as_os_str().to_owned(),
            "-lm".into(),
        ],
        NativePlatform::WindowsMsvc => vec![
            "/nologo".into(),
            "/std:c11".into(),
            "/W4".into(),
            "/WX".into(),
            "/fp:strict".into(),
            source.as_os_str().to_owned(),
            format!("/Fe:{}", output.display()).into(),
            format!("/Fo:{}.obj", output.display()).into(),
        ],
    }
}

pub fn build_native(request: &NativeBuildRequest<'_>) -> Result<NativeBuildResult, Error> {
    let platform = native_platform();
    let compiler = select_c_compiler(
        request.explicit_compiler,
        request.environment_compiler,
        platform,
    )?;
    let parent = request
        .output_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| native_error("system clock is before the Unix epoch"))?
        .as_nanos();
    let stem = request
        .output_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("faraweave-output");
    let c_path = parent.join(format!(".{stem}.{nonce}.c"));
    let native_path = temporary_native_path(parent, stem, nonce);
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&c_path)
            .map_err(|error| {
                native_error(format!("unable to create temporary C source: {error}"))
            })?;
        file.write_all(request.c_source.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|error| {
                native_error(format!("unable to write temporary C source: {error}"))
            })?;
        drop(file);
        let arguments = make_c_compiler_arguments(platform, &c_path, &native_path);
        let output = Command::new(&compiler.executable)
            .args(&arguments)
            .output()
            .map_err(|error| {
                native_error(format!(
                    "unable to launch compiler '{}': {error}",
                    compiler.executable.to_string_lossy()
                ))
            })?;
        if !output.status.success() {
            return Err(native_error(format!(
                "compiler '{}' exited with {}",
                compiler.executable.to_string_lossy(),
                output.status
            )));
        }
        publish_file(&native_path, request.output_path)?;
        Ok(NativeBuildResult {
            compiler: compiler.clone(),
        })
    })();
    let _ = fs::remove_file(&c_path);
    let _ = fs::remove_file(&native_path);
    let _ = fs::remove_file(format!("{}.obj", native_path.display()));
    result
}

fn temporary_native_path(parent: &Path, stem: &str, nonce: u128) -> PathBuf {
    let suffix = if cfg!(windows) { ".exe" } else { "" };
    parent.join(format!(".{stem}.{nonce}.native{suffix}"))
}

pub(crate) fn publish_file(temporary: &Path, output: &Path) -> Result<(), Error> {
    publish_file_atomically(temporary, output)
        .map_err(|error| native_error(format!("unable to publish native output: {error}")))
}

#[doc(hidden)]
pub fn publish_file_atomically(temporary: &Path, output: &Path) -> std::io::Result<()> {
    if !output.exists() {
        return fs::rename(temporary, output);
    }
    replace_existing_file(temporary, output)
}

#[cfg(not(windows))]
fn replace_existing_file(temporary: &Path, output: &Path) -> std::io::Result<()> {
    fs::rename(temporary, output)
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn replace_existing_file(temporary: &Path, output: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn ReplaceFileW(
            replaced_file_name: *const u16,
            replacement_file_name: *const u16,
            backup_file_name: *const u16,
            replace_flags: u32,
            exclude: *mut core::ffi::c_void,
            reserved: *mut core::ffi::c_void,
        ) -> i32;
    }

    let mut output_wide: Vec<u16> = output.as_os_str().encode_wide().collect();
    let mut temporary_wide: Vec<u16> = temporary.as_os_str().encode_wide().collect();
    output_wide.push(0);
    temporary_wide.push(0);
    // SAFETY: both path buffers are NUL-terminated and live for the call; the
    // optional backup, exclusion, and reserved pointers are permitted to be
    // null by ReplaceFileW. Both files were resolved by the caller.
    let replaced = unsafe {
        ReplaceFileW(
            output_wide.as_ptr(),
            temporary_wide.as_ptr(),
            ptr::null(),
            1,
            ptr::null_mut(),
            ptr::null_mut(),
        )
    };
    if replaced == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn native_error(message: impl Into<String>) -> Error {
    Error::new(
        ErrorKind::NativeBuildError,
        SourceLocation::start(),
        message,
    )
}
