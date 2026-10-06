use super::{
    DataInput, RecordCommand, Session, SshPart, TagCommand, TotpAlgorithmArg, parse_record_id,
    read_record_password, success,
};
use crate::{Result, output};
use repass_storage::record::types::Timestamp;
use repass_storage::{
    Data, FieldUpdate, NewRecord, RecordPatch, SshKey, StorageError, Totp, TotpAlgorithm,
};
use std::collections::HashSet;
use std::io::{BufRead, Write};

pub(super) fn execute_record(
    command: RecordCommand,
    session: &mut Session,
    input: &mut impl BufRead,
    output: &mut impl Write,
    interactive: bool,
) -> Result<()> {
    match command {
        RecordCommand::Create {
            name,
            data,
            username,
            host,
            notes,
            tag,
        } => {
            let storage = session.ensure_storage(output, interactive)?;
            if name.is_empty() {
                return Err("record name cannot be empty".into());
            }
            let mut seen = HashSet::new();
            for id in &tag {
                if storage.get_tag(*id).is_none() {
                    return Err(StorageError::TagNotFound(*id).into());
                }
                if !seen.insert(*id) {
                    return Err(StorageError::DuplicateTagId(*id).into());
                }
            }
            let data = read_data(data, session, input, output, interactive)?
                .into_iter()
                .collect();
            let id = session
                .ensure_storage(output, interactive)?
                .create_record(NewRecord {
                    name: name.0,
                    data,
                    username,
                    host,
                    notes,
                    tags: tag,
                })?;
            success(output, format_args!("Record created with ID {id}"))
        }
        RecordCommand::List { tag, name, host } => execute_record(
            RecordCommand::Find {
                tag,
                name,
                host,
                query: None,
            },
            session,
            input,
            output,
            interactive,
        ),
        RecordCommand::Find {
            tag,
            name,
            host,
            query,
        } => {
            let storage = session.ensure_storage(output, interactive)?;
            let records = match query {
                Some(query) => {
                    storage.fuzzy_search_records(&query, name.as_deref(), host.as_ref(), &tag)?
                }
                None => storage.search_records_by_host(name.as_deref(), host.as_ref(), &tag)?,
            };
            for record in records {
                let tag_names = record
                    .tags
                    .iter()
                    .map(|id| {
                        storage
                            .get_tag(*id)
                            .map(|tag| {
                                if tag.is_technical() {
                                    format!("{}:{} (technical)", id, output::tag_text(tag.name()))
                                } else {
                                    format!("{}:{}", id, output::tag_text(tag.name()))
                                }
                            })
                            .unwrap_or_else(|| format!("{id}:#tag-{id} (technical)"))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                writeln!(
                    output,
                    "{}\t{}\t{}\t{}\t{}\t{}",
                    record.id,
                    output::text(record.name),
                    output::text(record.username.unwrap_or("")),
                    tag_names,
                    record.host.map(ToString::to_string).unwrap_or_default(),
                    record
                        .data()
                        .iter()
                        .map(|entry| format!("{}:{}", entry.id, entry.value.kind()))
                        .collect::<Vec<_>>()
                        .join(", ")
                )?;
            }
            Ok(())
        }
        RecordCommand::Show {
            record_id,
            reveal,
            data_id,
            raw,
            ssh_part,
        } => {
            let id = parse_record_id(&record_id)?;
            let storage = session.ensure_storage(output, interactive)?;
            let record = storage.get_record(id)?;
            let selected = data_id
                .map(|id| {
                    record
                        .data()
                        .iter()
                        .find(|entry| entry.id == id)
                        .ok_or(StorageError::DataNotFound(id))
                })
                .transpose()?;
            if raw {
                let entry = selected.ok_or("raw output requires --data-id")?;
                let value = match (&entry.value, ssh_part) {
                    (Data::SshKey(key), Some(SshPart::Private)) => key
                        .private_key
                        .as_deref()
                        .ok_or("selected SSH element has no private key")?,
                    (Data::SshKey(key), Some(SshPart::Public)) => key
                        .public_key
                        .as_deref()
                        .ok_or("selected SSH element has no public key")?,
                    (Data::SshKey(_), None) => {
                        return Err("raw SSH output requires --ssh-part private or public".into());
                    }
                    (_, Some(_)) => return Err("--ssh-part requires an SSH data element".into()),
                    (Data::Password(value) | Data::Code(value), None) => value,
                    (Data::Totp(totp), None) => &totp.secret,
                };
                output.write_all(value.as_bytes())?;
                return Ok(());
            }
            writeln!(output, "ID: {}", record.id)?;
            writeln!(output, "Name: {}", output::text(record.name))?;
            writeln!(
                output,
                "Username: {}",
                output::text(record.username.unwrap_or(""))
            )?;
            writeln!(
                output,
                "Host: {}",
                record.host.map(ToString::to_string).unwrap_or_default()
            )?;
            output::multiline(output, "Notes", record.notes.unwrap_or(""))?;
            let tag_names = record
                .tags
                .iter()
                .map(|id| {
                    storage
                        .get_tag(*id)
                        .map(|tag| {
                            if tag.is_technical() {
                                format!("{} (technical)", output::tag_text(tag.name()))
                            } else {
                                output::tag_text(tag.name())
                            }
                        })
                        .unwrap_or_else(|| format!("#tag-{id}"))
                })
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(output, "Tags: {tag_names}")?;
            for entry in record.data() {
                if data_id.is_some_and(|id| entry.id != id) {
                    continue;
                }
                writeln!(output, "Data {} ({}):", entry.id, entry.value.kind())?;
                show_data(&entry.value, reveal, output)?;
            }
            writeln!(output, "Created: {}", format_timestamp(*record.created)?)?;
            writeln!(output, "Updated: {}", format_timestamp(*record.updated)?)?;
            Ok(())
        }
        RecordCommand::Update {
            record_id,
            name,
            username,
            host,
            notes,
            remove_username,
            remove_host,
            remove_notes,
            data,
            replace_data,
            remove_data,
            add_tag,
            remove_tag,
        } => {
            let id = parse_record_id(&record_id)?;
            if replace_data.is_some_and(|data_id| remove_data.contains(&data_id)) {
                return Err("cannot replace and remove the same data element".into());
            }
            let storage = session.ensure_storage(output, interactive)?;
            let record = storage.get_record(id)?;
            if name.as_ref().is_some_and(String::is_empty) {
                return Err("record name cannot be empty".into());
            }
            for tags in [&add_tag, &remove_tag] {
                let mut seen = HashSet::new();
                for tag in tags {
                    if storage.get_tag(*tag).is_none() {
                        return Err(StorageError::TagNotFound(*tag).into());
                    }
                    if !seen.insert(*tag) {
                        return Err(StorageError::DuplicateTagId(*tag).into());
                    }
                }
            }
            if let Some(tag) = add_tag.iter().find(|tag| remove_tag.contains(tag)) {
                return Err(StorageError::ConflictingTagUpdate(*tag).into());
            }
            let mut touched = HashSet::new();
            for data_id in replace_data.iter().chain(&remove_data) {
                if !touched.insert(*data_id) {
                    return Err(StorageError::ConflictingDataUpdate(*data_id).into());
                }
                if !record.data().iter().any(|entry| entry.id == *data_id) {
                    return Err(StorageError::DataNotFound(*data_id).into());
                }
            }
            let value = read_data(data, session, input, output, interactive)?;
            let mut add_data = Vec::new();
            let mut replacements = Vec::new();
            match (replace_data, value) {
                (Some(data_id), Some(value)) => replacements.push((data_id, value)),
                (Some(_), None) => return Err("--replace-data requires a data source".into()),
                (None, Some(value)) => add_data.push(value),
                (None, None) => {}
            }
            let changed = session.ensure_storage(output, interactive)?.update_record(
                id,
                RecordPatch {
                    name,
                    add_data,
                    replace_data: replacements,
                    remove_data,
                    username: optional_field(username, remove_username),
                    host: optional_field(host, remove_host),
                    notes: optional_field(notes, remove_notes),
                    add_tags: add_tag,
                    remove_tags: remove_tag,
                },
            )?;
            if changed {
                success(output, format_args!("Record {id} updated"))
            } else {
                writeln!(output, "Record {id} unchanged")?;
                Ok(())
            }
        }
        RecordCommand::Remove { record_id } => {
            let id = parse_record_id(&record_id)?;
            session
                .ensure_storage(output, interactive)?
                .delete_record(id)?;
            success(output, format_args!("Record {id} removed"))
        }
    }
}

// Persisted timestamps cover all u64 milliseconds, beyond the calendar range.
fn format_timestamp(timestamp: Timestamp) -> Result<String> {
    let milliseconds = timestamp.as_unix_millis();
    let datetime =
        match time::OffsetDateTime::from_unix_timestamp_nanos(i128::from(milliseconds) * 1_000_000)
        {
            Ok(datetime) => datetime,
            Err(_) => {
                return Ok(format!(
                    "outside supported date range (Unix ms: {milliseconds})"
                ));
            }
        };
    Ok(datetime.format(time::macros::format_description!(
        "[year]-[month]-[day] [hour]:[minute]"
    ))?)
}

fn optional_field<T>(value: Option<T>, clear: bool) -> FieldUpdate<T> {
    if clear {
        FieldUpdate::Clear
    } else {
        value.map_or(FieldUpdate::Keep, FieldUpdate::Set)
    }
}

fn read_secret_line(
    label: &str,
    session: &mut Session,
    input: &mut impl BufRead,
    output: &mut impl Write,
    interactive: bool,
) -> Result<String> {
    if interactive {
        crate::output::styled(output, crate::output::PROMPT, format_args!("{label}: "))?;
        output.flush()?;
        return Ok(session.read_secret()?);
    }
    let mut text = crate::input::read_line(input, crate::input::MAX_TEXT_BYTES)?;
    if text.is_empty() {
        return Err(format!("expected {label} on stdin").into());
    }
    if text.ends_with('\n') {
        text.pop();
        if text.ends_with('\r') {
            text.pop();
        }
    }
    Ok(text)
}

fn read_key(
    label: &str,
    input: &mut impl BufRead,
    output: &mut impl Write,
    interactive: bool,
) -> Result<String> {
    let mut key = String::new();
    if !interactive {
        return Ok(crate::input::read_text(input)?);
    }
    writeln!(
        output,
        "{label}: enter key text; finish with a line containing only '.'"
    )?;
    output.flush()?;
    loop {
        let line = crate::input::read_line(input, crate::input::MAX_TEXT_BYTES - key.len() + 3)?;
        if line.is_empty() {
            return Err("input ended before the SSH key terminator '.'".into());
        }
        if line.trim_end_matches(['\r', '\n']) == "." {
            break;
        }
        if key.len() + line.len() > crate::input::MAX_TEXT_BYTES {
            return Err("SSH key exceeds 1 MiB size limit".into());
        }
        key.push_str(&line);
    }
    Ok(key)
}

fn read_data(
    args: DataInput,
    session: &mut Session,
    input: &mut impl BufRead,
    output: &mut impl Write,
    interactive: bool,
) -> Result<Option<Data>> {
    let ssh = args.private_key_file.is_some()
        || args.public_key_file.is_some()
        || args.private_key_stdin
        || args.public_key_stdin;
    if [args.password_stdin, args.code_stdin, args.totp_stdin, ssh]
        .into_iter()
        .filter(|present| *present)
        .count()
        > 1
    {
        return Err("choose one data type per operation".into());
    }
    if !interactive && args.private_key_stdin && args.public_key_stdin {
        return Err(
            "only one SSH field can read stdin until EOF; use a file for the other field".into(),
        );
    }
    if args.digits.is_some_and(|digits| !(6..=8).contains(&digits)) {
        return Err("TOTP digits must be between 6 and 8".into());
    }
    if args.period == Some(0) {
        return Err("TOTP period must be positive".into());
    }
    let value = if args.password_stdin {
        Data::Password(read_record_password(session, input, output, interactive)?)
    } else if args.code_stdin {
        Data::Code(read_secret_line(
            "Code",
            session,
            input,
            output,
            interactive,
        )?)
    } else if args.totp_stdin {
        let secret = read_secret_line("TOTP secret (Base32)", session, input, output, interactive)?;
        let algorithm = match args.algorithm.unwrap_or_default() {
            TotpAlgorithmArg::Sha1 => TotpAlgorithm::Sha1,
            TotpAlgorithmArg::Sha256 => TotpAlgorithm::Sha256,
            TotpAlgorithmArg::Sha512 => TotpAlgorithm::Sha512,
        };
        Data::Totp(Totp::new(
            secret,
            algorithm,
            args.digits.unwrap_or(6),
            args.period.unwrap_or(30),
        )?)
    } else if ssh {
        let private_key = match args.private_key_file {
            Some(path) => Some(crate::input::read_file(&path)?),
            None if args.private_key_stdin => {
                Some(read_key("Private SSH key", input, output, interactive)?)
            }
            None => None,
        };
        let public_key = match args.public_key_file {
            Some(path) => Some(crate::input::read_file(&path)?),
            None if args.public_key_stdin => {
                Some(read_key("Public SSH key", input, output, interactive)?)
            }
            None => None,
        };
        Data::SshKey(SshKey::new(private_key, public_key)?)
    } else {
        return Ok(None);
    };
    value.validate()?;
    Ok(Some(value))
}

fn show_data(data: &Data, reveal: bool, output: &mut impl Write) -> Result<()> {
    match data {
        Data::Password(value) => writeln!(
            output,
            "Password: {}",
            if reveal {
                output::text(value)
            } else {
                "********".into()
            }
        )?,
        Data::Code(value) => writeln!(
            output,
            "Code: {}",
            if reveal {
                output::text(value)
            } else {
                "********".into()
            }
        )?,
        Data::SshKey(key) => {
            for (label, value) in [
                ("Private SSH key", &key.private_key),
                ("Public SSH key", &key.public_key),
            ] {
                if let Some(value) = value {
                    if reveal {
                        output::multiline(output, label, value)?;
                    } else {
                        writeln!(output, "{label}: ********")?;
                    }
                }
            }
        }
        Data::Totp(totp) => {
            writeln!(
                output,
                "TOTP secret: {}",
                if reveal { &totp.secret } else { "********" }
            )?;
            writeln!(
                output,
                "Algorithm: {:?}\nDigits: {}\nPeriod (seconds): {}",
                totp.algorithm, totp.digits, totp.period
            )?;
        }
    }
    Ok(())
}

pub(super) fn execute_tag(
    command: TagCommand,
    session: &mut Session,
    output: &mut impl Write,
    interactive: bool,
) -> Result<()> {
    match command {
        TagCommand::Create { name } => {
            let id = session
                .ensure_storage(output, interactive)?
                .create_tag(name.0)?;
            success(output, format_args!("Tag created with ID {id}"))
        }
        TagCommand::List => {
            let storage = session.ensure_storage(output, interactive)?;
            for tag in storage.list_tags() {
                if tag.is_technical() {
                    writeln!(
                        output,
                        "{}\t{}\t(technical)",
                        tag.id(),
                        output::text(tag.name())
                    )?;
                } else {
                    writeln!(output, "{}\t{}", tag.id(), output::text(tag.name()))?;
                }
            }
            Ok(())
        }
        TagCommand::Remove { tag_id } => {
            session
                .ensure_storage(output, interactive)?
                .delete_tag(tag_id.0)?;
            success(output, format_args!("Tag {} removed", tag_id.0))
        }
        TagCommand::Rename { tag_id, name } => {
            session
                .ensure_storage(output, interactive)?
                .rename_tag(tag_id, name)?;
            success(output, format_args!("Tag {tag_id} renamed"))
        }
        TagCommand::Recover => {
            let count = session
                .ensure_storage(output, interactive)?
                .recover_tags()?;
            success(
                output,
                format_args!("Tag catalog rebuilt with {count} tags"),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_show_dates_to_the_minute_and_handle_extreme_values() {
        for (milliseconds, expected) in [
            (0, "1970-01-01 00:00"),
            (1_709_164_800_123, "2024-02-29 00:00"),
            (253_402_300_799_999, "9999-12-31 23:59"),
        ] {
            assert_eq!(
                format_timestamp(Timestamp::from_unix_millis(milliseconds)).unwrap(),
                expected
            );
        }
        assert_eq!(
            format_timestamp(Timestamp::from_unix_millis(u64::MAX)).unwrap(),
            "outside supported date range (Unix ms: 18446744073709551615)"
        );
    }
}
