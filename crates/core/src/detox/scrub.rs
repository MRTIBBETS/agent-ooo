use super::sanitizer::{contains_json_unicode_escape, has_secret_candidate, redact_secrets_only};
use crate::OooError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{File, Permissions};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

const COPY_BUF_SIZE: usize = 8192;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InPlaceRedactionReceipt {
    pub target_path: String,
    pub backup_path: Option<String>,
    pub backup_verified: bool,
    pub modified: bool,
    pub lines_scanned: usize,
    pub lines_modified: usize,
    pub redactions_count: usize,
    pub bytes_before: u64,
    pub bytes_after: u64,
    pub original_blake3: String,
    pub final_blake3: String,
}

/// Resolves all transcript files that should be scrubbed in place for a given input path.
/// When `path` is `transcript.jsonl` and a sibling `transcript_full.jsonl` exists,
/// both files are returned (`transcript_full.jsonl` first) so neither retains leaked secrets.
pub fn resolve_in_place_targets(path: &Path) -> Vec<PathBuf> {
    if path.is_dir() {
        return vec![crate::checkin::discovery::resolve_scan_target(path)];
    }
    if path.file_name().and_then(|n| n.to_str()) == Some("transcript.jsonl") {
        let full_path = path.with_file_name("transcript_full.jsonl");
        if full_path.is_file() {
            return vec![full_path, path.to_path_buf()];
        }
    }
    vec![path.to_path_buf()]
}

fn is_structural_key(key: &str) -> bool {
    matches!(
        key,
        "id" | "tool_use_id"
            | "uuid"
            | "parentUuid"
            | "sessionId"
            | "type"
            | "role"
            | "source"
            | "status"
            | "created_at"
            | "timestamp"
    )
}

fn redact_json_value(value: &mut Value, redactions: &mut usize) {
    match value {
        Value::String(s) => {
            if has_secret_candidate(s.as_bytes()) {
                let (clean, count) = redact_secrets_only(s);
                if count > 0 {
                    *s = clean;
                    *redactions += count;
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                redact_json_value(item, redactions);
            }
        }
        Value::Object(map) => {
            for (key, val) in map.iter_mut() {
                if is_structural_key(key) {
                    continue;
                }
                redact_json_value(val, redactions);
            }
        }
        _ => {}
    }
}

fn split_line_ending(line: &str) -> (&str, &str) {
    if let Some(body) = line.strip_suffix("\r\n") {
        (body, "\r\n")
    } else if let Some(body) = line.strip_suffix('\n') {
        (body, "\n")
    } else {
        (line, "")
    }
}

fn write_tracked<W: Write>(
    writer: &mut W,
    hasher: &mut blake3::Hasher,
    total_bytes: &mut u64,
    bytes: &[u8],
) -> crate::Result<()> {
    writer.write_all(bytes)?;
    hasher.update(bytes);
    *total_bytes += bytes.len() as u64;
    Ok(())
}

fn stream_hash_and_len(path: &Path) -> crate::Result<(u64, String)> {
    let mut file = File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = [0u8; COPY_BUF_SIZE];
    let mut total: u64 = 0;

    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        total += n as u64;
    }

    Ok((total, hasher.finalize().to_hex().to_string()))
}

fn create_verified_backup(
    src_path: &Path,
    parent_dir: &Path,
    backup_path: &Path,
    expected_bytes: u64,
    expected_blake3: &str,
    perms: &Permissions,
) -> crate::Result<()> {
    if std::fs::symlink_metadata(backup_path).is_ok() {
        return Err(OooError::General(format!(
            "Refusing to overwrite existing backup file: {}",
            backup_path.display()
        )));
    }

    let mut src = File::open(src_path)?;
    let mut backup_temp = NamedTempFile::new_in(parent_dir)?;
    let mut buf = [0u8; COPY_BUF_SIZE];
    let mut copied_bytes: u64 = 0;
    let mut copy_hasher = blake3::Hasher::new();

    loop {
        let n = src.read(&mut buf)?;
        if n == 0 {
            break;
        }
        backup_temp.as_file_mut().write_all(&buf[..n])?;
        copy_hasher.update(&buf[..n]);
        copied_bytes += n as u64;
    }

    backup_temp.as_file().sync_all()?;
    backup_temp.as_file().set_permissions(perms.clone())?;

    let copy_digest = copy_hasher.finalize().to_hex().to_string();
    if copied_bytes != expected_bytes || copy_digest != expected_blake3 {
        return Err(OooError::General(format!(
            "Backup verification failed before persistence for {} (expected {} bytes / {}, got {} bytes / {})",
            src_path.display(),
            expected_bytes,
            expected_blake3,
            copied_bytes,
            copy_digest
        )));
    }

    backup_temp
        .persist_noclobber(backup_path)
        .map_err(|e| OooError::Io(e.error))?;

    let (disk_bytes, disk_digest) = stream_hash_and_len(backup_path)?;
    if disk_bytes != expected_bytes || disk_digest != expected_blake3 {
        return Err(OooError::General(format!(
            "On-disk backup verification failed for {} (expected {} bytes / {}, got {} bytes / {})",
            backup_path.display(),
            expected_bytes,
            expected_blake3,
            disk_bytes,
            disk_digest
        )));
    }

    Ok(())
}

