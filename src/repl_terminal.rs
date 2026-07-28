use std::env;
use std::ffi::OsStr;
use std::io::{self, IsTerminal, Write};

const ANSI_CLEAR_AND_HOME: &[u8] = b"\x1b[2J\x1b[H";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReplInputKind {
    Blank,
    Clear,
    Evaluate,
}

pub(crate) fn classify_repl_input(line: &str) -> ReplInputKind {
    if line.bytes().all(|byte| matches!(byte, b' ' | b'\t')) {
        ReplInputKind::Blank
    } else if line == ".cls" {
        ReplInputKind::Clear
    } else {
        ReplInputKind::Evaluate
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TerminalCapability {
    Redirected,
    Windows,
    Ansi,
    Unsupported,
}

pub(crate) fn classify_terminal(
    interactive: bool,
    windows_console: bool,
    term: Option<&OsStr>,
) -> TerminalCapability {
    if !interactive {
        return TerminalCapability::Redirected;
    }
    if windows_console {
        TerminalCapability::Windows
    } else if term.is_some_and(|value| !value.is_empty() && value != OsStr::new("dumb")) {
        TerminalCapability::Ansi
    } else {
        TerminalCapability::Unsupported
    }
}

pub(crate) fn stdout_capability() -> TerminalCapability {
    let term = env::var_os("TERM");
    classify_stdout_capability(
        io::stdout().is_terminal(),
        term.as_deref(),
        windows_stdout_is_console,
    )
}

fn classify_stdout_capability(
    interactive: bool,
    term: Option<&OsStr>,
    windows_console_probe: impl FnOnce() -> bool,
) -> TerminalCapability {
    if !interactive {
        return TerminalCapability::Redirected;
    }
    classify_terminal(true, windows_console_probe(), term)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClearOutcome {
    Redirected,
    Windows,
    Ansi,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TerminalOutputFailureReason {
    WriteFailed,
    FlushFailed,
}

impl TerminalOutputFailureReason {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::WriteFailed => "write_failed",
            Self::FlushFailed => "flush_failed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TerminalOutputFailure {
    pub(crate) reason: TerminalOutputFailureReason,
    pub(crate) pending_byte_count: usize,
    pub(crate) accepted_byte_count: usize,
    pub(crate) output_position: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ClearFailure {
    UnsupportedTerminal,
    WindowsOperationFailed,
    Output(TerminalOutputFailure),
}

pub(crate) fn report_clear_failure(
    failure: ClearFailure,
    output: &mut impl Write,
) -> io::Result<()> {
    match failure {
        ClearFailure::UnsupportedTerminal => {
            writeln!(
                output,
                "faraweave_repl_clear_error reason=unsupported_terminal"
            )
        }
        ClearFailure::WindowsOperationFailed => {
            writeln!(
                output,
                "faraweave_repl_clear_error reason=terminal_operation_failed"
            )
        }
        ClearFailure::Output(failure) => writeln!(
            output,
            "faraweave_repl_clear_error reason={} pending_byte_count={} accepted_byte_count={} output_position={}",
            failure.reason.name(),
            failure.pending_byte_count,
            failure.accepted_byte_count,
            failure.output_position
        ),
    }
}

pub(crate) fn clear_terminal(
    capability: TerminalCapability,
    output: &mut impl Write,
    clear_windows: impl FnOnce() -> Result<(), ClearFailure>,
) -> Result<ClearOutcome, ClearFailure> {
    match capability {
        TerminalCapability::Redirected => Ok(ClearOutcome::Redirected),
        TerminalCapability::Unsupported => Err(ClearFailure::UnsupportedTerminal),
        TerminalCapability::Windows => {
            clear_windows()?;
            Ok(ClearOutcome::Windows)
        }
        TerminalCapability::Ansi => {
            publish_ansi(output)?;
            Ok(ClearOutcome::Ansi)
        }
    }
}

fn publish_ansi(output: &mut impl Write) -> Result<(), ClearFailure> {
    let mut accepted = 0usize;
    while accepted < ANSI_CLEAR_AND_HOME.len() {
        match output.write(&ANSI_CLEAR_AND_HOME[accepted..]) {
            Ok(0) | Err(_) => {
                return Err(ClearFailure::Output(TerminalOutputFailure {
                    reason: TerminalOutputFailureReason::WriteFailed,
                    pending_byte_count: ANSI_CLEAR_AND_HOME.len(),
                    accepted_byte_count: accepted,
                    output_position: accepted,
                }));
            }
            Ok(count) => accepted = accepted.saturating_add(count),
        }
    }
    output.flush().map_err(|_| {
        ClearFailure::Output(TerminalOutputFailure {
            reason: TerminalOutputFailureReason::FlushFailed,
            pending_byte_count: ANSI_CLEAR_AND_HOME.len(),
            accepted_byte_count: accepted,
            output_position: accepted,
        })
    })
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows_console {
    use std::ffi::c_void;
    use std::mem::MaybeUninit;

    use super::ClearFailure;

    type Handle = *mut c_void;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Coord {
        x: i16,
        y: i16,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct SmallRect {
        left: i16,
        top: i16,
        right: i16,
        bottom: i16,
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct ConsoleScreenBufferInfo {
        size: Coord,
        cursor_position: Coord,
        attributes: u16,
        window: SmallRect,
        maximum_window_size: Coord,
    }

    #[link(name = "Kernel32")]
    unsafe extern "system" {
        fn GetStdHandle(standard_handle: u32) -> Handle;
        fn GetConsoleScreenBufferInfo(
            output: Handle,
            information: *mut ConsoleScreenBufferInfo,
        ) -> i32;
        fn FillConsoleOutputCharacterW(
            output: Handle,
            character: u16,
            length: u32,
            coordinate: Coord,
            written: *mut u32,
        ) -> i32;
        fn FillConsoleOutputAttribute(
            output: Handle,
            attribute: u16,
            length: u32,
            coordinate: Coord,
            written: *mut u32,
        ) -> i32;
        fn SetConsoleCursorPosition(output: Handle, coordinate: Coord) -> i32;
    }

    const STD_OUTPUT_HANDLE: u32 = -11_i32 as u32;

    fn stdout_screen_buffer() -> Result<(Handle, ConsoleScreenBufferInfo), ClearFailure> {
        let mut information = MaybeUninit::<ConsoleScreenBufferInfo>::uninit();

        // SAFETY: `information` is a valid stack out-pointer and is read only
        // after the API reports success. The returned opaque handle is used
        // only with Console APIs that accept an output screen-buffer handle.
        unsafe {
            let output = GetStdHandle(STD_OUTPUT_HANDLE);
            if output.is_null()
                || output as isize == -1
                || GetConsoleScreenBufferInfo(output, information.as_mut_ptr()) == 0
            {
                return Err(ClearFailure::WindowsOperationFailed);
            }
            Ok((output, information.assume_init()))
        }
    }

    pub(super) fn stdout_is_screen_buffer() -> bool {
        stdout_screen_buffer().is_ok()
    }

    pub(super) fn clear() -> Result<(), ClearFailure> {
        let (output, information) = stdout_screen_buffer()?;
        let home = Coord { x: 0, y: 0 };
        let mut written = 0_u32;
        let width =
            u32::try_from(information.size.x).map_err(|_| ClearFailure::WindowsOperationFailed)?;
        let height =
            u32::try_from(information.size.y).map_err(|_| ClearFailure::WindowsOperationFailed)?;
        let cells = width
            .checked_mul(height)
            .filter(|count| *count != 0)
            .ok_or(ClearFailure::WindowsOperationFailed)?;

        // SAFETY: `output` and `information` came from a successful
        // GetConsoleScreenBufferInfo call. All out-pointers refer to valid
        // stack storage, and `home` and `cells` are checked buffer coordinates.
        unsafe {
            if FillConsoleOutputCharacterW(output, u16::from(b' '), cells, home, &mut written) == 0
                || written != cells
            {
                return Err(ClearFailure::WindowsOperationFailed);
            }
            written = 0;
            if FillConsoleOutputAttribute(output, information.attributes, cells, home, &mut written)
                == 0
                || written != cells
                || SetConsoleCursorPosition(output, home) == 0
            {
                return Err(ClearFailure::WindowsOperationFailed);
            }
        }
        Ok(())
    }
}

#[cfg(windows)]
fn windows_stdout_is_console() -> bool {
    windows_console::stdout_is_screen_buffer()
}

#[cfg(not(windows))]
fn windows_stdout_is_console() -> bool {
    false
}

#[cfg(windows)]
pub(crate) fn clear_windows_console() -> Result<(), ClearFailure> {
    windows_console::clear()
}

#[cfg(not(windows))]
pub(crate) fn clear_windows_console() -> Result<(), ClearFailure> {
    Err(ClearFailure::UnsupportedTerminal)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ShortWriter {
        accepted: usize,
        flush_ok: bool,
        bytes: Vec<u8>,
    }

    impl Write for ShortWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.accepted == 0 {
                return Ok(0);
            }
            let count = self.accepted.min(bytes.len());
            self.bytes.extend_from_slice(&bytes[..count]);
            self.accepted -= count;
            Ok(count)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.flush_ok
                .then_some(())
                .ok_or_else(|| io::Error::other("injected flush failure"))
        }
    }

    #[test]
    fn input_command_is_exact_case_sensitive_and_rejects_variants() {
        assert_eq!(classify_repl_input(".cls"), ReplInputKind::Clear);
        assert_eq!(classify_repl_input(""), ReplInputKind::Blank);
        assert_eq!(classify_repl_input(" \t"), ReplInputKind::Blank);
        for input in [".CLS", ".Cls", ".cls ", " .cls", ".cls now", ".clsfoo"] {
            assert_eq!(
                classify_repl_input(input),
                ReplInputKind::Evaluate,
                "{input}"
            );
        }
    }

    #[test]
    fn capability_classification_separates_redirected_windows_ansi_and_unsupported() {
        assert_eq!(
            classify_terminal(false, true, None),
            TerminalCapability::Redirected
        );
        assert_eq!(
            classify_terminal(true, true, None),
            TerminalCapability::Windows
        );
        assert_eq!(
            classify_terminal(true, false, Some(OsStr::new("xterm-256color"))),
            TerminalCapability::Ansi
        );
        for term in [None, Some(OsStr::new("")), Some(OsStr::new("dumb"))] {
            assert_eq!(
                classify_terminal(true, false, term),
                TerminalCapability::Unsupported
            );
        }
    }

    #[test]
    fn stdout_capability_probes_native_windows_support_and_falls_back_to_ansi() {
        let mut redirected_probe_called = false;
        assert_eq!(
            classify_stdout_capability(false, Some(OsStr::new("xterm")), || {
                redirected_probe_called = true;
                true
            }),
            TerminalCapability::Redirected
        );
        assert!(!redirected_probe_called);

        assert_eq!(
            classify_stdout_capability(true, None, || true),
            TerminalCapability::Windows
        );
        assert_eq!(
            classify_stdout_capability(true, Some(OsStr::new("xterm-256color")), || false),
            TerminalCapability::Ansi
        );
        assert_eq!(
            classify_stdout_capability(true, Some(OsStr::new("dumb")), || false),
            TerminalCapability::Unsupported
        );
    }

    #[test]
    fn ansi_and_windows_paths_clear_and_home_without_crossing_output_seams() {
        let mut ansi = Vec::new();
        assert_eq!(
            clear_terminal(TerminalCapability::Ansi, &mut ansi, || {
                Err(ClearFailure::WindowsOperationFailed)
            }),
            Ok(ClearOutcome::Ansi)
        );
        assert_eq!(ansi, ANSI_CLEAR_AND_HOME);

        let mut windows_output = Vec::new();
        let mut windows_called = false;
        assert_eq!(
            clear_terminal(TerminalCapability::Windows, &mut windows_output, || {
                windows_called = true;
                Ok(())
            }),
            Ok(ClearOutcome::Windows)
        );
        assert!(windows_called);
        assert!(windows_output.is_empty());

        assert_eq!(
            clear_terminal(TerminalCapability::Windows, &mut windows_output, || Err(
                ClearFailure::WindowsOperationFailed
            )),
            Err(ClearFailure::WindowsOperationFailed)
        );
        assert!(windows_output.is_empty());
    }

    #[test]
    fn redirected_and_unsupported_paths_never_publish_control_bytes() {
        let mut redirected = Vec::new();
        let mut windows_called = false;
        assert_eq!(
            clear_terminal(TerminalCapability::Redirected, &mut redirected, || {
                windows_called = true;
                Ok(())
            }),
            Ok(ClearOutcome::Redirected)
        );
        assert!(!windows_called);
        assert!(redirected.is_empty());

        let mut unsupported = Vec::new();
        assert_eq!(
            clear_terminal(TerminalCapability::Unsupported, &mut unsupported, || Ok(())),
            Err(ClearFailure::UnsupportedTerminal)
        );
        assert!(unsupported.is_empty());
    }

    #[test]
    fn ansi_partial_write_and_flush_failures_are_exact() {
        let partial = clear_terminal(
            TerminalCapability::Ansi,
            &mut ShortWriter {
                accepted: 3,
                flush_ok: true,
                bytes: Vec::new(),
            },
            || Ok(()),
        )
        .expect_err("partial ANSI write");
        assert_eq!(
            partial,
            ClearFailure::Output(TerminalOutputFailure {
                reason: TerminalOutputFailureReason::WriteFailed,
                pending_byte_count: ANSI_CLEAR_AND_HOME.len(),
                accepted_byte_count: 3,
                output_position: 3,
            })
        );

        let flush = clear_terminal(
            TerminalCapability::Ansi,
            &mut ShortWriter {
                accepted: ANSI_CLEAR_AND_HOME.len(),
                flush_ok: false,
                bytes: Vec::new(),
            },
            || Ok(()),
        )
        .expect_err("ANSI flush");
        assert_eq!(
            flush,
            ClearFailure::Output(TerminalOutputFailure {
                reason: TerminalOutputFailureReason::FlushFailed,
                pending_byte_count: ANSI_CLEAR_AND_HOME.len(),
                accepted_byte_count: ANSI_CLEAR_AND_HOME.len(),
                output_position: ANSI_CLEAR_AND_HOME.len(),
            })
        );
    }

    #[test]
    fn clear_failure_diagnostics_are_exact_and_writes_remain_recoverable() {
        let cases = [
            (
                ClearFailure::UnsupportedTerminal,
                "faraweave_repl_clear_error reason=unsupported_terminal\n",
            ),
            (
                ClearFailure::WindowsOperationFailed,
                "faraweave_repl_clear_error reason=terminal_operation_failed\n",
            ),
            (
                ClearFailure::Output(TerminalOutputFailure {
                    reason: TerminalOutputFailureReason::WriteFailed,
                    pending_byte_count: ANSI_CLEAR_AND_HOME.len(),
                    accepted_byte_count: 3,
                    output_position: 3,
                }),
                "faraweave_repl_clear_error reason=write_failed pending_byte_count=7 accepted_byte_count=3 output_position=3\n",
            ),
        ];
        for (failure, expected) in cases {
            let mut diagnostic = Vec::new();
            report_clear_failure(failure, &mut diagnostic).expect("diagnostic output");
            assert_eq!(diagnostic, expected.as_bytes());
        }

        let failure = report_clear_failure(
            ClearFailure::UnsupportedTerminal,
            &mut ShortWriter {
                accepted: 0,
                flush_ok: true,
                bytes: Vec::new(),
            },
        );
        assert!(failure.is_err());
    }
}
