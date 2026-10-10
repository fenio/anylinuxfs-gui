use serde::Serialize;
use std::sync::Arc;
use tauri::State;

use crate::cli::{execute_command, execute_command_with_elevation};
use crate::elevation::{ElevationState, TerminalInteraction};

// Run only in the selected, already-mounted VM. No user-controlled shell text,
// extra mounts, or filesystem changes are needed: -a includes other subtrees.
const DISCOVER: &str = r#"set -eu
command -v btrfs >/dev/null || { echo 'btrfs-progs is missing from the VM' >&2; exit 1; }
target=$(awk '$3 == "btrfs" { print $2; exit }' /proc/self/mounts)
[ -n "$target" ] || { echo 'No mounted Btrfs filesystem in this VM' >&2; exit 1; }
target=$(printf '%b' "$target")
all=$(btrfs subvolume list -a -p -u -q "$target")
readonly=$(btrfs subvolume list -a -r "$target")
default=$(btrfs subvolume get-default "$target")
printf '\nALFS_BTRFS_BEGIN\n%s\nALFS_BTRFS_READONLY\n%s\nALFS_BTRFS_DEFAULT\n%s\nALFS_BTRFS_END\n' "$all" "$readonly" "$default"
"#;

#[derive(Debug, Serialize)]
pub struct BtrfsSubvolume {
    id: u64,
    path: String,
    parent_id: u64,
    snapshot: bool,
    read_only: bool,
}

#[derive(Debug, Serialize)]
pub struct BtrfsSubvolumeList {
    default_id: u64,
    subvolumes: Vec<BtrfsSubvolume>,
}

fn parse_subvolumes(output: &str) -> Result<BtrfsSubvolumeList, String> {
    let body = output.split_once("ALFS_BTRFS_BEGIN\n")
        .and_then(|(_, rest)| rest.split_once("ALFS_BTRFS_END").map(|(body, _)| body))
        .ok_or_else(|| "Subvolume discovery did not complete. Update anylinuxfs to a version supporting vm exec and check that btrfs-progs is installed in the VM.".to_string())?;
    let (all, rest) = body
        .split_once("ALFS_BTRFS_READONLY\n")
        .ok_or("Invalid Btrfs discovery output")?;
    let (readonly, default) = rest
        .split_once("ALFS_BTRFS_DEFAULT\n")
        .ok_or("Invalid Btrfs discovery output")?;
    let default_id = default.split_whitespace().collect::<Vec<_>>();
    let default_id = match default_id.as_slice() {
        ["ID", id, ..] => id
            .parse::<u64>()
            .map_err(|_| "Invalid default subvolume ID")?,
        _ => return Err("Unable to determine the default Btrfs subvolume".into()),
    };
    let readonly_ids: std::collections::HashSet<u64> = readonly
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            (fields.next()? == "ID").then_some(())?;
            fields.next()?.parse().ok()
        })
        .collect();
    let mut subvolumes = Vec::new();
    for line in all.lines().filter(|line| !line.trim().is_empty()) {
        let (metadata, path) = line
            .split_once(" path ")
            .ok_or("Invalid Btrfs subvolume record")?;
        let fields: Vec<_> = metadata.split_whitespace().collect();
        if fields.first() != Some(&"ID") {
            return Err("Invalid Btrfs subvolume record".into());
        }
        let id = fields
            .get(1)
            .and_then(|id| id.parse::<u64>().ok())
            .ok_or("Invalid subvolume ID")?;
        let field = |name: &str| {
            fields
                .iter()
                .position(|value| *value == name)
                .and_then(|i| fields.get(i + 1))
                .copied()
        };
        let parent_id = field("parent")
            .and_then(|id| id.parse().ok())
            .ok_or("Invalid parent subvolume ID")?;
        let snapshot = field("parent_uuid").ok_or("Missing snapshot metadata")? != "-";
        subvolumes.push(BtrfsSubvolume {
            id,
            parent_id,
            snapshot,
            read_only: readonly_ids.contains(&id),
            path: path.strip_prefix("<FS_TREE>/").unwrap_or(path).to_string(),
        });
    }
    subvolumes.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(BtrfsSubvolumeList {
        default_id,
        subvolumes,
    })
}

