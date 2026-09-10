use serde::{Deserialize, Serialize};

/// A machine-wide desktop application registered in a Windows uninstall key.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct InstalledProgram {
    pub name: String,
    pub version: Option<String>,
    pub publisher: Option<String>,
}

#[cfg(windows)]
pub(super) async fn query_programs() -> Option<Vec<InstalledProgram>> {
    match tokio::task::spawn_blocking(registry::query_programs).await {
        Ok(Ok(programs)) => Some(programs),
        Ok(Err(error)) => {
            tracing::warn!(%error, "failed to query installed programs");
            None
        }
        Err(error) => {
            tracing::warn!(%error, "installed programs registry task failed");
            None
        }
    }
}

#[cfg(not(windows))]
pub(super) async fn query_programs() -> Option<Vec<InstalledProgram>> {
    None
}

#[cfg(windows)]
mod registry {
    use super::*;

    const UNINSTALL_PATH: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
    // OpenOptions::access accepts Win32 flags; windows-registry does not export them.
    // https://learn.microsoft.com/en-us/windows/win32/winprog64/accessing-an-alternate-registry-view
    const KEY_WOW64_64KEY: u32 = 0x0100;
    const KEY_WOW64_32KEY: u32 = 0x0200;

    fn optional<T>(result: windows_registry::Result<T>) -> windows_registry::Result<Option<T>> {
        match result {
            Ok(value) => Ok(Some(value)),
            // HRESULT_FROM_WIN32(ERROR_FILE_NOT_FOUND / ERROR_PATH_NOT_FOUND).
            Err(error) if matches!(error.code().0 as u32, 0x80070002 | 0x80070003) => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub(super) fn query_programs() -> anyhow::Result<Vec<InstalledProgram>> {
        let mut programs = Vec::new();
        // On 32-bit Windows the view flags are ignored; normalization removes duplicates.
        for view in [KEY_WOW64_32KEY, KEY_WOW64_64KEY] {
            let Some(uninstall) = optional(
                windows_registry::LOCAL_MACHINE
                    .options()
                    .read()
                    .access(view)
                    .open(UNINSTALL_PATH),
            )?
            else {
                continue;
            };
            for name in uninstall.keys()? {
                let Some(key) = optional(uninstall.options().read().access(view).open(name))?
                else {
                    // An application may have been uninstalled during enumeration.
                    continue;
                };
                if optional(key.get_u32("SystemComponent"))? == Some(1) {
                    continue;
                }
                let Some(name) = nonblank(optional(key.get_string("DisplayName"))?) else {
                    continue;
                };
                programs.push(InstalledProgram {
                    name,
                    version: optional(key.get_string("DisplayVersion"))?,
                    publisher: optional(key.get_string("Publisher"))?,
                });
            }
        }
        Ok(normalize_programs(programs))
    }
}

#[cfg(any(windows, test))]
fn nonblank(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

#[cfg(any(windows, test))]
fn normalize_programs(mut programs: Vec<InstalledProgram>) -> Vec<InstalledProgram> {
    for program in &mut programs {
        program.version = nonblank(program.version.take());
        program.publisher = nonblank(program.publisher.take());
    }
    programs.sort_by(|a, b| {
        (&a.name, &a.version, &a.publisher).cmp(&(&b.name, &b.version, &b.publisher))
    });
    programs.dedup();
    programs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn program(name: &str, version: Option<&str>, publisher: Option<&str>) -> InstalledProgram {
        InstalledProgram {
            name: name.into(),
            version: version.map(String::from),
            publisher: publisher.map(String::from),
        }
    }

    #[test]
    fn normalizes_empty_and_single_results() {
        assert!(normalize_programs(vec![]).is_empty());
        let name = "B\u{fc}ro \u{5de5}\u{5177}";
        assert_eq!(
            normalize_programs(vec![program(name, Some(" "), Some("\t"))]),
            vec![program(name, None, None)]
        );
        assert_eq!(nonblank(None), None);
        assert_eq!(nonblank(Some(" \t".into())), None);
        assert_eq!(nonblank(Some(name.into())), Some(name.into()));
    }

    #[test]
    fn sorts_and_deduplicates_programs() {
        assert_eq!(
            normalize_programs(vec![
                program("Z", Some("2"), Some("B")),
                program("A", None, None),
                program("Z", Some("1"), Some("B")),
                program("Z", Some("2"), Some("A")),
                program("Z", Some("2"), Some("B")),
                program("A", Some(" "), Some("")),
            ]),
            vec![
                program("A", None, None),
                program("Z", Some("1"), Some("B")),
                program("Z", Some("2"), Some("A")),
                program("Z", Some("2"), Some("B")),
            ]
        );
    }

    #[cfg(windows)]
    #[tokio::test]
    #[ignore = "reads installed programs from the local Windows machine"]
    async fn query_local_programs() {
        assert!(query_programs().await.is_some());
    }

    #[cfg(not(windows))]
    #[tokio::test]
    async fn query_is_unavailable_outside_windows() {
        assert!(query_programs().await.is_none());
    }
}