/// Safely scrubs leaked secrets from `path` in place using constant-memory streaming,
/// optional `.bak` verification, and atomic same-directory temp-file replacement.
pub fn redact_transcript_in_place(
    path: &Path,
    create_backup: bool,
) -> crate::Result<InPlaceRedactionReceipt> {
    let meta = std::fs::symlink_metadata(path)?;
    if !meta.is_file() {
        return Err(OooError::General(format!(
            "Refusing to scrub non-regular file or symlink: {}",
            path.display()
        )));
    }

    let original_perms = meta.permissions();
    let bytes_before = meta.len();
    let parent_dir = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };

    let planned_backup = if create_backup {
        let mut os = path.as_os_str().to_os_string();
        os.push(".bak");
        Some(PathBuf::from(os))
    } else {
        None
    };

    let src_file = File::open(path)?;
    let mut reader = BufReader::new(src_file);
    let mut redacted_temp = NamedTempFile::new_in(parent_dir)?;

    let mut orig_hasher = blake3::Hasher::new();
    let mut final_hasher = blake3::Hasher::new();
    let mut bytes_after: u64 = 0;
    let mut lines_scanned: usize = 0;
    let mut lines_modified: usize = 0;
    let mut redactions_count: usize = 0;
    let mut line = String::new();

    {
        let mut writer = BufWriter::new(&mut redacted_temp);
        loop {
            line.clear();
            let bytes_read = reader.read_line(&mut line)?;
            if bytes_read == 0 {
                break;
            }
            orig_hasher.update(line.as_bytes());
            lines_scanned += 1;

            let line_bytes = line.as_bytes();
            if !has_secret_candidate(line_bytes) && !contains_json_unicode_escape(line_bytes) {
                write_tracked(&mut writer, &mut final_hasher, &mut bytes_after, line_bytes)?;
                continue;
            }

            let (body, eol) = split_line_ending(&line);
            if let Ok(mut val) = serde_json::from_str::<Value>(body) {
                let mut line_redactions = 0usize;
                redact_json_value(&mut val, &mut line_redactions);
                if line_redactions > 0 {
                    let serialized = serde_json::to_vec(&val)?;
                    write_tracked(
                        &mut writer,
                        &mut final_hasher,
                        &mut bytes_after,
                        &serialized,
                    )?;
                    write_tracked(
                        &mut writer,
                        &mut final_hasher,
                        &mut bytes_after,
                        eol.as_bytes(),
                    )?;
                    lines_modified += 1;
                    redactions_count += line_redactions;
                } else {
                    write_tracked(&mut writer, &mut final_hasher, &mut bytes_after, line_bytes)?;
                }
            } else {
                let (clean, line_redactions) = redact_secrets_only(body);
                if line_redactions > 0 {
                    write_tracked(
                        &mut writer,
                        &mut final_hasher,
                        &mut bytes_after,
                        clean.as_bytes(),
                    )?;
                    write_tracked(
                        &mut writer,
                        &mut final_hasher,
                        &mut bytes_after,
                        eol.as_bytes(),
                    )?;
                    lines_modified += 1;
                    redactions_count += line_redactions;
                } else {
                    write_tracked(&mut writer, &mut final_hasher, &mut bytes_after, line_bytes)?;
                }
            }
        }
        writer.flush()?;
    }

    redacted_temp.as_file().sync_all()?;
    let original_blake3 = orig_hasher.finalize().to_hex().to_string();
    let final_blake3 = final_hasher.finalize().to_hex().to_string();

    if redactions_count == 0 {
        drop(redacted_temp);
        return Ok(InPlaceRedactionReceipt {
            target_path: path.to_string_lossy().to_string(),
            backup_path: None,
            backup_verified: false,
            modified: false,
            lines_scanned,
            lines_modified: 0,
            redactions_count: 0,
            bytes_before,
            bytes_after: bytes_before,
            original_blake3: original_blake3.clone(),
            final_blake3: original_blake3,
        });
    }

    if let Some(ref bp) = planned_backup {
        create_verified_backup(
            path,
            parent_dir,
            bp,
            bytes_before,
            &original_blake3,
            &original_perms,
        )?;
    }

    redacted_temp.as_file().set_permissions(original_perms)?;
    redacted_temp
        .persist(path)
        .map_err(|e| OooError::Io(e.error))?;

    let (disk_bytes, disk_digest) = stream_hash_and_len(path)?;
    if disk_bytes != bytes_after || disk_digest != final_blake3 {
        return Err(OooError::General(format!(
            "On-disk final verification failed for {} (expected {} bytes / {}, got {} bytes / {})",
            path.display(),
            bytes_after,
            final_blake3,
            disk_bytes,
            disk_digest
        )));
    }

    Ok(InPlaceRedactionReceipt {
        target_path: path.to_string_lossy().to_string(),
        backup_path: planned_backup.map(|p| p.to_string_lossy().to_string()),
        backup_verified: create_backup,
        modified: true,
        lines_scanned,
        lines_modified,
        redactions_count,
        bytes_before,
        bytes_after,
        original_blake3,
        final_blake3,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detox::sanitizer::scan_transcript_secrets;
    use tempfile::TempDir;

    #[test]
    fn test_in_place_redaction_with_backup_verification() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("session.jsonl");

        let clean_line_1 = "{\"id\":\"sk-ant-structural-12345678901234567890\",\"source\":\"USER\",\"content\":\"hello world\"}\r\n";
        let secret_nested = "{\"source\":\"MODEL\",\"content\":[{\"type\":\"text\",\"text\":\"leaked sk-ant-api03-AbCdEfGhIjKlMnOpQrStUvWx and \x1b[31mred\x1b[0m\"}],\"tool_calls\":[{\"name\":\"bash\",\"args\":{\"cmd\":\"export AWS=AKIAABCDEFGHIJKLMNOP\"}}]}\r\n";
        let sidechain_line = "{\"isSidechain\":true,\"source\":\"MODEL\",\"content\":\"sidechain ghp_abcdefghijklmnopqrstuvwxyz0123456789\"}\n";
        let clean_line_2 = "  {\"source\":\"USER\",  \"content\": \"keep formatting exact\"}\n";
        let malformed_line = "not-json \x1b[32mBearer abcDEF1234567890ghijKLMN\x1b[0m";

        let original_content =
            format!("{clean_line_1}{secret_nested}{sidechain_line}{clean_line_2}{malformed_line}");
        std::fs::write(&file_path, &original_content).unwrap();

        let receipt = redact_transcript_in_place(&file_path, true).unwrap();
        assert!(receipt.modified);
        assert!(receipt.backup_verified);
        assert_eq!(receipt.lines_scanned, 5);
        assert_eq!(receipt.lines_modified, 3);
        assert_eq!(receipt.redactions_count, 4);
        assert_eq!(receipt.bytes_before, original_content.len() as u64);

        let backup_path = PathBuf::from(receipt.backup_path.as_ref().unwrap());
        assert!(backup_path.exists());
        let backup_bytes = std::fs::read(&backup_path).unwrap();
        assert_eq!(backup_bytes, original_content.as_bytes());
        assert_eq!(
            blake3::hash(&backup_bytes).to_hex().to_string(),
            receipt.original_blake3
        );

        let updated_bytes = std::fs::read(&file_path).unwrap();
        let updated_str = String::from_utf8(updated_bytes.clone()).unwrap();
        assert_eq!(updated_bytes.len() as u64, receipt.bytes_after);
        assert_eq!(
            blake3::hash(&updated_bytes).to_hex().to_string(),
            receipt.final_blake3
        );

        // Clean lines preserved byte-for-byte
        assert!(updated_str.starts_with(clean_line_1));
        assert!(updated_str.contains(clean_line_2));

        // Line endings, structural keys, and ANSI codes preserved
        assert!(updated_str.starts_with(clean_line_1));
        assert!(updated_str.contains(clean_line_2));
        assert!(updated_str.contains("\r\n"));
        assert!(updated_str.contains("\x1b[31mred\x1b[0m"));
        assert!(updated_str.contains("not-json \x1b[32m[REDACTED_SECRET]\x1b[0m"));
        assert!(!updated_str.ends_with('\n'));
    }

    #[test]
    fn test_in_place_redaction_idempotent_when_backup_already_exists_on_clean_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("idempotent.jsonl");
        let original =
            "{\"source\":\"USER\",\"content\":\"sk-ant-api03-AbCdEfGhIjKlMnOpQrStUvWx\"}\n";
        std::fs::write(&file_path, original).unwrap();

        let first = redact_transcript_in_place(&file_path, true).unwrap();
        assert!(first.modified);
        assert!(first.backup_verified);

        // Second pass on the now-clean file must succeed as a no-op without failing on existing .bak
        let second = redact_transcript_in_place(&file_path, true).unwrap();
        assert!(!second.modified);
        assert_eq!(second.backup_path, None);
        assert_eq!(second.redactions_count, 0);
    }

    #[test]
    fn test_in_place_redaction_preserves_clean_lines_and_passes_scanner() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("clean_scan.jsonl");

        let clean_line_1 =
            "{\"id\":\"msg-12345\",\"source\":\"USER\",\"content\":\"hello world\"}\r\n";
        let secret_nested = "{\"source\":\"MODEL\",\"content\":[{\"type\":\"text\",\"text\":\"leaked sk-ant-api03-AbCdEfGhIjKlMnOpQrStUvWx and \x1b[31mred\x1b[0m\"}],\"tool_calls\":[{\"name\":\"bash\",\"args\":{\"cmd\":\"export AWS=AKIAABCDEFGHIJKLMNOP\"}}]}\r\n";
        let sidechain_line = "{\"isSidechain\":true,\"source\":\"MODEL\",\"content\":\"sidechain ghp_abcdefghijklmnopqrstuvwxyz0123456789\"}\n";
        let clean_line_2 = "  {\"source\":\"USER\",  \"content\": \"keep formatting exact\"}\n";
        let malformed_line = "not-json \x1b[32mBearer abcDEF1234567890ghijKLMN\x1b[0m";

        let original_content =
            format!("{clean_line_1}{secret_nested}{sidechain_line}{clean_line_2}{malformed_line}");
        std::fs::write(&file_path, &original_content).unwrap();

        let receipt = redact_transcript_in_place(&file_path, true).unwrap();
        assert!(receipt.modified);
        assert!(receipt.backup_verified);
        assert_eq!(receipt.redactions_count, 4);

        let scan = scan_transcript_secrets(&file_path).unwrap();
        assert_eq!(scan.total_occurrences, 0);
        assert_eq!(scan.distinct_secrets, 0);
    }

    #[test]
    fn test_in_place_redaction_clean_file_is_noop() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("clean.jsonl");
        let content = "{\"source\":\"USER\",\"content\":\"Use Bearer tokens safely\"}\n";
        std::fs::write(&file_path, content).unwrap();

        let receipt = redact_transcript_in_place(&file_path, true).unwrap();
        assert!(!receipt.modified);
        assert_eq!(receipt.backup_path, None);
        assert!(!receipt.backup_verified);
        assert_eq!(receipt.lines_scanned, 1);
        assert_eq!(receipt.lines_modified, 0);
        assert_eq!(receipt.redactions_count, 0);
        assert_eq!(receipt.bytes_before, receipt.bytes_after);
        assert_eq!(receipt.original_blake3, receipt.final_blake3);

        let bak_path = dir.path().join("clean.jsonl.bak");
        assert!(!bak_path.exists());
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), content);
    }

    #[test]
    fn test_in_place_redaction_refuses_existing_backup_and_non_regular_files() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("transcript.jsonl");
        let bak_path = dir.path().join("transcript.jsonl.bak");
        let original =
            "{\"source\":\"USER\",\"content\":\"sk-ant-api03-AbCdEfGhIjKlMnOpQrStUvWx\"}\n";

        std::fs::write(&file_path, original).unwrap();
        std::fs::write(&bak_path, "existing backup").unwrap();

        let err = redact_transcript_in_place(&file_path, true).unwrap_err();
        assert!(err.to_string().contains("existing backup"));
        assert_eq!(std::fs::read_to_string(&file_path).unwrap(), original);
        assert_eq!(
            std::fs::read_to_string(&bak_path).unwrap(),
            "existing backup"
        );

        let dir_err = redact_transcript_in_place(dir.path(), false).unwrap_err();
        assert!(dir_err.to_string().contains("non-regular file"));

        #[cfg(unix)]
        {
            let link_path = dir.path().join("link.jsonl");
            std::os::unix::fs::symlink(&file_path, &link_path).unwrap();
            let link_err = redact_transcript_in_place(&link_path, false).unwrap_err();
            assert!(link_err.to_string().contains("non-regular file"));
            assert_eq!(std::fs::read_to_string(&file_path).unwrap(), original);
        }
    }

    #[test]
    fn test_resolve_in_place_targets_with_sibling_full() {
        let dir = TempDir::new().unwrap();
        let compact = dir.path().join("transcript.jsonl");
        let full = dir.path().join("transcript_full.jsonl");
        let standalone = dir.path().join("custom.jsonl");

        std::fs::write(&compact, "{}\n").unwrap();
        assert_eq!(resolve_in_place_targets(&compact), vec![compact.clone()]);

        std::fs::write(&full, "{}\n").unwrap();
        assert_eq!(
            resolve_in_place_targets(&compact),
            vec![full.clone(), compact.clone()]
        );

        std::fs::write(&standalone, "{}\n").unwrap();
        assert_eq!(resolve_in_place_targets(&standalone), vec![standalone]);
    }
}