#[tauri::command]
pub async fn list_btrfs_subvolumes(
    elevation_state: State<'_, Arc<ElevationState>>,
    device: String,
) -> Result<BtrfsSubvolumeList, String> {
    super::disk::validate_device_path(&device)?;
    let elevation = elevation_state.inner().clone();
    let operation = format!("btrfs-list:{}", device);
    let guard = elevation.begin_operation(operation.clone())?;
    let mode = guard.mode();
    let result = tokio::task::spawn_blocking(move || {
        let mounts = super::get_mount_status_sync()?;
        let mount = mounts
            .iter()
            .find(|mount| mount.device == device)
            .ok_or("Mount the filesystem before discovering Btrfs subvolumes")?;
        if mount.filesystem.as_deref() != Some("btrfs") {
            return Err("The mounted filesystem is not Btrfs".into());
        }
        let help = execute_command(&["vm", "exec", "--help"], false, None, true).map_err(|_| {
            "Update anylinuxfs to a version supporting vm exec to discover subvolumes".to_string()
        })?;
        if !help.contains("--") {
            return Err(
                "The installed anylinuxfs CLI does not support VM command execution".into(),
            );
        }
        let output = execute_command_with_elevation(
            &["vm", "exec", &device, "--", "sh", "-c", DISCOVER],
            true,
            None,
            false,
            mode,
            &elevation,
            TerminalInteraction::CaptureOutput { operation },
        )
        .map_err(|error| error.message())?;
        parse_subvolumes(&output.replace("\r\n", "\n"))
    })
    .await
    .map_err(|error| format!("Subvolume discovery failed: {}", error))?;
    drop(guard);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn discovery_script_quotes_mount_paths_and_requires_successful_metadata_reads() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let write_tool = |name: &str, script: &str| {
            let path = dir.path().join(name);
            std::fs::write(&path, script).unwrap();
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
        };
        write_tool(
            "awk",
            "#!/bin/sh\nprintf '%s\\n' '/mnt/disk\\040with\\040spaces'\n",
        );
        write_tool(
            "btrfs",
            r#"#!/bin/sh
for last do :; done
[ "$last" = '/mnt/disk with spaces' ] || exit 2
case "$2" in
list)
    [ "$4" = '-r' ] && exit 0
    printf '%s\n' 'ID 256 gen 10 parent 5 top level 5 parent_uuid - uuid abc path <FS_TREE>/@'
    ;;
get-default) printf '%s\n' 'ID 256 gen 10 top level 5 path @' ;;
*) exit 3 ;;
esac
"#,
        );
        let run = || {
            std::process::Command::new("/bin/sh")
                .args(["-c", DISCOVER])
                .env("PATH", dir.path())
                .output()
                .unwrap()
        };
        let result = run();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            parse_subvolumes(&String::from_utf8_lossy(&result.stdout))
                .unwrap()
                .default_id,
            256
        );

        write_tool("btrfs", "#!/bin/sh\nexit 1\n");
        let result = run();
        assert!(!result.status.success());
        assert!(parse_subvolumes(&String::from_utf8_lossy(&result.stdout)).is_err());
    }

    #[test]
    fn parses_nested_paths_snapshots_and_readonly_flags() {
        let output = "noise\nALFS_BTRFS_BEGIN\nID 256 gen 10 parent 5 top level 5 parent_uuid - uuid abc path <FS_TREE>/@\nID 257 gen 11 parent 256 top level 256 parent_uuid abc uuid def path <FS_TREE>/@/.snapshots/with spaces\nALFS_BTRFS_READONLY\nID 257 gen 11 top level 256 path @/.snapshots/with spaces\nALFS_BTRFS_DEFAULT\nID 256 gen 10 top level 5 path @\nALFS_BTRFS_END\n";
        let result = parse_subvolumes(output).unwrap();
        assert_eq!(result.default_id, 256);
        assert_eq!(result.subvolumes[0].path, "@");
        assert!(!result.subvolumes[0].snapshot);
        assert_eq!(result.subvolumes[1].path, "@/.snapshots/with spaces");
        assert_eq!(result.subvolumes[1].parent_id, 256);
        assert!(result.subvolumes[1].snapshot && result.subvolumes[1].read_only);
    }

    #[test]
    fn empty_filesystem_can_default_to_top_level() {
        let result = parse_subvolumes("ALFS_BTRFS_BEGIN\n\nALFS_BTRFS_READONLY\n\nALFS_BTRFS_DEFAULT\nID 5 (FS_TREE)\nALFS_BTRFS_END").unwrap();
        assert_eq!(result.default_id, 5);
        assert!(result.subvolumes.is_empty());
    }

    #[test]
    fn incomplete_and_malformed_output_is_not_an_empty_success() {
        for output in ["", "ALFS_BTRFS_BEGIN\n", "ALFS_BTRFS_BEGIN\ninvalid\nALFS_BTRFS_READONLY\n\nALFS_BTRFS_DEFAULT\nID 5\nALFS_BTRFS_END"] {
            assert!(parse_subvolumes(output).is_err());
        }
    }
}
