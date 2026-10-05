use super::{RecordCommand, Session, TagCommand, parse_record_id, read_record_password, success};
use crate::Result;
use repass_storage::{FieldUpdate, NewRecord, RecordPatch};
use std::io::{BufRead, Write};

pub(super) fn execute_record(
    command: RecordCommand,
    session: &mut Session,
    input: &mut impl BufRead,
    output: &mut impl Write,
    interactive: bool,
) -> Result<()> {
    match command {
        RecordCommand::Add {
            name,
            password_stdin: _,
            username,
            url,
            notes,
            tag,
        } => {
            session.ensure_storage(output, interactive)?;
            let password = read_record_password(session, input, output, interactive)?;
            let id = session
                .ensure_storage(output, interactive)?
                .create_record(NewRecord {
                    name,
                    password,
                    username,
                    url,
                    notes,
                    tags: tag,
                })?;
            success(output, format_args!("Record created with ID {id}"))
        }
        RecordCommand::List { tag, name } | RecordCommand::Find { tag, name } => {
            let storage = session.ensure_storage(output, interactive)?;
            let records = storage.search_records(name.as_deref(), &tag)?;
            for record in records {
                let tag_names = record
                    .tags
                    .iter()
                    .map(|id| {
                        storage
                            .get_tag(*id)
                            .map(|tag| {
                                if tag.is_technical() {
                                    format!("{}:{} (technical)", id, tag.name())
                                } else {
                                    format!("{}:{}", id, tag.name())
                                }
                            })
                            .unwrap_or_else(|| format!("{id}:#tag-{id} (technical)"))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                writeln!(
                    output,
                    "{}\t{}\t{}\t{}",
                    record.id,
                    record.name,
                    record.username.unwrap_or(""),
                    tag_names
                )?;
            }
            Ok(())
        }
        RecordCommand::Show { record_id, reveal } => {
            let id = parse_record_id(&record_id)?;
            let storage = session.ensure_storage(output, interactive)?;
            let record = storage.get_record(id)?;
            writeln!(output, "ID: {}", record.id)?;
            writeln!(output, "Name: {}", record.name)?;
            writeln!(output, "Username: {}", record.username.unwrap_or(""))?;
            writeln!(output, "URL: {}", record.url.unwrap_or(""))?;
            writeln!(output, "Notes: {}", record.notes.unwrap_or(""))?;
            let tag_names = record
                .tags
                .iter()
                .map(|id| {
                    storage
                        .get_tag(*id)
                        .map(|tag| {
                            if tag.is_technical() {
                                format!("{} (technical)", tag.name())
                            } else {
                                tag.name().to_owned()
                            }
                        })
                        .unwrap_or_else(|| format!("#tag-{id}"))
                })
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(output, "Tags: {tag_names}")?;
            writeln!(
                output,
                "Password: {}",
                if reveal {
                    record.password()
                } else {
                    "********"
                }
            )?;
            writeln!(
                output,
                "Created (Unix ms): {}",
                record.created.as_unix_millis()
            )?;
            writeln!(
                output,
                "Updated (Unix ms): {}",
                record.updated.as_unix_millis()
            )?;
            Ok(())
        }
        RecordCommand::Update {
            record_id,
            name,
            username,
            url,
            notes,
            clear_username,
            clear_url,
            clear_notes,
            password_stdin,
            add_tag,
            remove_tag,
        } => {
            let id = parse_record_id(&record_id)?;
            if password_stdin {
                session.ensure_storage(output, interactive)?;
            }
            let password = if password_stdin {
                Some(read_record_password(session, input, output, interactive)?)
            } else {
                None
            };
            let changed = session.ensure_storage(output, interactive)?.update_record(
                id,
                RecordPatch {
                    name,
                    password,
                    username: optional_field(username, clear_username),
                    url: optional_field(url, clear_url),
                    notes: optional_field(notes, clear_notes),
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
        RecordCommand::Delete { record_id } => {
            let id = parse_record_id(&record_id)?;
            session
                .ensure_storage(output, interactive)?
                .delete_record(id)?;
            success(output, format_args!("Record {id} deleted"))
        }
    }
}

fn optional_field(value: Option<String>, clear: bool) -> FieldUpdate<String> {
    if clear {
        FieldUpdate::Clear
    } else {
        value.map_or(FieldUpdate::Keep, FieldUpdate::Set)
    }
}

pub(super) fn execute_tag(
    command: TagCommand,
    session: &mut Session,
    output: &mut impl Write,
    interactive: bool,
) -> Result<()> {
    match command {
        TagCommand::Add { name } => {
            let id = session
                .ensure_storage(output, interactive)?
                .create_tag(name)?;
            success(output, format_args!("Tag created with ID {id}"))
        }
        TagCommand::List => {
            let storage = session.ensure_storage(output, interactive)?;
            for tag in storage.list_tags() {
                if tag.is_technical() {
                    writeln!(output, "{}\t{}\t(technical)", tag.id(), tag.name())?;
                } else {
                    writeln!(output, "{}\t{}", tag.id(), tag.name())?;
                }
            }
            Ok(())
        }
        TagCommand::Delete { tag_id } => {
            session
                .ensure_storage(output, interactive)?
                .delete_tag(tag_id)?;
            success(output, format_args!("Tag {tag_id} deleted"))
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
