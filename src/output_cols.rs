
#[derive(Debug)]
pub enum OutputColumn {
    // Generic columns found in all events
    Hostname,
    RecordID,
    Timestamp,
    Provider,
    EventID,
    Version,
    EventSpecific(u32), // 1-indexed event-specific data field
    UnformattedMessage, // Template string, if any
    FormattedMessage, // Formatted template string, if any
}

pub fn parse_column_names(names: &str) -> Result<Vec<OutputColumn>, String> {
    let column_names : Vec<&str> = names.split(",").collect();
    let mut columns = Vec::new();
    let mut last_prop_num : Option<u32> = None;
    let mut expand_next_prop_num : bool = false;

    for name in column_names {
        if name.eq("...") {
            expand_next_prop_num = true;
            continue;
        }
        let col = match name {
            "hostname" => OutputColumn::Hostname,
            "recordid" => OutputColumn::RecordID,
            "timestamp" => OutputColumn::Timestamp,
            "provider" => OutputColumn::Provider,
            "eventid" => OutputColumn::EventID,
            "version" => OutputColumn::Version,
            "unformatted_message" => OutputColumn::UnformattedMessage,
            "formatted_message" => OutputColumn::FormattedMessage,
            s if s.starts_with("variant") => {
                let s = s.replace("variant", "");
                let prop_num = match s.parse::<u32>() {
                    Ok(i) => i,
                    Err(e) => return Err(
                        format!("Unexpected output column name '{}' : {}", s, e)),
                };
                if expand_next_prop_num {
                    let last_prop_num = match last_prop_num {
                        Some(i) => i,
                        None => return Err(format!("Expecting variantN column name after '...'")),
                    };
                    for i in (last_prop_num+1)..prop_num {
                        columns.push(OutputColumn::EventSpecific(i));
                    }
                }
                OutputColumn::EventSpecific(prop_num)
            },
            other => return Err(format!("Unexpected output column name '{}'", other))
        };
        expand_next_prop_num = false;
        last_prop_num = match col {
            OutputColumn::EventSpecific(prop_num) => Some(prop_num),
            _ => None,
        };
        columns.push(col);
    }

    if expand_next_prop_num {
        return Err(format!("Expecting output column name after '...'"));
    }
    Ok(columns)
}